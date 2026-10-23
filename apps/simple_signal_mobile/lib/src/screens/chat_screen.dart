import 'dart:async';
import 'package:flutter/material.dart';
import '../ffi/bridge.dart';

class ChatScreen extends StatefulWidget {
  final ConversationSummary conversation;
  final Future<List<MessageSummary>> Function(String convId) onFetchMessages;
  final Future<MessageSummary> Function(String convId, String text) onSendMessage;
  final Future<List<MessageSummary>> Function(String convId)? onPollMessages;

  const ChatScreen({
    Key? key,
    required this.conversation,
    required this.onFetchMessages,
    required this.onSendMessage,
    this.onPollMessages,
  }) : super(key: key);

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final TextEditingController _msgController = TextEditingController();
  final List<MessageSummary> _messages = [];
  bool _isLoading = true;
  Timer? _pollTimer;

  @override
  void initState() {
    super.initState();
    _loadMessages();
    _startPolling();
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _msgController.dispose();
    super.dispose();
  }

  void _startPolling() {
    _pollTimer = Timer.periodic(const Duration(milliseconds: 1500), (_) async {
      if (!mounted || widget.onPollMessages == null) return;
      try {
        final incoming = await widget.onPollMessages!(widget.conversation.id);
        if (incoming.isNotEmpty && mounted) {
          setState(() {
            for (final msg in incoming) {
              if (!_messages.any((m) => m.id == msg.id)) {
                _messages.add(msg);
              }
            }
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
                    : ListView.builder(
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                        itemCount: _messages.length,
                        itemBuilder: (context, index) {
                          final msg = _messages[index];
                          final isOutbound = msg.direction == 'outbound';
                          return Align(
                            alignment: isOutbound ? Alignment.centerRight : Alignment.centerLeft,
                            child: Container(
                              margin: const EdgeInsets.symmetric(vertical: 4),
                              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                              constraints: BoxConstraints(
                                maxWidth: MediaQuery.of(context).size.width * 0.75,
                              ),
                              decoration: BoxDecoration(
                                color: isOutbound ? const Color(0xFF2C6BED) : const Color(0xFF2A313C),
                                borderRadius: BorderRadius.circular(16),
                              ),
                              child: Text(
                                msg.bodyText,
                                style: const TextStyle(color: Colors.white, fontSize: 14),
                              ),
                            ),
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
