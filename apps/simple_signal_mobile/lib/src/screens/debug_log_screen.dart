import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../logging/app_logger.dart';

enum LogFilter { all, errors, ffi, tor }

class DebugLogScreen extends StatefulWidget {
  const DebugLogScreen({Key? key}) : super(key: key);

  @override
  State<DebugLogScreen> createState() => _DebugLogScreenState();
}

class _DebugLogScreenState extends State<DebugLogScreen> {
  LogFilter _currentFilter = LogFilter.all;

  List<LogEntry> _getFilteredLogs() {
    final all = AppLogger.entries;
    switch (_currentFilter) {
      case LogFilter.all:
        return all;
      case LogFilter.errors:
        return all.where((e) => e.level == LogLevel.error).toList();
      case LogFilter.ffi:
        return all.where((e) => e.level == LogLevel.ffi || e.tag == 'FFI').toList();
      case LogFilter.tor:
        return all.where((e) => e.level == LogLevel.tor || e.tag == 'TOR').toList();
    }
  }

  void _copyLogsToClipboard() {
    final text = AppLogger.exportLogs();
    Clipboard.setData(ClipboardData(text: text));
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Scrubbed logs copied to clipboard (100% zero-knowledge)'),
        duration: Duration(seconds: 2),
      ),
    );
  }

  void _clearLogs() {
    setState(() {
      AppLogger.clear();
    });
  }

  Color _getColorForLevel(LogLevel level) {
    switch (level) {
      case LogLevel.error:
        return const Color(0xFFFF5252);
      case LogLevel.warning:
        return const Color(0xFFFFD740);
      case LogLevel.ffi:
        return const Color(0xFF64B5F6);
      case LogLevel.tor:
        return const Color(0xFF69F0AE);
      case LogLevel.info:
      default:
        return const Color(0xFFECEFF1);
    }
  }

  @override
  Widget build(BuildContext context) {
    final logs = _getFilteredLogs();

    return Scaffold(
      backgroundColor: const Color(0xFF101418),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A2028),
        title: const Text(
          'Diagnostic Logs',
          style: TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.copy_rounded),
            tooltip: 'Copy Logs',
            onPressed: _copyLogsToClipboard,
          ),
          IconButton(
            icon: const Icon(Icons.delete_outline_rounded),
            tooltip: 'Clear Logs',
            onPressed: _clearLogs,
          ),
        ],
      ),
      body: Column(
        children: [
          // Zero-knowledge privacy notice banner
          Container(
            width: double.infinity,
            margin: const EdgeInsets.all(12),
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: const Color(0xFF15222E),
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: Colors.tealAccent.withOpacity(0.3)),
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Icon(Icons.shield_rounded, color: Colors.tealAccent, size: 20),
                const SizedBox(width: 10),
                const Expanded(
                  child: Text(
                    'Zero-Knowledge & Local: All secrets, PINs, and auth keys are automatically scrubbed. No surveillance analytics or crash reports are ever sent externally.',
                    style: TextStyle(
                      color: Colors.white70,
                      fontSize: 12,
                      height: 1.35,
                    ),
                  ),
                ),
              ],
            ),
          ),

          // Filter chips
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Row(
              children: [
                ChoiceChip(
                  label: const Text('All'),
                  selected: _currentFilter == LogFilter.all,
                  onSelected: (selected) {
                    if (selected) setState(() => _currentFilter = LogFilter.all);
                  },
                ),
                const SizedBox(width: 8),
                ChoiceChip(
                  label: const Text('Errors'),
                  selected: _currentFilter == LogFilter.errors,
                  selectedColor: Colors.redAccent.withOpacity(0.3),
                  onSelected: (selected) {
                    if (selected) setState(() => _currentFilter = LogFilter.errors);
                  },
                ),
                const SizedBox(width: 8),
                ChoiceChip(
                  label: const Text('FFI'),
                  selected: _currentFilter == LogFilter.ffi,
                  selectedColor: Colors.blueAccent.withOpacity(0.3),
                  onSelected: (selected) {
                    if (selected) setState(() => _currentFilter = LogFilter.ffi);
                  },
                ),
                const SizedBox(width: 8),
                ChoiceChip(
                  label: const Text('Tor'),
                  selected: _currentFilter == LogFilter.tor,
                  selectedColor: Colors.tealAccent.withOpacity(0.3),
                  onSelected: (selected) {
                    if (selected) setState(() => _currentFilter = LogFilter.tor);
                  },
                ),
              ],
            ),
          ),
          const SizedBox(height: 8),

          // Log display area
          Expanded(
            child: logs.isEmpty
                ? const Center(
                    child: Text(
                      'No logs recorded',
                      style: TextStyle(color: Colors.white38, fontSize: 14),
                    ),
                  )
                : Container(
                    margin: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: const Color(0xFF0A0D10),
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(color: Colors.white12),
                    ),
                    child: ListView.builder(
                      itemCount: logs.length,
                      itemBuilder: (context, index) {
                        final entry = logs[index];
                        final timeStr = entry.timestamp.toIso8601String().split('T').last;
                        final color = _getColorForLevel(entry.level);

                        return Padding(
                          padding: const EdgeInsets.symmetric(vertical: 2),
                          child: SelectableText.rich(
                            TextSpan(
                              style: const TextStyle(
                                fontFamily: 'monospace',
                                fontSize: 11,
                                height: 1.4,
                              ),
                              children: [
                                TextSpan(
                                  text: '[$timeStr] ',
                                  style: const TextStyle(color: Colors.white38),
                                ),
                                TextSpan(
                                  text: '[${entry.tag}] ',
                                  style: TextStyle(color: color.withOpacity(0.85), fontWeight: FontWeight.bold),
                                ),
                                TextSpan(
                                  text: '${entry.level.name.toUpperCase()}: ',
                                  style: TextStyle(color: color, fontWeight: FontWeight.bold),
                                ),
                                TextSpan(
                                  text: entry.message,
                                  style: TextStyle(color: color),
                                ),
                                if (entry.error != null)
                                  TextSpan(
                                    text: '\n  ERROR: ${entry.error}',
                                    style: const TextStyle(color: Color(0xFFFF8A80)),
                                  ),
                                if (entry.stackTrace != null)
                                  TextSpan(
                                    text: '\n  STACK: ${entry.stackTrace}',
                                    style: const TextStyle(color: Colors.white38),
                                  ),
                              ],
                            ),
                          ),
                        );
                      },
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}
