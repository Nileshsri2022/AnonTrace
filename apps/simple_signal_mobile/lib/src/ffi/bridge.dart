import 'dart:async';

class ConversationSummary {
  final String id;
  final String sendQueueUri;
  final String receiveQueueId;
  final int createdAt;

  ConversationSummary({
    required this.id,
    required this.sendQueueUri,
    required this.receiveQueueId,
    required this.createdAt,
  });

  factory ConversationSummary.fromJson(Map<String, dynamic> json) {
    return ConversationSummary(
      id: json['id'] as String,
      sendQueueUri: json['send_queue_uri'] as String,
      receiveQueueId: json['receive_queue_id'] as String,
      createdAt: json['created_at'] as int,
    );
  }
}

class MessageSummary {
  final String id;
  final String conversationId;
  final String direction;
  final String bodyText;
  final int timestamp;
  final String status;

  MessageSummary({
    required this.id,
    required this.conversationId,
    required this.direction,
    required this.bodyText,
    required this.timestamp,
    required this.status,
  });

  factory MessageSummary.fromJson(Map<String, dynamic> json) {
    return MessageSummary(
      id: json['id'] as String,
      conversationId: json['conversation_id'] as String,
      direction: json['direction'] as String,
      bodyText: json['body_text'] as String,
      timestamp: json['timestamp'] as int,
      status: json['status'] as String,
    );
  }
}

abstract class SimpleSignalBridge {
  Stream<MessageSummary> get messageStream;
  Future<void> initApp({required String dbPath, required String masterPin});
  Future<String> getMyContactLink();
  Future<String?> getMyInboxQueueId();
  Future<ConversationSummary> importContactLink(String uri);
  Future<MessageSummary> sendChatMessage({required String conversationId, required String text});
  Future<List<ConversationSummary>> listConversations();
  Future<List<MessageSummary>> listMessages(String conversationId);
  Future<bool> deleteMessage(String messageId);
  Future<List<MessageSummary>> receiveMessages({String? conversationId});
  Future<void> setDormant(bool dormant);
  Future<bool> isActive();
}
