import 'dart:async';
import 'package:flutter/material.dart';
import '../ffi/bridge.dart';
import '../utils/timeline_adapter.dart';

class ChatScreen extends StatefulWidget {
  final ConversationSummary conversation;
  final Future<List<MessageSummary>> Function(String convId) onFetchMessages;
  final Future<MessageSummary> Function(String convId, String text) onSendMessage;
  final Future<List<MessageSummary>> Function(String convId)? onPollMessages;
  final Stream<MessageSummary>? messageStream;

  const ChatScreen({
    Key? key,
    required this.conversation,
    required this.onFetchMessages,
    required this.onSendMessage,
    this.onPollMessages,
    this.messageStream,
  }) : super(key: key);

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final TextEditingController _msgController = TextEditingController();
  final List<MessageSummary> _messages = [];
  bool _isLoading = true;
  Timer? _pollTimer;
  StreamSubscription<MessageSummary>? _streamSub;

  @override
  void initState() {
    super.initState();
    _loadMessages();
    _subscribeToStream();
    _startPolling();
  }

  @override
  void dispose() {
    _streamSub?.cancel();
    _pollTimer?.cancel();
    _msgController.dispose();
    super.dispose();
  }

  void _subscribeToStream() {
    if (widget.messageStream != null) {
      _streamSub = widget.messageStream!.listen((msg) {
        if (msg.conversationId == widget.conversation.id && mounted) {
          setState(() {
            if (!_messages.any((m) => m.id == msg.id)) {
              _messages.add(msg);
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
      } catch (_) {}
    });
  }

  Future<void> _loadMessages() async {
    try {
      final msgs = await widget.onFetchMessages(widget.conversation.id);
      setState(() {
        _messages.clear();
        _messages.addAll(msgs);
        _isLoading = false;
      });
    } catch (_) {
      setState(() => _isLoading = false);
    }
  }

  Future<void> _sendMessage() async {
    final text = _msgController.text.trim();
    if (text.isEmpty) return;

    _msgController.clear();
    try {
      final sent = await widget.onSendMessage(widget.conversation.id, text);
      setState(() {
        _messages.add(sent);
      });
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Send failed: $e')),
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
              children: const [
                Icon(Icons.lock_outline, size: 12, color: Colors.greenAccent),
                SizedBox(width: 4),
                Text(
                  'ML-KEM-1024 • Double Ratchet • Tor v3',
                  style: TextStyle(fontSize: 10, color: Colors.greenAccent),
                ),
              ],
            ),
          ],
        ),
      ),
      body: Column(
        children: [
          Expanded(
            child: _isLoading
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

                              return Align(
                                alignment: isOutbound ? Alignment.centerRight : Alignment.centerLeft,
                                child: Container(
                                  margin: const EdgeInsets.symmetric(vertical: 3),
                                  padding: const EdgeInsets.only(left: 12, right: 12, top: 8, bottom: 6),
                                  constraints: BoxConstraints(
                                    maxWidth: MediaQuery.of(context).size.width * 0.78,
                                  ),
                                  decoration: BoxDecoration(
                                    color: isOutbound ? const Color(0xFF2C6BED) : const Color(0xFF242C37),
                                    borderRadius: BorderRadius.only(
                                      topLeft: const Radius.circular(16),
                                      topRight: const Radius.circular(16),
                                      bottomLeft: Radius.circular(isOutbound ? 16 : 4),
                                      bottomRight: Radius.circular(isOutbound ? 4 : 16),
                                    ),
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
                                      Text(
                                        msg.bodyText,
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
                                            const Icon(
                                              Icons.done,
                                              size: 13,
                                              color: Colors.white70,
                                            ),
                                          ],
                                        ],
                                      ),
                                    ],
                                  ),
                                ),
                              );
                            },
                          );
                        },
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
