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

  testWidgets('ChatScreen shows Scroll-to-Bottom FAB when scrolled up and tapping scrolls down', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_scroll',
      sendQueueUri: 'https://ntfy.sh/q_scroll',
      receiveQueueId: 'q_scroll_in',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    // Generate 30 messages to produce a scrollable list
    final messages = List.generate(
      30,
      (i) => MessageSummary(
        id: 'msg_$i',
        conversationId: 'conv_scroll',
        direction: i % 2 == 0 ? 'inbound' : 'outbound',
        bodyText: 'Message $i content in conversation timeline',
        timestamp: (now.millisecondsSinceEpoch ~/ 1000) + i,
        status: 'sent',
      ),
    );

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          onFetchMessages: (_) async => messages,
          onSendMessage: (_, __) async => messages.first,
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Scroll up
    await tester.drag(find.byType(ListView), const Offset(0, 500));
    await tester.pumpAndSettle();

    // Verify FAB with down arrow icon appears
    expect(find.byKey(const ValueKey('scroll_to_bottom_fab')), findsOneWidget);

    // Tap FAB
    await tester.tap(find.byKey(const ValueKey('scroll_to_bottom_fab')));
    await tester.pumpAndSettle();

    // Verify FAB disappears when scrolled back to bottom
    expect(find.byKey(const ValueKey('scroll_to_bottom_fab')), findsNothing);
  });

  testWidgets('ChatScreen activates reply preview banner on reply action and renders quoted bubbles', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_reply',
      sendQueueUri: 'https://ntfy.sh/q_reply',
      receiveQueueId: 'q_reply_in',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    final msg1 = MessageSummary(
      id: 'm1',
      conversationId: 'conv_reply',
      direction: 'inbound',
      bodyText: 'Are you using post-quantum crypto?',
      timestamp: now.millisecondsSinceEpoch ~/ 1000,
      status: 'received',
    );

    final msg2 = MessageSummary(
      id: 'm2',
      conversationId: 'conv_reply',
      direction: 'outbound',
      bodyText: '[QUOTE:Are you using post-quantum crypto?]\nYes, ML-KEM-1024 enabled!',
      timestamp: (now.millisecondsSinceEpoch ~/ 1000) + 10,
      status: 'sent',
    );

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          onFetchMessages: (_) async => [msg1, msg2],
          onSendMessage: (_, __) async => msg1,
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Verify msg2 renders both the quote snippet and the reply text
    expect(find.text('Are you using post-quantum crypto?'), findsWidgets);
    expect(find.text('Yes, ML-KEM-1024 enabled!'), findsOneWidget);

    // Long press msg1 to trigger Reply
    await tester.longPress(find.text('Are you using post-quantum crypto?').first);
    await tester.pumpAndSettle();

    expect(find.text('Reply'), findsOneWidget);
    await tester.tap(find.text('Reply'));
    await tester.pumpAndSettle();

    // Verify reply preview banner is displayed
    expect(find.text('Anonymous Peer'), findsOneWidget);
    expect(find.byKey(const ValueKey('cancel_reply_btn')), findsOneWidget);

    // Tap cancel button
    await tester.tap(find.byKey(const ValueKey('cancel_reply_btn')));
    await tester.pumpAndSettle();

    // Verify reply banner closed
    expect(find.byKey(const ValueKey('cancel_reply_btn')), findsNothing);
  });

  testWidgets('ChatScreen displays single tick for sent, double grey for delivered, double blue for read', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_ticks',
      sendQueueUri: 'https://ntfy.sh/q_ticks',
      receiveQueueId: 'q_ticks_in',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    final msgSent = MessageSummary(
      id: 'm_sent',
      conversationId: 'conv_ticks',
      direction: 'outbound',
      bodyText: 'Msg 1 sent',
      timestamp: now.millisecondsSinceEpoch ~/ 1000,
      status: 'sent',
    );

    final msgDelivered = MessageSummary(
      id: 'm_delivered',
      conversationId: 'conv_ticks',
      direction: 'outbound',
      bodyText: 'Msg 2 delivered',
      timestamp: (now.millisecondsSinceEpoch ~/ 1000) + 1,
      status: 'delivered',
    );

    final msgRead = MessageSummary(
      id: 'm_read',
      conversationId: 'conv_ticks',
      direction: 'outbound',
      bodyText: 'Msg 3 read',
      timestamp: (now.millisecondsSinceEpoch ~/ 1000) + 2,
      status: 'read',
    );

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          onFetchMessages: (_) async => [msgSent, msgDelivered, msgRead],
          onSendMessage: (_, __) async => msgSent,
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Verify tick widgets
    expect(find.byKey(const ValueKey('tick_sent')), findsOneWidget);
    expect(find.byKey(const ValueKey('tick_delivered')), findsOneWidget);
    expect(find.byKey(const ValueKey('tick_read')), findsOneWidget);

    // Verify read tick has blue color (0xFF34B7F1)
    final readIcon = tester.widget<Icon>(find.byKey(const ValueKey('tick_read')));
    expect(readIcon.color, const Color(0xFF34B7F1));
    expect(readIcon.icon, Icons.done_all);

    // Verify delivered tick has white70 color
    final deliveredIcon = tester.widget<Icon>(find.byKey(const ValueKey('tick_delivered')));
    expect(deliveredIcon.color, Colors.white70);
    expect(deliveredIcon.icon, Icons.done_all);
  });

  testWidgets('ChatScreen supports disappearing messages timer configuration and renders timer icons', (WidgetTester tester) async {
    final now = DateTime.now();
    final conv = ConversationSummary(
      id: 'conv_ephemeral',
      sendQueueUri: 'https://ntfy.sh/q_ephemeral',
      receiveQueueId: 'q_ephemeral_in',
      createdAt: now.millisecondsSinceEpoch ~/ 1000,
    );

    final msgEphemeral = MessageSummary(
      id: 'm_eph',
      conversationId: 'conv_ephemeral',
      direction: 'outbound',
      bodyText: 'Disappearing confidential message',
      timestamp: now.millisecondsSinceEpoch ~/ 1000,
      status: 'sent',
      expiresAt: (now.millisecondsSinceEpoch ~/ 1000) + 3600,
    );

    int? configuredTimerSeconds;

    await tester.pumpWidget(
      MaterialApp(
        home: ChatScreen(
          conversation: conv,
          initialDisappearingTimer: 3600,
          onFetchMessages: (_) async => [msgEphemeral],
          onSendMessage: (_, __) async => msgEphemeral,
          onSetDisappearingTimer: (seconds) async {
            configuredTimerSeconds = seconds;
            return true;
          },
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Verify app bar displays 1h timer indicator
    expect(find.byKey(const ValueKey('appbar_timer_indicator')), findsOneWidget);
    expect(find.text('1h'), findsOneWidget);

    // Verify message bubble contains timer icon
    expect(find.byKey(const ValueKey('msg_timer_icon')), findsOneWidget);

    // Open timer menu
    await tester.tap(find.byKey(const ValueKey('disappearing_timer_menu')));
    await tester.pumpAndSettle();

    // Select "24 hours"
    expect(find.text('24 hours'), findsOneWidget);
    await tester.tap(find.text('24 hours'));
    await tester.pumpAndSettle();

    // Verify callback was triggered with 86400 seconds
    expect(configuredTimerSeconds, 86400);

    // Verify indicator updated to 1d (86400s)
    expect(find.text('1d'), findsOneWidget);
  });
}
