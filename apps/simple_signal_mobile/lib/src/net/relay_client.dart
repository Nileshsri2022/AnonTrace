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

/// Private-relay HTTPS transport client (Option B, Normal Mode).
/// Transmits and polls end-to-end encrypted post-quantum ciphertexts
/// across blind queues. The default relay is a deployment placeholder:
/// the operator must point `relay.anontrace.app` at a running
/// anontrace-relay behind TLS on port 443. Do NOT fall back to a
/// public relay (e.g. ntfy.sh) — a failed send must surface as a
/// failure, not a silent privacy downgrade.
class RelayClient {
  final String baseUrl;
  final HttpClient _client;
  bool _isClosed = false;

  RelayClient({
    this.baseUrl = 'https://relay.anontrace.app',
    HttpClient? client,
  }) : _client = client ?? HttpClient() {
    _client.connectionTimeout = const Duration(seconds: 10);
  }

  /// Sends an encrypted payload into a recipient's blind queue over HTTPS.
  /// `senderAuthHex` is the partner's sender auth key from their contact
  /// link; the relay rejects posts without it.
  Future<bool> sendPayload({
    required String queueId,
    required String payloadHex,
    String? senderAuthHex,
  }) async {
    try {
      final uri = Uri.parse('$baseUrl/q/$queueId');
      final request = await _client.postUrl(uri);
      request.persistentConnection = true;
      request.headers.set(HttpHeaders.contentTypeHeader, 'text/plain; charset=utf-8');
      if (senderAuthHex != null && senderAuthHex.isNotEmpty) {
        request.headers.set('X-Sender-Auth', senderAuthHex);
      }
      request.write(payloadHex);
      final response = await request.close();
      await response.drain();
      return response.statusCode == 200;
    } catch (e) {
      return false;
    }
  }

  /// Acknowledges (deletes) a message on the private relay after it
  /// has been processed. Zero retention: un-ACKed messages are kept
  /// only until the relay TTL backstop.
  Future<bool> ackMessage({
    required String queueId,
    required String messageId,
  }) async {
    try {
      final uri = Uri.parse('$baseUrl/q/$queueId/ack/$messageId');
      final request = await _client.postUrl(uri);
      final response = await request.close();
      await response.drain();
      return response.statusCode == 200;
    } catch (e) {
      return false;
    }
  }

  /// Listens continuously to an incoming queue using Server-Sent Events (SSE) / HTTP chunked stream.
  /// Yields messages in real-time (< 50ms) as they are pushed by the relay server.
  Stream<RelayMessage> listenToQueue({
    required String queueId,
    String? sinceId,
  }) async* {
    while (!_isClosed) {
      try {
        final query = (sinceId != null && sinceId.isNotEmpty)
            ? 'since=$sinceId'
            : 'since=all';
        final uri = Uri.parse('$baseUrl/q/$queueId/json?$query');
        final request = await _client.getUrl(uri);
        request.persistentConnection = true;
        request.headers.set(HttpHeaders.acceptHeader, 'application/x-ndjson, text/event-stream, */*');
        final response = await request.close();

        if (response.statusCode != 200) {
          await response.drain();
          if (_isClosed) break;
          await Future.delayed(const Duration(seconds: 2));
          continue;
        }

        final lines = response
            .transform(utf8.decoder)
            .transform(const LineSplitter());

        await for (final line in lines) {
          if (_isClosed) break;
          final trimmed = line.trim();
          if (trimmed.isEmpty) continue;
          try {
            final json = jsonDecode(trimmed) as Map<String, dynamic>;
            if (json['event'] == 'message' && json['message'] != null) {
              final msgId = json['id'] as String;
              sinceId = msgId;
              yield RelayMessage(
                id: msgId,
                payloadHex: json['message'] as String,
                timestamp: (json['time'] as int?) ?? DateTime.now().millisecondsSinceEpoch ~/ 1000,
              );
            }
          } catch (_) {}
        }
      } catch (_) {
        if (_isClosed) break;
        await Future.delayed(const Duration(seconds: 2));
      }
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
      final uri = Uri.parse('$baseUrl/q/$queueId/json?$query');
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
    _isClosed = true;
    _client.close(force: true);
  }
}
