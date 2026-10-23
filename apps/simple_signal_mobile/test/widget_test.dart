import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/main.dart';

void main() {
  testWidgets('AnonTrace app smoke test loads LockScreen', (WidgetTester tester) async {
    await tester.pumpWidget(const SimpleSignalAppWidget());

    // Verify LockScreen is presented with AnonTrace branding
    expect(find.text('AnonTrace'), findsOneWidget);
    expect(find.text('Zero Identifiers • PQXDH • Tor Onion Routing'), findsOneWidget);
  });
}
