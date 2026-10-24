import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/net/relay_client.dart';

void main() {
  test('RelayClient sends and polls payload from queue', () async {
    final client = RelayClient(baseUrl: 'https://ntfy.sh');
    final testQueueId = 'anontrace_test_tdd_${DateTime.now().millisecondsSinceEpoch}';
    final testPayload = '01deadbeefcafebabe';

    // 1. Send payload to queue
    final sendSuccess = await client.sendPayload(
      queueId: testQueueId,
      payloadHex: testPayload,
    );
    expect(sendSuccess, isTrue);

    // Wait a brief moment for public relay cache flush
    await Future.delayed(const Duration(milliseconds: 300));

    // 2. Poll payload from queue
    final messages = await client.pollQueue(queueId: testQueueId);
    expect(messages.isNotEmpty, isTrue);
    expect(messages.first.payloadHex, equals(testPayload));
  });
}
