import 'dart:async';
import '../logging/app_logger.dart';
import 'bridge.dart';

SimpleSignalBridge getPlatformBridge() => WebSimpleSignalBridge();

class WebSimpleSignalBridge implements SimpleSignalBridge {
  final StreamController<MessageSummary> _stubStreamController = StreamController<MessageSummary>.broadcast();
  final List<ConversationSummary> _conversations = [];
  final Map<String, List<MessageSummary>> _messages = {};
  final Map<String, int> _disappearingTimers = {};
  String _contactLink = '';

  @override
  Stream<MessageSummary> get messageStream => _stubStreamController.stream;

  @override
  Future<void> initApp({required String dbPath, required String masterPin}) async {
    AppLogger.info('WEB_BRIDGE', 'Initializing web in-memory bridge (sandboxed)');
    await Future.delayed(const Duration(milliseconds: 100));
    final randomSuffix = DateTime.now().millisecondsSinceEpoch % 100000;
    _contactLink =
        'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_web_$randomSuffix&auth=1111111111111111111111111111111111111111111111111111111111111111&ik=5696058f86bd9b645b49ec1a996ca25f45ee70a5ed02f98e6f3390857d06acac&idhk=c446bd64a0b22a5c4576ca6ea83f3331c1c6eccc6e811c2773d67ac987b6b465&spkid=1&spk=0c340b19acef708d94106244aba6d02948ba34fb97a2a94dcb3f8521e29f3e1f&sig=e4ddb33ddfefeaebc622f6171f1ce4f7bcb1e7979e814a3caece10b2ca9e7cc04dc210201416a9c4443913a6973ba899ca5d7f2b9e49f8cec77ca611edb1cc0d&otpkid=100&otpk=3040c59ce434f11046eb34b658c68e17a9fc6be6693b556bf7d76e036bf8c95b';
  }

  @override
  Future<String> getMyContactLink() async {
    AppLogger.info('WEB_BRIDGE', 'getMyContactLink requested');
    return _contactLink;
  }

  @override
  Future<String?> getMyInboxQueueId() async => 'anontrace_inbox_stub';

  @override
  Future<ConversationSummary> importContactLink(String uri) async {
    AppLogger.info('WEB_BRIDGE', 'importContactLink: $uri');
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
    AppLogger.info('WEB_BRIDGE', 'sendChatMessage to $conversationId');
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final timer = _disappearingTimers[conversationId] ?? 0;
    final msg = MessageSummary(
      id: 'msg_${DateTime.now().millisecondsSinceEpoch}',
      conversationId: conversationId,
      direction: 'outbound',
      bodyText: text,
      timestamp: now,
      status: 'sent',
      expiresAt: timer > 0 ? now + timer : null,
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
    await purgeExpiredMessages();
    return List.unmodifiable(_messages[conversationId] ?? []);
  }

  @override
  Future<bool> deleteMessage(String messageId) async {
    for (final list in _messages.values) {
      final idx = list.indexWhere((m) => m.id == messageId);
      if (idx != -1) {
        list.removeAt(idx);
        return true;
      }
    }
    return false;
  }

  @override
  Future<bool> updateMessageStatus({required String messageId, required String status}) async {
    for (final list in _messages.values) {
      final idx = list.indexWhere((m) => m.id == messageId);
      if (idx != -1) {
        final old = list[idx];
        list[idx] = MessageSummary(
          id: old.id,
          conversationId: old.conversationId,
          direction: old.direction,
          bodyText: old.bodyText,
          timestamp: old.timestamp,
          status: status,
          expiresAt: old.expiresAt,
        );
        return true;
      }
    }
    return false;
  }

  @override
  Future<bool> setDisappearingTimer({required String conversationId, required int durationSeconds}) async {
    _disappearingTimers[conversationId] = durationSeconds;
    return true;
  }

  @override
  Future<int> purgeExpiredMessages() async {
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    int purged = 0;
    for (final list in _messages.values) {
      final initialLen = list.length;
      list.removeWhere((m) => m.expiresAt != null && m.expiresAt! <= now);
      purged += (initialLen - list.length);
    }
    return purged;
  }

  @override
  Future<List<MessageSummary>> receiveMessages({String? conversationId}) async {
    return [];
  }

  @override
  Future<void> setDormant(bool dormant) async {}

  @override
  Future<bool> isActive() async => false;

  @override
  Future<bool> setTransportMode(String mode) async => false;
  @override
  Future<String?> getTransportMode() async => 'normal';
  @override
  Future<bool> startStealthBootstrap({required String dataDir, String? onionAddress, String? bridgeLines}) async => false;
  @override
  Map<String, dynamic>? getStealthStatus() => null;
  @override
  Future<bool> stealthSend({required String queueId, required String senderAuthHex, required String payloadHex}) async => false;
}
