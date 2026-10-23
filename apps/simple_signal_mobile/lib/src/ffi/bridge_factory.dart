import 'bridge.dart';
import 'bridge_stub.dart'
    if (dart.library.ffi) 'bridge_ffi.dart';

SimpleSignalBridge createSignalBridge() => getPlatformBridge();
