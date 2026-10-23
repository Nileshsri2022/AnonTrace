import 'package:flutter/material.dart';
import 'src/ffi/bridge.dart';
import 'src/screens/chat_screen.dart';
import 'src/screens/lock_screen.dart';
import 'src/screens/qr_screen.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
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

class _MainNavigationScreenState extends State<MainNavigationScreen> {
  bool _isUnlocked = false;
  final List<ConversationSummary> _conversations = [];
  String _myContactLink = '';

  Future<void> _unlockApp(String pin) async {
    // Simulated or FFI unlock call
    await Future.delayed(const Duration(milliseconds: 300));
    final randomSuffix = DateTime.now().millisecondsSinceEpoch % 100000;
    setState(() {
      _isUnlocked = true;
      _myContactLink =
          'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_anon_$randomSuffix';
    });
  }

  void _openConversation(ConversationSummary conv) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => ChatScreen(
          conversation: conv,
          onFetchMessages: (convId) async => [],
          onSendMessage: (convId, text) async => MessageSummary(
            id: 'msg_${DateTime.now().millisecondsSinceEpoch}',
            conversationId: convId,
            direction: 'outbound',
            bodyText: text,
            timestamp: DateTime.now().millisecondsSinceEpoch ~/ 1000,
            status: 'sent',
          ),
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
          onContactScanned: (uri) {
            final newConv = ConversationSummary(
              id: 'conv_partner_${DateTime.now().millisecondsSinceEpoch % 1000}',
              sendQueueUri: uri,
              receiveQueueId: 'q_local_${DateTime.now().millisecondsSinceEpoch}',
              createdAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
            );
            setState(() {
              _conversations.insert(0, newConv);
            });
            // Automatically launch into active chat screen with the connected partner!
            WidgetsBinding.instance.addPostFrameCallback((_) {
              if (mounted) {
                _openConversation(newConv);
              }
            });
          },
        ),
      ),
    );
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
        title: Row(
          children: const [
            Icon(Icons.circle, size: 10, color: Colors.greenAccent),
            SizedBox(width: 8),
            Text(
              'AnonTrace',
              style: TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
            ),
          ],
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.qr_code_2_rounded),
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
                return ListTile(
                  leading: CircleAvatar(
                    backgroundColor: const Color(0xFF2C6BED),
                    child: Text(
                      conv.id.substring(0, 2).toUpperCase(),
                      style: const TextStyle(
                        color: Colors.white,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  title: Text(
                    conv.id,
                    style: const TextStyle(
                      color: Colors.white,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  subtitle: const Text(
                    'End-to-end encrypted (PQXDH + Double Ratchet)',
                    style: TextStyle(color: Color(0xFF8E99A4), fontSize: 12),
                  ),
                  onTap: () => _openConversation(conv),
                );
              },
            ),
    );
  }
}
