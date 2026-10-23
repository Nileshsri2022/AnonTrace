import 'dart:async';
import 'bridge.dart';

SimpleSignalBridge getPlatformBridge() => WebSimpleSignalBridge();

class WebSimpleSignalBridge implements SimpleSignalBridge {
  final List<ConversationSummary> _conversations = [];
  final Map<String, List<MessageSummary>> _messages = {};
  String _contactLink = '';

  @override
  Future<void> initApp({required String dbPath, required String masterPin}) async {
    await Future.delayed(const Duration(milliseconds: 100));
    final randomSuffix = DateTime.now().millisecondsSinceEpoch % 100000;
    _contactLink =
        'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_anon_$randomSuffix';
  }

  @override
  Future<String> getMyContactLink() async {
    return _contactLink;
  }

  @override
  Future<ConversationSummary> importContactLink(String uri) async {
    final newConv = ConversationSummary(
      id: 'conv_web_${DateTime.now().millisecondsSinceEpoch % 1000}',
      sendQueueUri: uri,
      receiveQueueId: 'q_web_${DateTime.now().millisecondsSinceEpoch}',
      createdAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
    _conversations.insert(0, newConv);
    return newConv;
  }

  @override
  Future<MessageSummary> sendChatMessage({
    required String conversationId,
    required String text,
  }) async {
    final msg = MessageSummary(
      id: 'msg_${DateTime.now().millisecondsSinceEpoch}',
      conversationId: conversationId,
      direction: 'outbound',
      bodyText: text,
      timestamp: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      status: 'sent',
    );
    _messages.putIfAbsent(conversationId, () => []).add(msg);
    return msg;
  }

  @override
  Future<List<ConversationSummary>> listConversations() async {
    return List.unmodifiable(_conversations);
  }

  @override
  Future<List<MessageSummary>> listMessages(String conversationId) async {
    return List.unmodifiable(_messages[conversationId] ?? []);
  }

  @override
  Future<void> setDormant(bool dormant) async {}

  @override
  Future<bool> isActive() async => true;
}
