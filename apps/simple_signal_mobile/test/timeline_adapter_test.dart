import 'package:flutter_test/flutter_test.dart';
import 'package:simple_signal_mobile/src/ffi/bridge.dart';
import 'package:simple_signal_mobile/src/utils/timeline_adapter.dart';

void main() {
  group('TimelineAdapter', () {
    test('formatMessageTime formats epoch seconds and milliseconds to 12-hour format', () {
      // 2026-10-24 14:35:00 UTC
      final dt = DateTime.utc(2026, 10, 24, 14, 35, 0).toLocal();
      final epochSeconds = dt.millisecondsSinceEpoch ~/ 1000;

      final formatted = formatMessageTime(epochSeconds);
      // Check that it contains hour and minutes and AM/PM
      expect(formatted, matches(r'^\d{1,2}:\d{2}\s+(AM|PM)$'));
    });

    test('formatDateHeader returns Today, Yesterday, and formatted date', () {
      final now = DateTime.now();

      final today = DateTime(now.year, now.month, now.day, 10, 30);
      expect(formatDateHeader(today, now: now), equals('Today'));

      final yesterday = today.subtract(const Duration(days: 1));
      expect(formatDateHeader(yesterday, now: now), equals('Yesterday'));

      final olderDate = DateTime(2025, 4, 15, 12, 0);
      expect(formatDateHeader(olderDate, now: now), equals('15 April 2025'));
    });

    test('groupMessagesByDate groups consecutive messages by date header', () {
      final now = DateTime.now();
      final yesterday = now.subtract(const Duration(days: 1));

      final msg1 = MessageSummary(
        id: 'msg_1',
        conversationId: 'c1',
        direction: 'inbound',
        bodyText: 'Hello yesterday',
        timestamp: yesterday.millisecondsSinceEpoch ~/ 1000,
        status: 'received',
      );

      final msg2 = MessageSummary(
        id: 'msg_2',
        conversationId: 'c1',
        direction: 'outbound',
        bodyText: 'Hello today 1',
        timestamp: now.millisecondsSinceEpoch ~/ 1000,
        status: 'sent',
      );

      final msg3 = MessageSummary(
        id: 'msg_3',
        conversationId: 'c1',
        direction: 'inbound',
        bodyText: 'Hello today 2',
        timestamp: (now.millisecondsSinceEpoch ~/ 1000) + 60,
        status: 'received',
      );

      final items = groupMessagesByDate([msg1, msg2, msg3], now: now);

      // Expected timeline:
      // [DateHeaderItem(Yesterday), MessageBubbleItem(msg1), DateHeaderItem(Today), MessageBubbleItem(msg2), MessageBubbleItem(msg3)]
      expect(items.length, equals(5));

      expect(items[0], isA<DateHeaderItem>());
      expect((items[0] as DateHeaderItem).label, equals('Yesterday'));

      expect(items[1], isA<MessageBubbleItem>());
      expect((items[1] as MessageBubbleItem).message.id, equals('msg_1'));

      expect(items[2], isA<DateHeaderItem>());
      expect((items[2] as DateHeaderItem).label, equals('Today'));

      expect(items[3], isA<MessageBubbleItem>());
      expect((items[3] as MessageBubbleItem).message.id, equals('msg_2'));

      expect(items[4], isA<MessageBubbleItem>());
      expect((items[4] as MessageBubbleItem).message.id, equals('msg_3'));
    });

    test('groupMessagesByDate returns empty list for empty input', () {
      final items = groupMessagesByDate([]);
      expect(items, isEmpty);
    });

    test('buildQuotedPayload and parseMessageBody correctly format and extract quotes', () {
      final payload = buildQuotedPayload('Previous text snippet', 'My reply answer');
      expect(payload, equals('[QUOTE:Previous text snippet]\nMy reply answer'));

      final parsed = parseMessageBody(payload);
      expect(parsed.quotedSnippet, equals('Previous text snippet'));
      expect(parsed.mainText, equals('My reply answer'));

      final normalParsed = parseMessageBody('Plain regular message');
      expect(normalParsed.quotedSnippet, isNull);
      expect(normalParsed.mainText, equals('Plain regular message'));
    });
  });
}
