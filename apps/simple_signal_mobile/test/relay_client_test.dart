import 'dart:convert';
import 'dart:io';
import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/net/relay_client.dart';

void main() {
  test('RelayClient sends and polls payload from queue via loopback relay', () async {
    final Map<String, List<String>> queueStore = {};

    // Bind local test HTTP server to verify real HTTP socket transmission deterministically
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    server.listen((HttpRequest request) async {
      final pathSegments = request.uri.pathSegments;
      if (pathSegments.isEmpty) {
        request.response.statusCode = 404;
        await request.response.close();
        return;
      }

      final queueId = pathSegments[0];

      if (request.method == 'POST') {
        final body = await utf8.decodeStream(request);
        queueStore.putIfAbsent(queueId, () => []).add(body);
        request.response.statusCode = 200;
        request.response.write('{"id":"msg_${queueStore[queueId]!.length}"}');
        await request.response.close();
      } else if (request.method == 'GET') {
        final messages = queueStore[queueId] ?? [];
        request.response.statusCode = 200;
        request.response.headers.contentType = ContentType('application', 'x-ndjson', charset: 'utf-8');
        for (var i = 0; i < messages.length; i++) {
          final jsonLine = jsonEncode({
            'id': 'msg_$i',
            'time': 1792831427,
            'event': 'message',
            'topic': queueId,
            'message': messages[i],
          });
          request.response.writeln(jsonLine);
        }
        await request.response.close();
      }
    });

    try {
      final client = RelayClient(baseUrl: 'http://${server.address.host}:${server.port}');
      const testQueueId = 'anontrace_test_queue';
      const testPayload = '01deadbeefcafebabe';

      // 1. Send payload to queue
      final sendSuccess = await client.sendPayload(
        queueId: testQueueId,
        payloadHex: testPayload,
      );
      expect(sendSuccess, isTrue);

      // 2. Poll payload from queue
      final messages = await client.pollQueue(queueId: testQueueId);
      expect(messages.isNotEmpty, isTrue);
      expect(messages.first.payloadHex, equals(testPayload));
    } finally {
      await server.close();
    }
  });

  test('RelayClient streams incoming payload in real time via listenToQueue', () async {
    final List<HttpResponse> openStreamResponses = [];

    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    server.listen((HttpRequest request) async {
      final pathSegments = request.uri.pathSegments;
      if (pathSegments.isEmpty) {
        request.response.statusCode = 404;
        await request.response.close();
        return;
      }

      final queueId = pathSegments[0];

      if (request.method == 'POST') {
        final body = await utf8.decodeStream(request);
        request.response.statusCode = 200;
        request.response.write('{"id":"msg_realtime"}');
        await request.response.close();

        // Push directly into open stream response
        for (final res in openStreamResponses) {
          final jsonLine = jsonEncode({
            'id': 'msg_stream_1',
            'time': 1792831427,
            'event': 'message',
            'topic': queueId,
            'message': body,
          });
          res.writeln(jsonLine);
        }
      } else if (request.method == 'GET') {
        request.response.statusCode = 200;
        request.response.headers.contentType = ContentType('application', 'x-ndjson', charset: 'utf-8');
        request.response.bufferOutput = false;
        openStreamResponses.add(request.response);
      }
    });

    try {
      final client = RelayClient(baseUrl: 'http://${server.address.host}:${server.port}');
      const testQueueId = 'stream_test_queue';
      const testPayload = '01cafebabedeadbeef';

      final stream = client.listenToQueue(queueId: testQueueId);
      final receivedFuture = stream.first;

      // Small delay to ensure stream is connected
      await Future.delayed(const Duration(milliseconds: 50));

      // Send payload
      await client.sendPayload(queueId: testQueueId, payloadHex: testPayload);

      // Verify payload is received via stream
      final received = await receivedFuture.timeout(const Duration(seconds: 3));
      expect(received.payloadHex, equals(testPayload));
      client.close();
    } finally {
      for (final res in openStreamResponses) {
        try {
          await res.close();
        } catch (_) {}
      }
      await server.close();
    }
  });
}
