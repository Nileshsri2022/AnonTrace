import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/logging/app_logger.dart';
import 'package:simple_signal_mobile/src/screens/debug_log_screen.dart';

void main() {
  setUp(() {
    AppLogger.clear();
  });

  testWidgets('DebugLogScreen displays recorded logs and privacy banner', (WidgetTester tester) async {
    AppLogger.info('BOOT', 'App starting up');
    AppLogger.error('NET', 'Failed connection');

    await tester.pumpWidget(
      const MaterialApp(
        home: DebugLogScreen(),
      ),
    );

    // Verify privacy notice is visible
    expect(find.textContaining('Zero-Knowledge'), findsOneWidget);

    // Verify logs are rendered
    expect(find.textContaining('App starting up'), findsOneWidget);
    expect(find.textContaining('Failed connection'), findsOneWidget);
  });

  testWidgets('DebugLogScreen filters logs by category chip', (WidgetTester tester) async {
    AppLogger.tor('Tor circuit built');
    AppLogger.ffi('simple_signal_init', 'Bridge initialized');
    AppLogger.error('CORE', 'Fatal crash averted');

    await tester.pumpWidget(
      const MaterialApp(
        home: DebugLogScreen(),
      ),
    );

    expect(find.textContaining('Tor circuit built'), findsOneWidget);
    expect(find.textContaining('Fatal crash averted'), findsOneWidget);

    // Tap "Errors" chip
    final errorChip = find.widgetWithText(ChoiceChip, 'Errors');
    expect(errorChip, findsOneWidget);
    await tester.tap(errorChip);
    await tester.pumpAndSettle();

    // Now only error log should be present
    expect(find.textContaining('Fatal crash averted'), findsOneWidget);
    expect(find.textContaining('Tor circuit built'), findsNothing);
  });

  testWidgets('DebugLogScreen copies logs to clipboard', (WidgetTester tester) async {
    final List<MethodCall> log = [];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (MethodCall methodCall) async {
      log.add(methodCall);
      return null;
    });

    AppLogger.info('DIAG', 'Copyable secret-scrubbed line');

    await tester.pumpWidget(
      const MaterialApp(
        home: DebugLogScreen(),
      ),
    );

    final copyButton = find.byTooltip('Copy Logs');
    expect(copyButton, findsOneWidget);
    await tester.tap(copyButton);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    // Verify clipboard call was made
    expect(
      log.any((call) => call.method == 'Clipboard.setData'),
      isTrue,
    );

    // Verify snackbar is shown
    expect(find.textContaining('copied to clipboard'), findsOneWidget);
    await tester.pumpAndSettle();
  });
}
