import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/logging/log_scrubber.dart';

void main() {
  group('LogScrubber', () {
    test('redacts master PIN in text', () {
      const input = 'Initializing with masterPin: 123456 and dbPath: /data/anontrace.db';
      final output = LogScrubber.scrub(input);
      expect(output.contains('123456'), isFalse);
      expect(output.contains('[REDACTED_PIN]'), isTrue);
      expect(output.contains('/data/anontrace.db'), isTrue);
    });

    test('redacts contact link cryptographic prekeys and auth tokens', () {
      const input =
          'Importing contact link: simplex:/contact#/?v=1&relay=test.onion&queue=q1&auth=11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff&ik=5696058f86bd9b645b49ec1a996ca25f45ee70a5ed02f98e6f3390857d06acac&kpk=deadbeef';
      final output = LogScrubber.scrub(input);
      expect(output.contains('11223344556677889900aabbccddeeff'), isFalse);
      expect(output.contains('5696058f86bd9b645b49ec1a996ca25f'), isFalse);
      expect(output.contains('auth=[REDACTED]'), isTrue);
      expect(output.contains('ik=[REDACTED]'), isTrue);
      expect(output.contains('relay=test.onion'), isTrue);
    });

    test('preserves benign diagnostic exceptions and stack traces', () {
      const input = 'ProcessException: Process exited abnormally (exit code 1) in line 42';
      final output = LogScrubber.scrub(input);
      expect(output, equals(input));
    });
  });
}
