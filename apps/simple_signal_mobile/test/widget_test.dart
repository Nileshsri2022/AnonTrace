import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/main.dart';

void main() {
  testWidgets('SimpleSignalApp smoke test', (WidgetTester tester) async {
    await tester.pumpWidget(const SimpleSignalAppWidget());
    expect(find.text('Simple + Signal'), findsOneWidget);
  });
}
