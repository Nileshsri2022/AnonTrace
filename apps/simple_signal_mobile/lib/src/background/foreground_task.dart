import 'dart:async';

import 'package:flutter_foreground_task/flutter_foreground_task.dart';

/// Foreground service for AnonTrace (Production Roadmap Step 4,
/// Ticket 26.4 full wiring — previously only a 60 s poll timer
/// existed, which Android Doze freezes when the app is killed).
///
/// Design, stated plainly because it limits what this service can do:
///
/// * The background isolate CANNOT decrypt or poll the relay itself:
///   it has no PIN and must never re-init the Rust core with a
///   stored PIN. Its only job is to keep the process alive and send
///   a tick to the main isolate every 30 s.
/// * The main isolate, on receiving a tick, polls the relay via the
///   already-unlocked bridge and stores messages in the DB. If the
///   user locked the app (UI auto-lock), the main isolate still
///   runs while the foreground service keeps the process alive;
///   messages appear after the next unlock.
/// * If Android kills the process despite the foreground service
///   (aggressive OEM battery savers do this), delivery stops until
///   the user reopens the app. No foreground service, FCM, or poll
///   can promise otherwise on those devices — see Issue 30 §4.
///
/// Battery cost: a foreground service + 30 s heartbeat costs more
/// than the old 60 s timer alone. It is therefore OPT-IN: the user
/// starts it from the AppBar button, and it stops when they tap
/// again or when the app is disposed. It is never started silently
/// on unlock.

/// Entry point for the background isolate. Must be top-level.
@pragma('vm:entry-point')
void startForegroundCallback() {
  FlutterForegroundTask.setTaskHandler(AnonTraceTaskHandler());
}

class AnonTraceTaskHandler extends TaskHandler {
  @override
  Future<void> onStart(DateTime timestamp, TaskStarter starter) async {
    // Nothing to initialise here: no PIN, no Rust core, no relay
    // client in this isolate, by design (see the header comment).
  }

  @override
  void onRepeatEvent(DateTime timestamp) {
    // Tick the main isolate; it does the actual relay poll.
    FlutterForegroundTask.sendDataToMain({'type': 'poll_tick'});
  }

  @override
  Future<void> onDestroy(DateTime timestamp, bool isTimeout) async {}

  @override
  void onNotificationButtonPressed(String id) {}

  @override
  void onNotificationPressed() {
    FlutterForegroundTask.launchApp();
  }
}

/// Small wrapper so main.dart does not need to know the plugin API.
class AnonTraceForegroundService {
  static bool _initialised = false;

  static void init() {
    if (_initialised) return;
    FlutterForegroundTask.init(
      androidNotificationOptions: AndroidNotificationOptions(
        channelId: 'anontrace_foreground',
        channelName: 'AnonTrace Background',
        channelDescription:
            'Keeps AnonTrace running so messages can arrive while the app is in the background.',
        channelImportance: NotificationChannelImportance.LOW,
        priority: NotificationPriority.LOW,
      ),
      iosNotificationOptions: const IOSNotificationOptions(
        showNotification: true,
        playSound: false,
      ),
      foregroundTaskOptions: ForegroundTaskOptions(
        interval: 30000,
        autoRunOnBoot: false,
        allowWakeLock: true,
        allowWifiLock: true,
      ),
    );
    _initialised = true;
  }

  static Future<bool> get isRunning =>
      FlutterForegroundTask.isRunningService;

  static Future<void> start() async {
    init();
    await FlutterForegroundTask.startService(
      notificationTitle: 'AnonTrace is running',
      notificationText: 'Listening for new messages in the background.',
      callback: startForegroundCallback,
    );
  }

  static Future<void> stop() async {
    await FlutterForegroundTask.stopService();
  }
}
