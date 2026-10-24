import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../ffi/bridge.dart';
import '../utils/timeline_adapter.dart';

class ChatScreen extends StatefulWidget {
  final ConversationSummary conversation;
  final Future<List<MessageSummary>> Function(String convId) onFetchMessages;
  final Future<MessageSummary> Function(String convId, String text) onSendMessage;
  final Future<List<MessageSummary>> Function(String convId)? onPollMessages;
  final Future<bool> Function(String messageId)? onDeleteMessage;
  final Future<bool> Function(int seconds)? onSetDisappearingTimer;
  final int initialDisappearingTimer;
  final Stream<MessageSummary>? messageStream;

  const ChatScreen({
    Key? key,
    required this.conversation,
    required this.onFetchMessages,
    required this.onSendMessage,
    this.onPollMessages,
    this.onDeleteMessage,
    this.onSetDisappearingTimer,
    this.initialDisappearingTimer = 0,
    this.messageStream,
  }) : super(key: key);

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final TextEditingController _msgController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  final List<MessageSummary> _messages = [];
  bool _isLoading = true;
  bool _showScrollToBottom = false;
  int _unreadWhileScrolled = 0;
  int _disappearingTimerSeconds = 0;
  MessageSummary? _replyingTo;
  Timer? _pollTimer;
  StreamSubscription<MessageSummary>? _streamSub;

  @override
  void initState() {
    super.initState();
    _disappearingTimerSeconds = widget.initialDisappearingTimer;
    _scrollController.addListener(_onScrollChanged);
    _loadMessages();
    _subscribeToStream();
    _startPolling();
  }

  void _purgeExpired() {
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final hadExpired = _messages.any((m) => m.expiresAt != null && m.expiresAt! <= now);
    if (hadExpired && mounted) {
      setState(() {
        _messages.removeWhere((m) => m.expiresAt != null && m.expiresAt! <= now);
      });
    }
  }

  String _formatTimerDuration(int seconds) {
    if (seconds >= 86400) {
      final days = seconds ~/ 86400;
      return '${days}d';
    } else if (seconds >= 3600) {
      final hours = seconds ~/ 3600;
      return '${hours}h';
    } else if (seconds >= 60) {
      final mins = seconds ~/ 60;
      return '${mins}m';
    } else {
      return '${seconds}s';
    }
  }

  @override
  void dispose() {
    _scrollController.removeListener(_onScrollChanged);
    _scrollController.dispose();
    _streamSub?.cancel();
    _pollTimer?.cancel();
    _msgController.dispose();
    super.dispose();
  }

  void _onScrollChanged() {
    if (!_scrollController.hasClients) return;
    final maxScroll = _scrollController.position.maxScrollExtent;
    final currentScroll = _scrollController.offset;
    final isScrolledAway = (maxScroll - currentScroll) > 80;
    if (isScrolledAway != _showScrollToBottom) {
      setState(() {
        _showScrollToBottom = isScrolledAway;
        if (!isScrolledAway) {
          _unreadWhileScrolled = 0;
        }
      });
    }
  }

  void _scrollToBottom() {
    if (_scrollController.hasClients) {
      _scrollController.animateTo(
        _scrollController.position.maxScrollExtent,
        duration: const Duration(milliseconds: 300),
        curve: Curves.easeOut,
      );
      setState(() {
        _showScrollToBottom = false;
        _unreadWhileScrolled = 0;
      });
    }
  }

  void _subscribeToStream() {
    if (widget.messageStream != null) {
      _streamSub = widget.messageStream!.listen((msg) {
        if (msg.conversationId == widget.conversation.id && mounted) {
          setState(() {
            if (!_messages.any((m) => m.id == msg.id)) {
              _messages.add(msg);
              if (_showScrollToBottom) {
                _unreadWhileScrolled++;
              }
            }
          });
        }
      });
    }
  }

