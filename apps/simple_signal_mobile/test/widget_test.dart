import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/main.dart';
import 'package:simple_signal_mobile/src/screens/qr_screen.dart';

void main() {
  testWidgets('AnonTrace app smoke test loads LockScreen', (WidgetTester tester) async {
    await tester.pumpWidget(const SimpleSignalAppWidget());

    // Verify LockScreen is presented with AnonTrace branding
    expect(find.text('AnonTrace'), findsOneWidget);
    expect(find.text('Zero Identifiers • PQXDH • Tor Onion Routing'), findsOneWidget);
  });

  testWidgets('QrContactScreen renders contact URI without throwing', (WidgetTester tester) async {
    final compactUri =
        'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_alice_temp_99&auth=${'77' * 32}&ik=${'11' * 32}&idhk=${'22' * 32}&spkid=1&spk=${'33' * 32}&sig=${'44' * 64}&otpkid=100&otpk=${'55' * 32}';
    await tester.pumpWidget(
      MaterialApp(
        home: QrContactScreen(
          contactUri: compactUri,
          onContactScanned: (_) {},
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Verify QR screen renders TabBar and action button
    expect(find.text('My QR Code'), findsOneWidget);
    expect(find.text('Copy Contact Link'), findsOneWidget);
  });
}
