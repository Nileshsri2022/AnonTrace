import '../ffi/bridge.dart';

/// Sealed hierarchy representing items rendered in the conversation timeline viewport.
sealed class TimelineItem {
  const TimelineItem();
}

/// Centered date divider chip rendered when calendar day changes.
class DateHeaderItem extends TimelineItem {
  final String label;
  final DateTime date;

  const DateHeaderItem({required this.label, required this.date});
}

/// Message bubble rendered for incoming or outgoing chat message.
class MessageBubbleItem extends TimelineItem {
  final MessageSummary message;
  final String formattedTime;

  const MessageBubbleItem({required this.message, required this.formattedTime});
}

/// Parses an epoch timestamp in either seconds or milliseconds to local DateTime.
DateTime parseMessageDateTime(int timestamp) {
  if (timestamp <= 0) {
    return DateTime.now();
  }
  final ms = timestamp < 10000000000 ? timestamp * 1000 : timestamp;
  return DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true).toLocal();
}

/// Formats a timestamp into a 12-hour local time string (`hh:mm a`, e.g. "10:45 AM").
String formatMessageTime(int timestamp) {
  final dt = parseMessageDateTime(timestamp);
  int hour = dt.hour;
  final period = hour >= 12 ? 'PM' : 'AM';
  hour = hour % 12;
  if (hour == 0) hour = 12;
  final minuteStr = dt.minute.toString().padLeft(2, '0');
  return '$hour:$minuteStr $period';
}

const List<String> _monthNames = [
  '',
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December'
];

/// Formats a date into "Today", "Yesterday", or "d MMMM yyyy".
String formatDateHeader(DateTime date, {DateTime? now}) {
  final current = now ?? DateTime.now();
  final dYear = date.year;
  final dMonth = date.month;
  final dDay = date.day;

  final cYear = current.year;
  final cMonth = current.month;
  final cDay = current.day;

  if (dYear == cYear && dMonth == cMonth && dDay == cDay) {
    return 'Today';
  }

  final yesterday = current.subtract(const Duration(days: 1));
  if (dYear == yesterday.year && dMonth == yesterday.month && dDay == yesterday.day) {
    return 'Yesterday';
  }

  final monthName = (dMonth >= 1 && dMonth <= 12) ? _monthNames[dMonth] : '$dMonth';
  return '$dDay $monthName $dYear';
}

/// Transforms linear list of messages into grouped timeline items with date chips.
List<TimelineItem> groupMessagesByDate(List<MessageSummary> messages, {DateTime? now}) {
  if (messages.isEmpty) return const [];

  final List<TimelineItem> items = [];
  DateTime? lastDate;

  for (final msg in messages) {
    final msgDt = parseMessageDateTime(msg.timestamp);
    final msgDay = DateTime(msgDt.year, msgDt.month, msgDt.day);

    if (lastDate == null ||
        lastDate.year != msgDay.year ||
        lastDate.month != msgDay.month ||
        lastDate.day != msgDay.day) {
      final label = formatDateHeader(msgDay, now: now);
      items.add(DateHeaderItem(label: label, date: msgDay));
      lastDate = msgDay;
    }

    items.add(MessageBubbleItem(
      message: msg,
      formattedTime: formatMessageTime(msg.timestamp),
    ));
  }

  return items;
}
