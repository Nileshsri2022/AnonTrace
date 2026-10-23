import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/logging/app_logger.dart';

void main() {
  setUp(() {
    AppLogger.clear();
  });

  group('AppLogger', () {
    test('records info and error log entries with scrubbing', () {
      AppLogger.info('AUTH', 'User attempted login with pin: 9999');
      AppLogger.error('NET', 'Connection timeout to relay');

      final logs = AppLogger.getLogs();
      expect(logs.length, equals(2));
      expect(logs[0].message.contains('9999'), isFalse);
      expect(logs[0].message.contains('[REDACTED_PIN]'), isTrue);
      expect(logs[1].tag, equals('NET'));
      expect(logs[1].isError, isTrue);
    });

    test('enforces rolling buffer limit', () {
      for (var i = 0; i < 600; i++) {
        AppLogger.info('TEST', 'Message number $i');
      }

      final logs = AppLogger.getLogs();
      expect(logs.length, equals(500));
      expect(logs.first.message, equals('Message number 100'));
      expect(logs.last.message, equals('Message number 599'));
    });

    test('formats logs as readable export text', () {
      AppLogger.info('INIT', 'App initialized');
      AppLogger.error('SQL', 'DB lock', 'LockedException', StackTrace.empty);

      final exported = AppLogger.exportLogsAsText();
      expect(exported.contains('[INIT]'), isTrue);
      expect(exported.contains('App initialized'), isTrue);
      expect(exported.contains('[SQL] ERROR: DB lock'), isTrue);
      expect(exported.contains('LockedException'), isTrue);
    });

    test('filters logs by error level', () {
      AppLogger.info('A', 'Info 1');
      AppLogger.error('B', 'Error 1');
      AppLogger.info('C', 'Info 2');

      final errorOnly = AppLogger.getLogs(errorsOnly: true);
      expect(errorOnly.length, equals(1));
      expect(errorOnly.first.tag, equals('B'));
    });
  });
}