  void _startPolling() {
    // Passive backup poll every 3 seconds to catch any messages saved while offline
    _pollTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      if (!mounted) return;
      try {
        if (widget.onPollMessages != null) {
          await widget.onPollMessages!(widget.conversation.id);
        }
        final msgs = await widget.onFetchMessages(widget.conversation.id);
        if (mounted && msgs.length != _messages.length) {
          setState(() {
            _messages.clear();
            _messages.addAll(msgs);
          });
        }
        _purgeExpired();
      } catch (_) {}
    });
  }

  Future<void> _loadMessages() async {
    try {
      final msgs = await widget.onFetchMessages(widget.conversation.id);
      if (mounted) {
        setState(() {
          _messages.clear();
          _messages.addAll(msgs);
          _isLoading = false;
        });
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && _scrollController.hasClients) {
            _scrollController.jumpTo(_scrollController.position.maxScrollExtent);
          }
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  Future<void> _sendMessage() async {
    final text = _msgController.text.trim();
    if (text.isEmpty) return;

    final quotedMsg = _replyingTo;
    final outboundText = quotedMsg != null
        ? buildQuotedPayload(quotedMsg.bodyText, text)
        : text;

    _msgController.clear();
    setState(() {
      _replyingTo = null;
    });

    try {
      final sent = await widget.onSendMessage(widget.conversation.id, outboundText);
      final sentWithExpiry = (sent.expiresAt == null && _disappearingTimerSeconds > 0)
          ? MessageSummary(
              id: sent.id,
              conversationId: sent.conversationId,
              direction: sent.direction,
              bodyText: sent.bodyText,
              timestamp: sent.timestamp,
              status: sent.status,
              expiresAt: sent.timestamp + _disappearingTimerSeconds,
            )
          : sent;
      if (mounted) {
        setState(() {
          _messages.add(sentWithExpiry);
        });
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && _scrollController.hasClients) {
            _scrollController.animateTo(
              _scrollController.position.maxScrollExtent,
              duration: const Duration(milliseconds: 250),
              curve: Curves.easeOut,
            );
          }
        });
      }
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Send failed: $e')),
      );
    }
  }

  void _showMessageActionSheet(BuildContext context, MessageSummary msg) {
    showModalBottomSheet(
      context: context,
      backgroundColor: const Color(0xFF1E2630),
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (bottomSheetContext) {
        return SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                margin: const EdgeInsets.symmetric(vertical: 8),
                width: 36,
                height: 4,
                decoration: BoxDecoration(
                  color: Colors.white24,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              ListTile(
                leading: const Icon(Icons.reply_rounded, color: Colors.blueAccent),
                title: const Text('Reply', style: TextStyle(color: Colors.white)),
                onTap: () {
                  Navigator.pop(bottomSheetContext);
                  setState(() {
                    _replyingTo = msg;
                  });
                },
              ),
              ListTile(
                leading: const Icon(Icons.copy_rounded, color: Colors.white),
                title: const Text('Copy Text', style: TextStyle(color: Colors.white)),
                onTap: () async {
                  Navigator.pop(bottomSheetContext);
                  await Clipboard.setData(ClipboardData(text: msg.bodyText));
                  if (mounted) {
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(
                        content: Text('Message copied to clipboard'),
                        duration: Duration(seconds: 2),
                      ),
                    );
                  }
                },
              ),
              ListTile(
                leading: const Icon(Icons.delete_outline_rounded, color: Colors.redAccent),
                title: const Text('Delete for Me', style: TextStyle(color: Colors.redAccent)),
                onTap: () async {
                  Navigator.pop(bottomSheetContext);
                  final confirmed = await showDialog<bool>(
                    context: context,
                    builder: (dialogCtx) => AlertDialog(
                      backgroundColor: const Color(0xFF1E2630),
                      title: const Text('Delete Message?', style: TextStyle(color: Colors.white)),
                      content: const Text(
                        'This message will be permanently deleted from your local encrypted storage.',
                        style: TextStyle(color: Colors.white70),
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
                    final success = await widget.onDeleteMessage?.call(msg.id) ?? true;
                    if (success && mounted) {
                      setState(() {
                        _messages.removeWhere((m) => m.id == msg.id);
                      });
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                          content: Text('Message deleted'),
                          duration: Duration(seconds: 2),
                        ),
                      );
                    }
                  }
                },
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildStatusTick(String status) {
    if (status == 'read') {
      return const Icon(
        Icons.done_all,
        key: ValueKey('tick_read'),
        size: 14,
        color: Color(0xFF34B7F1),
      );
    } else if (status == 'delivered') {
      return const Icon(
        Icons.done_all,
        key: ValueKey('tick_delivered'),
        size: 14,
        color: Colors.white70,
      );
    } else {
      return const Icon(
        Icons.done,
        key: ValueKey('tick_sent'),
        size: 13,
        color: Colors.white70,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF101418),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A2028),
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              widget.conversation.id,
              style: const TextStyle(fontSize: 15, fontWeight: FontWeight.bold),
            ),
            Row(
              children: [
                if (_disappearingTimerSeconds > 0) ...[
                  const Icon(Icons.timer_outlined, size: 10, color: Colors.amberAccent),
                  const SizedBox(width: 3),
                  Text(
                    _formatTimerDuration(_disappearingTimerSeconds),
                    key: const ValueKey('appbar_timer_indicator'),
                    style: const TextStyle(fontSize: 10, color: Colors.amberAccent, fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(width: 6),
                ],
                const Icon(Icons.lock_outline, size: 12, color: Colors.greenAccent),
                const SizedBox(width: 4),
                const Text(
                  'ML-KEM-1024 • Double Ratchet • Tor v3',
                  style: TextStyle(fontSize: 10, color: Colors.greenAccent),
                ),
              ],
            ),
          ],
        ),
        actions: [
          PopupMenuButton<int>(
            key: const ValueKey('disappearing_timer_menu'),
            icon: Icon(
              _disappearingTimerSeconds > 0 ? Icons.timer_rounded : Icons.timer_outlined,
              color: _disappearingTimerSeconds > 0 ? Colors.amberAccent : Colors.white70,
            ),
            tooltip: 'Disappearing Messages',
            onSelected: (int seconds) async {
              setState(() {
                _disappearingTimerSeconds = seconds;
              });
              await widget.onSetDisappearingTimer?.call(seconds);
              if (mounted) {
                final label = seconds == 0 ? 'off' : _formatTimerDuration(seconds);
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(
                    content: Text('Disappearing messages set to $label'),
                    duration: const Duration(seconds: 2),
                  ),
                );
              }
            },
            itemBuilder: (BuildContext context) => <PopupMenuEntry<int>>[
              const PopupMenuItem<int>(
                value: 0,
                child: Text('Off'),
              ),
              const PopupMenuItem<int>(
                value: 300,
                child: Text('5 minutes'),
              ),
              const PopupMenuItem<int>(
                value: 3600,
                child: Text('1 hour'),
              ),
              const PopupMenuItem<int>(
                value: 86400,
                child: Text('24 hours'),
              ),
              const PopupMenuItem<int>(
                value: 604800,
                child: Text('7 days'),
              ),
            ],
          ),
        ],
      ),
      body: Column(
        children: [
          Expanded(
            child: Stack(
              children: [
                _isLoading
                    ? const Center(child: CircularProgressIndicator())
                    : _messages.isEmpty
                        ? Center(
                        child: SingleChildScrollView(
                          padding: const EdgeInsets.all(28.0),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Container(
                                padding: const EdgeInsets.all(18),
                                decoration: BoxDecoration(
                                  color: const Color(0xFF1E242C),
                                  shape: BoxShape.circle,
                                  boxShadow: [
                                    BoxShadow(
                                      color: Colors.tealAccent.withOpacity(0.1),
                                      blurRadius: 20,
                                      spreadRadius: 2,
                                    ),
                                  ],
                                ),
                                child: const Icon(
                                  Icons.lock_rounded,
                                  size: 44,
                                  color: Colors.tealAccent,
                                ),
                              ),
                              const SizedBox(height: 18),
                              const Text(
                                'Pairwise Onion Channel Established',
                                style: TextStyle(
                                  color: Colors.white,
                                  fontSize: 16,
                                  fontWeight: FontWeight.bold,
                                ),
                                textAlign: TextAlign.center,
                              ),
                              const SizedBox(height: 8),
                              const Text(
                                'All messages are protected with ML-KEM-1024 post-quantum key encapsulation, Double Ratchet forward secrecy, and routed over Tor v3 onion circuits.',
                                textAlign: TextAlign.center,
                                style: TextStyle(
                                  color: Colors.white60,
                                  fontSize: 12,
                                  height: 1.4,
                                ),
                              ),
                              const SizedBox(height: 16),
                              Container(
                                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                                decoration: BoxDecoration(
                                  color: const Color(0xFF161B22),
                                  borderRadius: BorderRadius.circular(10),
                                  border: Border.all(color: Colors.white12),
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    const Icon(Icons.circle, size: 8, color: Colors.greenAccent),
                                    const SizedBox(width: 8),
                                    Flexible(
                                      child: Text(
                                        widget.conversation.sendQueueUri,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: const TextStyle(
                                          color: Colors.greenAccent,
                                          fontSize: 11,
                                          fontFamily: 'monospace',
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                              const SizedBox(height: 16),
                              const Text(
                                'Send a message below to start chatting.',
                                style: TextStyle(color: Colors.white38, fontSize: 12),
                              ),
                            ],
                          ),
                        ),
                      )
                    : Builder(
                        builder: (context) {
                          final timelineItems = groupMessagesByDate(_messages);
                          return ListView.builder(
                            controller: _scrollController,
                            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                            itemCount: timelineItems.length,
                            itemBuilder: (context, index) {
                              final item = timelineItems[index];

                              if (item is DateHeaderItem) {
                                return Center(
                                  child: Container(
                                    margin: const EdgeInsets.symmetric(vertical: 10),
                                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 5),
                                    decoration: BoxDecoration(
                                      color: const Color(0xFF1E2630),
                                      borderRadius: BorderRadius.circular(12),
                                      border: Border.all(color: Colors.white12, width: 0.5),
                                      boxShadow: [
                                        BoxShadow(
                                          color: Colors.black.withOpacity(0.2),
                                          blurRadius: 4,
                                          offset: const Offset(0, 1),
                                        ),
                                      ],
                                    ),
                                    child: Text(
                                      item.label,
                                      style: const TextStyle(
                                        color: Colors.white70,
                                        fontSize: 11,
                                        fontWeight: FontWeight.w600,
                                        letterSpacing: 0.3,
                                      ),
                                    ),
                                  ),
                                );
                              }

                              final bubble = item as MessageBubbleItem;
                              final msg = bubble.message;
                              final isOutbound = msg.direction == 'outbound';

                              final borderRadius = BorderRadius.only(
                                topLeft: const Radius.circular(16),
                                topRight: const Radius.circular(16),
                                bottomLeft: Radius.circular(isOutbound ? 16 : 4),
                                bottomRight: Radius.circular(isOutbound ? 4 : 16),
                              );

                              final parsedBody = parseMessageBody(msg.bodyText);

                              return Align(
                                alignment: isOutbound ? Alignment.centerRight : Alignment.centerLeft,
                                child: Dismissible(
                                  key: Key('msg_${msg.id}'),
                                  direction: DismissDirection.startToEnd,
                                  confirmDismiss: (_) async {
                                    setState(() => _replyingTo = msg);
                                    return false;
                                  },
                                  background: Container(
                                    alignment: Alignment.centerLeft,
                                    padding: const EdgeInsets.only(left: 16),
                                    child: const Icon(Icons.reply_rounded, color: Colors.blueAccent),
                                  ),
                                  child: InkWell(
                                    onLongPress: () => _showMessageActionSheet(context, msg),
                                    borderRadius: borderRadius,
                                    child: Container(
                                      margin: const EdgeInsets.symmetric(vertical: 3),
                                      padding: const EdgeInsets.only(left: 12, right: 12, top: 8, bottom: 6),
                                      constraints: BoxConstraints(
                                        maxWidth: MediaQuery.of(context).size.width * 0.78,
                                      ),
                                      decoration: BoxDecoration(
                                        color: isOutbound ? const Color(0xFF2C6BED) : const Color(0xFF242C37),
                                        borderRadius: borderRadius,
                                        boxShadow: [
                                          BoxShadow(
                                            color: Colors.black.withOpacity(0.15),
                                            blurRadius: 3,
                                            offset: const Offset(0, 1),
                                          ),
                                        ],
                                      ),
                                      child: Column(
                                        crossAxisAlignment: isOutbound ? CrossAxisAlignment.end : CrossAxisAlignment.start,
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          if (parsedBody.quotedSnippet != null)
                                            Container(
                                              margin: const EdgeInsets.only(bottom: 6),
                                              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                                              decoration: BoxDecoration(
                                                color: isOutbound ? Colors.black.withOpacity(0.2) : Colors.black.withOpacity(0.25),
                                                borderRadius: BorderRadius.circular(6),
                                                border: Border(
                                                  left: BorderSide(
                                                    color: isOutbound ? Colors.white70 : const Color(0xFF2C6BED),
                                                    width: 3,
                                                  ),
                                                ),
                                              ),
                                              child: Text(
                                                parsedBody.quotedSnippet!,
                                                maxLines: 2,
                                                overflow: TextOverflow.ellipsis,
                                                style: const TextStyle(
                                                  color: Colors.white70,
                                                  fontSize: 11,
                                                  fontStyle: FontStyle.italic,
                                                ),
                                              ),
                                            ),
                                          Text(
                                            parsedBody.mainText,
                                            style: const TextStyle(
                                              color: Colors.white,
                                              fontSize: 14,
                                              height: 1.3,
                                            ),
                                          ),
                                          const SizedBox(height: 3),
                                          Row(
                                            mainAxisSize: MainAxisSize.min,
                                            children: [
                                              if (bubble.message.expiresAt != null) ...[
                                                Icon(
                                                  Icons.timer_outlined,
                                                  key: const ValueKey('msg_timer_icon'),
                                                  size: 11,
                                                  color: isOutbound ? Colors.white60 : Colors.white38,
                                                ),
                                                const SizedBox(width: 3),
                                              ],
                                              Text(
                                                bubble.formattedTime,
                                                style: TextStyle(
                                                  color: isOutbound ? Colors.white70 : Colors.white54,
                                                  fontSize: 10,
                                                  fontWeight: FontWeight.w400,
                                                ),
                                              ),
                                              if (isOutbound) ...[
                                                const SizedBox(width: 4),
                                                _buildStatusTick(bubble.message.status),
                                              ],
                                            ],
                                          ),
                                        ],
                                      ),
                                    ),
                                  ),
                                ),
                              );
                            },
                          );
                        },
                      ),
                if (_showScrollToBottom)
                  Positioned(
                    right: 16,
                    bottom: 16,
                    child: GestureDetector(
                      key: const ValueKey('scroll_to_bottom_fab'),
                      onTap: _scrollToBottom,
                      child: Container(
                        width: 44,
                        height: 44,
                        decoration: BoxDecoration(
                          color: const Color(0xFF1E2630),
                          shape: BoxShape.circle,
                          border: Border.all(color: Colors.white24, width: 1),
                          boxShadow: [
                            BoxShadow(
                              color: Colors.black.withOpacity(0.35),
                              blurRadius: 8,
                              offset: const Offset(0, 3),
                            ),
                          ],
                        ),
                        child: Stack(
                          alignment: Alignment.center,
                          clipBehavior: Clip.none,
                          children: [
                            const Icon(
                              Icons.keyboard_arrow_down_rounded,
                              color: Colors.white,
                              size: 28,
                            ),
                            if (_unreadWhileScrolled > 0)
                              Positioned(
                                top: -3,
                                right: -3,
                                child: Container(
                                  padding: const EdgeInsets.all(4),
                                  decoration: const BoxDecoration(
                                    color: Colors.greenAccent,
                                    shape: BoxShape.circle,
                                  ),
                                  constraints: const BoxConstraints(
                                    minWidth: 18,
                                    minHeight: 18,
                                  ),
                                  child: Center(
                                    child: Text(
                                      '$_unreadWhileScrolled',
                                      style: const TextStyle(
                                        color: Colors.black,
                                        fontSize: 10,
                                        fontWeight: FontWeight.bold,
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
          if (_replyingTo != null)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              color: const Color(0xFF161C24),
              child: Row(
                children: [
                  Container(
                    width: 4,
                    height: 36,
                    decoration: BoxDecoration(
                      color: const Color(0xFF2C6BED),
                      borderRadius: BorderRadius.circular(2),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          _replyingTo!.direction == 'outbound' ? 'You' : 'Anonymous Peer',
                          style: const TextStyle(
                            color: Color(0xFF2C6BED),
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          _replyingTo!.bodyText,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(color: Colors.white70, fontSize: 13),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    key: const ValueKey('cancel_reply_btn'),
                    icon: const Icon(Icons.close_rounded, size: 20, color: Colors.white60),
                    onPressed: () => setState(() => _replyingTo = null),
                  ),
                ],
              ),
            ),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            color: const Color(0xFF1A2028),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _msgController,
                    style: const TextStyle(color: Colors.white),
                    decoration: InputDecoration(
                      hintText: 'Message via onion circuit...',
                      hintStyle: const TextStyle(color: Colors.white38),
                      filled: true,
                      fillColor: const Color(0xFF262D38),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(24),
                        borderSide: BorderSide.none,
                      ),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                    ),
                    onSubmitted: (_) => _sendMessage(),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  icon: const Icon(Icons.send_rounded, color: Color(0xFF2C6BED)),
                  onPressed: _sendMessage,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
