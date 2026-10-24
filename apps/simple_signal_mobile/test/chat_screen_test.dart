import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/ffi/bridge.dart';
import 'package:simple_signal_mobile/src/screens/chat_screen.dart';

void main() {
  testWidgets('ChatScreen displays DateHeaderChip and formatted message timestamps', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_123',
      sendQueueUri: 'https://ntfy.sh/q_out_123',
      receiveQueueId: 'q_in_123',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    final msg = MessageSummary(
      id: 'm1',
      conversationId: 'conv_123',
      direction: 'outbound',
      bodyText: 'Secured with ML-KEM and Double Ratchet',
      timestamp: now.millisecondsSinceEpoch ~/ 1000,
      status: 'sent',
    );

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          onFetchMessages: (_) async => [msg],
          onSendMessage: (_, __) async => msg,
        ),
      ),
    );

    // Initial load
    await tester.pumpAndSettle();

    // Verify message text is rendered
    expect(find.text('Secured with ML-KEM and Double Ratchet'), findsOneWidget);

    // Verify Date Header Chip "Today" is rendered
    expect(find.text('Today'), findsOneWidget);

    // Verify outbound check icon is rendered
    expect(find.byIcon(Icons.done), findsOneWidget);
  });

  testWidgets('ChatScreen long-press on message opens action sheet with Copy and Delete options', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_123',
      sendQueueUri: 'https://ntfy.sh/q_out_123',
      receiveQueueId: 'q_in_123',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    final msg = MessageSummary(
      id: 'm1',
      conversationId: 'conv_123',
      direction: 'inbound',
      bodyText: 'Top secret communication',
      timestamp: now.millisecondsSinceEpoch ~/ 1000,
      status: 'received',
    );

    bool deleteCalled = false;

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          onFetchMessages: (_) async => [msg],
          onSendMessage: (_, __) async => msg,
          onDeleteMessage: (msgId) async {
            deleteCalled = true;
            return true;
          },
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Long press on the message text
    await tester.longPress(find.text('Top secret communication'));
    await tester.pumpAndSettle();

    // Verify modal bottom sheet options are visible
    expect(find.text('Copy Text'), findsOneWidget);
    expect(find.text('Delete for Me'), findsOneWidget);

    // Tap Delete for Me
    await tester.tap(find.text('Delete for Me'));
    await tester.pumpAndSettle();

    // Verify confirmation dialog
    expect(find.text('Delete Message?'), findsOneWidget);
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(deleteCalled, isTrue);
    expect(find.text('Top secret communication'), findsNothing);
  });
}
