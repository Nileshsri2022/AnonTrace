import 'dart:collection';
import 'package:flutter/foundation.dart';
import 'log_scrubber.dart';

enum LogLevel { info, warning, error, ffi, tor }

class LogEntry {
  final DateTime timestamp;
  final LogLevel level;
  final String tag;
  final String message;
  final String? error;
  final String? stackTrace;

  LogEntry({
    required this.timestamp,
    required this.level,
    required this.tag,
    required this.message,
    this.error,
    this.stackTrace,
  });

  bool get isError => level == LogLevel.error;

  String format() {
    final timeStr =
        "${timestamp.hour.toString().padLeft(2, '0')}:${timestamp.minute.toString().padLeft(2, '0')}:${timestamp.second.toString().padLeft(2, '0')}.${timestamp.millisecond.toString().padLeft(3, '0')}";
    final levelStr = level == LogLevel.error ? 'ERROR' : level.name.toUpperCase();
    final base = '[$timeStr] [$tag] $levelStr: $message';
    if (error != null && error!.isNotEmpty) {
      if (stackTrace != null && stackTrace!.isNotEmpty) {
        return '$base\n  Exception: $error\n$stackTrace';
      }
      return '$base\n  Exception: $error';
    }
    return base;
  }
}

class AppLogger {
  static const int maxCapacity = 500;
  static final DoubleLinkedQueue<LogEntry> _queue = DoubleLinkedQueue<LogEntry>();

  /// Clears all logs (useful in tests or when user purges history).
  static void clear() {
    _queue.clear();
  }

  static void info(String tag, String message) {
    _add(LogLevel.info, tag, message);
  }

  static void warning(String tag, String message) {
    _add(LogLevel.warning, tag, message);
  }

  static void error(
    String tag,
    String message, [
    Object? errorObj,
    StackTrace? stack,
  ]) {
    _add(
      LogLevel.error,
      tag,
      message,
      errorObj?.toString(),
      stack?.toString(),
    );
  }

  static void ffi(String functionName, String jsonResult) {
    _add(LogLevel.ffi, 'FFI', '$functionName -> $jsonResult');
  }

  static void tor(String event) {
    _add(LogLevel.tor, 'TOR', event);
  }

  static void _add(
    LogLevel level,
    String tag,
    String rawMessage, [
    String? rawError,
    String? rawStack,
  ]) {
    final scrubbedMessage = LogScrubber.scrub(rawMessage);
    final scrubbedError = rawError != null ? LogScrubber.scrub(rawError) : null;

    final entry = LogEntry(
      timestamp: DateTime.now(),
      level: level,
      tag: tag,
      message: scrubbedMessage,
      error: scrubbedError,
      stackTrace: rawStack,
    );

    _queue.addLast(entry);
    while (_queue.length > maxCapacity) {
      _queue.removeFirst();
    }

    if (kDebugMode) {
      // In debug mode, also mirror to standard platform developer console
      debugPrint('[AnonTrace] ${entry.format()}');
    }
  }

  /// Returns unmodifiable list of all log entries.
  static List<LogEntry> get entries => List.unmodifiable(_queue);

  /// Returns unmodifiable list of log entries, optionally filtered.
  static List<LogEntry> getLogs({String? tag, bool errorsOnly = false}) {
    var list = _queue.toList();
    if (errorsOnly) {
      list = list.where((e) => e.isError).toList();
    }
    if (tag != null && tag.isNotEmpty) {
      list = list.where((e) => e.tag.toUpperCase() == tag.toUpperCase()).toList();
    }
    return List.unmodifiable(list);
  }

  /// Exports all collected logs formatted as plain text for copying/sharing.
  static String exportLogs() => exportLogsAsText();

  static String exportLogsAsText() {
    final sb = StringBuffer();
    sb.writeln('=== AnonTrace Diagnostic & Crash Logs (Signal Standard) ===');
    sb.writeln('Generated: ${DateTime.now().toIso8601String()}');
    sb.writeln('Total Log Entries: ${_queue.length}');
    sb.writeln('===========================================================');
    for (final entry in _queue) {
      sb.writeln(entry.format());
    }
    return sb.toString();
  }

  /// Initializes global uncaught error hooks.
  static void initErrorHooks() {
    FlutterError.onError = (FlutterErrorDetails details) {
      AppLogger.error(
        'FLUTTER_UI',
        details.exceptionAsString(),
        details.exception,
        details.stack,
      );
      FlutterError.presentError(details);
    };

    PlatformDispatcher.instance.onError = (Object error, StackTrace stack) {
      AppLogger.error('ASYNC_UNCAUGHT', error.toString(), error, stack);
      return true; // handled
    };
  }
}
