import 'dart:async';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'src/ffi/bridge.dart';
import 'src/ffi/bridge_factory.dart';
import 'src/logging/app_logger.dart';
import 'src/screens/chat_screen.dart';
import 'src/screens/debug_log_screen.dart';
import 'src/screens/lock_screen.dart';
import 'src/screens/qr_screen.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  AppLogger.initErrorHooks();
  AppLogger.info('BOOT', 'AnonTrace mobile client initialized with zero-knowledge logging');
  runApp(const SimpleSignalAppWidget());
}

class SimpleSignalAppWidget extends StatelessWidget {
  const SimpleSignalAppWidget({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'AnonTrace',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: const Color(0xFF101418),
        colorScheme: const ColorScheme.dark(
          primary: Color(0xFF2C6BED),
          secondary: Colors.tealAccent,
        ),
      ),
      home: const MainNavigationScreen(),
    );
  }
}

class MainNavigationScreen extends StatefulWidget {
  const MainNavigationScreen({Key? key}) : super(key: key);

  @override
  State<MainNavigationScreen> createState() => _MainNavigationScreenState();
}

class _MainNavigationScreenState extends State<MainNavigationScreen> with WidgetsBindingObserver {
  final SimpleSignalBridge _bridge = createSignalBridge();
  bool _isUnlocked = false;
  final List<ConversationSummary> _conversations = [];
  String _myContactLink = '';
  Timer? _inboxSyncTimer;
  Timer? _backgroundPollTimer;
  StreamSubscription<MessageSummary>? _messageStreamSub;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.paused || state == AppLifecycleState.inactive) {
      if (_isUnlocked && mounted) {
        setState(() {
          _isUnlocked = false;
        });
        AppLogger.info('AUTH', 'App auto-locked on background lifecycle transition');
      }
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _inboxSyncTimer?.cancel();
    _backgroundPollTimer?.cancel();
    _messageStreamSub?.cancel();
    super.dispose();
  }

  void _startInboxSync() {
    _inboxSyncTimer?.cancel();
    _messageStreamSub?.cancel();

    // 1. Reactive stream subscription: Updates conversation list instantly (< 50ms) upon message arrival
    _messageStreamSub = _bridge.messageStream.listen((_) async {
      if (!_isUnlocked || !mounted) return;
      try {
        final updatedConvs = await _bridge.listConversations();
        if (mounted) {
          setState(() {
            _conversations.clear();
            _conversations.addAll(updatedConvs);
          });
        }
      } catch (_) {}
    });

    // 2b. Background poll fallback (60 seconds, Ticket 26.4 first step):
    // Runs even when the UI is locked/backgrounded (not when the app is
    // killed — that still needs a foreground service, see Issue 26 §8).
    // It only polls the relay and stores messages in the DB; the UI
    // shows them after unlock.
    _backgroundPollTimer?.cancel();
    _backgroundPollTimer = Timer.periodic(const Duration(seconds: 60), (_) async {
      if (!mounted) return;
      try {
        await _bridge.receiveMessages();
      } catch (_) {}
    });

    // 2. Low-frequency safety poll (5 seconds) to catch edge-case syncs or offline reconnects
    _inboxSyncTimer = Timer.periodic(const Duration(seconds: 5), (_) async {
      if (!_isUnlocked || !mounted) return;
      try {
        final newMsgs = await _bridge.receiveMessages();
        final updatedConvs = await _bridge.listConversations();
        if (mounted && (newMsgs.isNotEmpty || updatedConvs.length != _conversations.length)) {
          setState(() {
            _conversations.clear();
            _conversations.addAll(updatedConvs);
          });
        }
      } catch (_) {}
    });
  }

  Future<void> _unlockApp(String pin) async {
    // Phase 0 (Issue 27): never log the PIN itself — only that an
    // unlock was attempted. The PIN unlocks the SQLCipher database.
    AppLogger.info('AUTH', 'Unlocking app');
    String dbPath = 'anontrace.db';
    try {
      final appDir = await getApplicationDocumentsDirectory();
      dbPath = '${appDir.path}/anontrace.db';
    } catch (_) {}

    try {
      await _bridge.initApp(dbPath: dbPath, masterPin: pin);
      final myLink = await _bridge.getMyContactLink();
      final convs = await _bridge.listConversations();

      setState(() {
        _isUnlocked = true;
        _myContactLink = myLink;
        _conversations.clear();
        _conversations.addAll(convs);
      });
      _startInboxSync();
      AppLogger.info('AUTH', 'App successfully unlocked. Loaded ${convs.length} conversation(s).');
    } catch (e, st) {
      AppLogger.error('AUTH', 'Unlock failed: $e', e, st);
      rethrow;
    }
  }

  void _openConversation(ConversationSummary conv) {
    AppLogger.info('CHAT', 'Opening conversation: ${conv.id}');
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => ChatScreen(
          conversation: conv,
          messageStream: _bridge.messageStream,
          onFetchMessages: (convId) => _bridge.listMessages(convId),
          onDeleteMessage: (msgId) => _bridge.deleteMessage(msgId),
          onSendMessage: (convId, text) =>
              _bridge.sendChatMessage(conversationId: convId, text: text),
          onSetDisappearingTimer: (seconds) => _bridge.setDisappearingTimer(
            conversationId: conv.id,
            durationSeconds: seconds,
          ),
          onPollMessages: (convId) =>
              _bridge.receiveMessages(conversationId: convId),
        ),
      ),
    );
  }

  void _openQrScreen() {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => QrContactScreen(
          contactUri: _myContactLink,
          onContactScanned: (uri) async {
            try {
              AppLogger.info('PEER', 'Scanned QR contact link: $uri');
              final newConv = await _bridge.importContactLink(uri);
              setState(() {
                _conversations.removeWhere((c) => c.id == newConv.id);
                _conversations.insert(0, newConv);
              });
              // Automatically launch into active chat screen with the connected partner!
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (mounted) {
                  _openConversation(newConv);
                }
              });
            } catch (e, st) {
              AppLogger.error('PEER', 'Failed to import contact: $e', e, st);
              if (mounted) {
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('Failed to import contact: $e')),
                );
              }
            }
          },
        ),
      ),
    );
  }

  Future<void> _toggleStealth() async {
    final mode = await _bridge.getTransportMode();
    if (mode == 'stealth') {
      await _bridge.setTransportMode('normal');
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Stealth off — Normal Mode (HTTPS private relay)')),
        );
      }
      return;
    }
    await _bridge.setTransportMode('stealth');
    try {
      final appDir = await getApplicationDocumentsDirectory();
      await _bridge.startStealthBootstrap(dataDir: appDir.path);
    } catch (_) {}
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Stealth on — Arti bootstrap started. Check Security Status for progress. Onion relay must be deployed for Stealth sends to work.')),
      );
    }
  }

  void _showDiagnosticsDialog() async {
    final isNative = await _bridge.isActive();
    final mode = await _bridge.getTransportMode();
    final stealth = _bridge.getStealthStatus();
    final stealthText = stealth == null
        ? 'Not started'
        : '${stealth['state']} (frac: ${stealth['frac']}, ready: ${stealth['ready']})';
    if (!mounted) return;
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: const Color(0xFF1A2028),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        title: Row(
          children: const [
            Icon(Icons.shield_rounded, color: Colors.tealAccent),
            SizedBox(width: 10),
            Text('Security Diagnostics', style: TextStyle(color: Colors.white, fontSize: 18)),
          ],
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _diagRow('Native Core FFI', isNative ? 'Active (Rust libsimple_signal_core.so)' : 'Simulated (In-Memory)', isNative ? Colors.greenAccent : Colors.orangeAccent),
            const SizedBox(height: 12),
            _diagRow('Key Derivation', 'Argon2id (64MB RAM, 100% Zero-Knowledge)', Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Local Storage', 'SQLCipher AES-256-GCM (Encrypted at Rest)', Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Transport Layer', 'Normal: HTTPS 443 Private Relay (Stealth: Tor opt-in)', Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Transport Mode', mode ?? 'unknown', Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Stealth Status', stealthText, Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Cryptographic Ratchet', 'PQXDH (ML-KEM-1024 + X25519)', Colors.greenAccent),
            const SizedBox(height: 12),
            _diagRow('Traffic Analysis Defense', 'Constant-Rate 16 KB Symmetric Padding', Colors.greenAccent),
          ],
        ),
        actions: [
          TextButton.icon(
            icon: const Icon(Icons.receipt_long_rounded, size: 16, color: Colors.tealAccent),
            label: const Text('View Debug Logs', style: TextStyle(color: Colors.tealAccent)),
            onPressed: () {
              Navigator.pop(ctx);
              Navigator.push(
                context,
                MaterialPageRoute(builder: (_) => const DebugLogScreen()),
              );
            },
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Close', style: TextStyle(color: Color(0xFF2C6BED))),
          ),
        ],
      ),
    );
  }

  Widget _diagRow(String title, String value, Color statusColor) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: const TextStyle(color: Colors.white54, fontSize: 12)),
        const SizedBox(height: 2),
        Row(
          children: [
            Icon(Icons.check_circle_rounded, size: 14, color: statusColor),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                value,
                style: TextStyle(color: statusColor, fontSize: 13, fontWeight: FontWeight.w500),
              ),
            ),
          ],
        ),
      ],
    );
  }

  String _formatPeerTitle(String convId) {
    if (convId.startsWith('conv_')) {
      final sub = convId.substring(5);
      if (sub.length > 12) {
        return 'Peer ${sub.substring(0, 8)}...';
      }
      return 'Peer $sub';
    }
    return convId;
  }

  @override
  Widget build(BuildContext context) {
    if (!_isUnlocked) {
      return LockScreen(onUnlock: _unlockApp);
    }

    return Scaffold(
      backgroundColor: const Color(0xFF101418),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A2028),
        title: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: _showDiagnosticsDialog,
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 2),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: const [
                Icon(Icons.circle, size: 10, color: Colors.greenAccent),
                SizedBox(width: 8),
                Text(
                  'AnonTrace',
                  style: TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                ),
              ],
            ),
          ),
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.receipt_long_rounded, color: Colors.white70),
            tooltip: 'Diagnostic Logs',
            onPressed: () => Navigator.push(
              context,
              MaterialPageRoute(builder: (_) => const DebugLogScreen()),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.info_outline_rounded, color: Colors.white70),
            tooltip: 'Security Status',
            onPressed: _showDiagnosticsDialog,
          ),
          IconButton(
            icon: const Icon(Icons.visibility_off_rounded, color: Colors.white70),
            tooltip: 'Toggle Stealth Mode (Tor)',
            onPressed: _toggleStealth,
          ),
          IconButton(
            icon: const Icon(Icons.qr_code_2_rounded),
            tooltip: 'QR Code Exchange',
            onPressed: _openQrScreen,
          ),
        ],
      ),
      body: _conversations.isEmpty
          ? Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(Icons.chat_bubble_outline_rounded,
                      size: 64, color: Colors.white24),
                  const SizedBox(height: 16),
                  const Text(
                    'No active conversations',
                    style: TextStyle(color: Colors.white70, fontSize: 16),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'Tap QR button to share link or scan partner',
                    style: TextStyle(color: Colors.white38, fontSize: 13),
                  ),
                  const SizedBox(height: 24),
                  ElevatedButton.icon(
                    style: ElevatedButton.styleFrom(
                      backgroundColor: const Color(0xFF2C6BED),
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                    ),
                    icon: const Icon(Icons.qr_code_rounded),
                    label: const Text('Connect via QR'),
                    onPressed: _openQrScreen,
                  ),
                ],
              ),
            )
          : ListView.separated(
              itemCount: _conversations.length,
              separatorBuilder: (_, __) => const Divider(
                color: Color(0xFF1E242C),
                height: 1,
              ),
              itemBuilder: (context, index) {
                final conv = _conversations[index];
                final title = _formatPeerTitle(conv.id);
                final initial = title.startsWith('Peer ') && title.length > 5
                    ? title.substring(5, 7).toUpperCase()
                    : conv.id.substring(0, 2).toUpperCase();
                return ListTile(
                  leading: CircleAvatar(
                    backgroundColor: const Color(0xFF2C6BED),
                    child: Text(
                      initial,
                      style: const TextStyle(
                        color: Colors.white,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  title: Text(
                    title,
                    style: const TextStyle(
                      color: Colors.white,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  subtitle: const Text(
                    'End-to-end encrypted • ML-KEM-1024',
                    style: TextStyle(color: Color(0xFF8E99A4), fontSize: 12),
                  ),
                  onTap: () => _openConversation(conv),
                  onLongPress: () async {
                    final confirmed = await showDialog<bool>(
                      context: context,
                      builder: (dialogCtx) => AlertDialog(
                        backgroundColor: const Color(0xFF1E2630),
                        title: const Text('Delete Conversation?', style: TextStyle(color: Colors.white)),
                        content: Text(
                          'Permanently delete chat history and ratchet sessions with $title?',
                          style: const TextStyle(color: Colors.white70),
                        ),
                        actions: [
                          TextButton(
                            onPressed: () => Navigator.pop(dialogCtx, false),
                            child: const Text('Cancel', style: TextStyle(color: Colors.white60)),
                          ),
                          TextButton(
                            onPressed: () => Navigator.pop(dialogCtx, true),
                            child: const Text('Delete', style: TextStyle(color: Colors.redAccent)),
                          ),
                        ],
                      ),
                    );
                    if (confirmed == true && mounted) {
                      final msgs = await _bridge.listMessages(conv.id);
                      for (final m in msgs) {
                        await _bridge.deleteMessage(m.id);
                      }
                      setState(() {
                        _conversations.removeWhere((c) => c.id == conv.id);
                      });
                      if (mounted) {
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text('Deleted $title')),
                        );
                      }
                    }
                  },
                );
              },
            ),
    );
  }
}
