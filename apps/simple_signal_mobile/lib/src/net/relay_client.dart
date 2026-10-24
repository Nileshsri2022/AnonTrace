import 'dart:convert';
import 'dart:io';

class RelayMessage {
  final String id;
  final String payloadHex;
  final int timestamp;

  RelayMessage({
    required this.id,
    required this.payloadHex,
    required this.timestamp,
  });
}

/// SimpleX-compatible HTTPS relay queue transport client.
/// Transmits and polls end-to-end encrypted post-quantum ciphertexts across unidirectional queues.
class RelayClient {
  final String baseUrl;
  final HttpClient _client;

  RelayClient({
    this.baseUrl = 'https://ntfy.sh',
    HttpClient? client,
  }) : _client = client ?? HttpClient() {
    _client.connectionTimeout = const Duration(seconds: 10);
  }

  /// Sends an encrypted payload into a recipient's queue over HTTPS.
  Future<bool> sendPayload({
    required String queueId,
    required String payloadHex,
  }) async {
    try {
      final uri = Uri.parse('$baseUrl/$queueId');
      final request = await _client.postUrl(uri);
      request.persistentConnection = false;
      request.headers.set(HttpHeaders.contentTypeHeader, 'text/plain; charset=utf-8');
      request.write(payloadHex);
      final response = await request.close();
      final respBody = await response.transform(utf8.decoder).join();
      return response.statusCode == 200;
    } catch (e) {
      return false;
    }
  }

  /// Polls pending messages from an inbox queue over HTTPS.
  Future<List<RelayMessage>> pollQueue({
    required String queueId,
    String? sinceId,
  }) async {
    try {
      final query = (sinceId != null && sinceId.isNotEmpty)
          ? 'poll=1&since=$sinceId'
          : 'poll=1&since=all';
      final uri = Uri.parse('$baseUrl/$queueId/json?$query');
      final request = await _client.getUrl(uri);
      request.persistentConnection = false;
      request.headers.set(HttpHeaders.acceptHeader, '*/*');
      final response = await request.close();
      final body = await response.transform(utf8.decoder).join();
      if (response.statusCode != 200) {
        return [];
      }
      final lines = body.split('\n').where((l) => l.trim().isNotEmpty);
      final List<RelayMessage> messages = [];

      for (final line in lines) {
        try {
          final json = jsonDecode(line) as Map<String, dynamic>;
          if (json['event'] == 'message' && json['message'] != null) {
            messages.add(RelayMessage(
              id: json['id'] as String,
              payloadHex: json['message'] as String,
              timestamp: (json['time'] as int?) ?? DateTime.now().millisecondsSinceEpoch ~/ 1000,
            ));
          }
        } catch (_) {}
      }
      return messages;
    } catch (_) {
      return [];
    }
  }

  void close() {
    _client.close();
  }
}
