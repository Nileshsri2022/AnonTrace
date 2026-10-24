import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import '../logging/app_logger.dart';
import '../net/relay_client.dart';
import 'bridge.dart';

SimpleSignalBridge getPlatformBridge() => NativeSimpleSignalBridge();

typedef _InitC = Pointer<Utf8> Function(Pointer<Utf8> dbPath, Pointer<Utf8> passphrase);
typedef _InitDart = Pointer<Utf8> Function(Pointer<Utf8> dbPath, Pointer<Utf8> passphrase);

typedef _GetLinkC = Pointer<Utf8> Function();
typedef _GetLinkDart = Pointer<Utf8> Function();

typedef _GetQueueC = Pointer<Utf8> Function();
typedef _GetQueueDart = Pointer<Utf8> Function();

typedef _ProcessPayloadC = Pointer<Utf8> Function(Pointer<Utf8> payloadHex);
typedef _ProcessPayloadDart = Pointer<Utf8> Function(Pointer<Utf8> payloadHex);

typedef _ImportLinkC = Pointer<Utf8> Function(Pointer<Utf8> uri);
typedef _ImportLinkDart = Pointer<Utf8> Function(Pointer<Utf8> uri);

typedef _SendMessageC = Pointer<Utf8> Function(Pointer<Utf8> convId, Pointer<Utf8> text);
typedef _SendMessageDart = Pointer<Utf8> Function(Pointer<Utf8> convId, Pointer<Utf8> text);

typedef _ListConvC = Pointer<Utf8> Function();
typedef _ListConvDart = Pointer<Utf8> Function();

typedef _ListMsgC = Pointer<Utf8> Function(Pointer<Utf8> convId);
typedef _ListMsgDart = Pointer<Utf8> Function(Pointer<Utf8> convId);

typedef _DeleteMsgC = Pointer<Utf8> Function(Pointer<Utf8> msgId);
typedef _DeleteMsgDart = Pointer<Utf8> Function(Pointer<Utf8> msgId);

typedef _UpdateStatusC = Pointer<Utf8> Function(Pointer<Utf8> msgId, Pointer<Utf8> status);
typedef _UpdateStatusDart = Pointer<Utf8> Function(Pointer<Utf8> msgId, Pointer<Utf8> status);

typedef _SetTimerC = Pointer<Utf8> Function(Pointer<Utf8> convId, Int64 seconds);
typedef _SetTimerDart = Pointer<Utf8> Function(Pointer<Utf8> convId, int seconds);

typedef _PurgeC = Pointer<Utf8> Function();
typedef _PurgeDart = Pointer<Utf8> Function();

typedef _RecvMsgC = Pointer<Utf8> Function(Pointer<Utf8> convId);
typedef _RecvMsgDart = Pointer<Utf8> Function(Pointer<Utf8> convId);

typedef _GetSenderAuthC = Pointer<Utf8> Function(Pointer<Utf8> convId);
typedef _GetSenderAuthDart = Pointer<Utf8> Function(Pointer<Utf8> convId);

typedef _SetTransportModeC = Pointer<Utf8> Function(Pointer<Utf8> mode);
typedef _SetTransportModeDart = Pointer<Utf8> Function(Pointer<Utf8> mode);

typedef _GetTransportModeC = Pointer<Utf8> Function();
typedef _GetTransportModeDart = Pointer<Utf8> Function();

typedef _StartStealthC = Pointer<Utf8> Function(Pointer<Utf8> dataDir, Pointer<Utf8> onion);
typedef _StartStealthDart = Pointer<Utf8> Function(Pointer<Utf8> dataDir, Pointer<Utf8> onion);

typedef _StartStealthBridgesC = Pointer<Utf8> Function(Pointer<Utf8> dataDir, Pointer<Utf8> onion, Pointer<Utf8> bridges);
typedef _StartStealthBridgesDart = Pointer<Utf8> Function(Pointer<Utf8> dataDir, Pointer<Utf8> onion, Pointer<Utf8> bridges);

typedef _GetStealthStatusC = Pointer<Utf8> Function();
typedef _GetStealthStatusDart = Pointer<Utf8> Function();

typedef _StealthSendC = Pointer<Utf8> Function(Pointer<Utf8> queueId, Pointer<Utf8> senderAuthHex, Pointer<Utf8> payloadHex);
typedef _StealthSendDart = Pointer<Utf8> Function(Pointer<Utf8> queueId, Pointer<Utf8> senderAuthHex, Pointer<Utf8> payloadHex);

typedef _FreeStringC = Void Function(Pointer<Utf8> ptr);
typedef _FreeStringDart = void Function(Pointer<Utf8> ptr);

class NativeSimpleSignalBridge implements SimpleSignalBridge {
  DynamicLibrary? _dylib;
  _InitDart? _initFn;
  _GetLinkDart? _getLinkFn;
  _GetQueueDart? _getQueueFn;
  _ProcessPayloadDart? _processPayloadFn;
  _ImportLinkDart? _importLinkFn;
  _SendMessageDart? _sendMessageFn;
  _ListConvDart? _listConvFn;
  _ListMsgDart? _listMsgFn;
  _DeleteMsgDart? _deleteMsgFn;
  _UpdateStatusDart? _updateStatusFn;
  _SetTimerDart? _setTimerFn;
  _PurgeDart? _purgeFn;
  _RecvMsgDart? _recvMsgFn;
  _GetSenderAuthDart? _getSenderAuthFn;
  _SetTransportModeDart? _setTransportModeFn;
  _GetTransportModeDart? _getTransportModeFn;
  _StartStealthDart? _startStealthFn;
  _StartStealthBridgesDart? _startStealthBridgesFn;
  _GetStealthStatusDart? _getStealthStatusFn;
  _StealthSendDart? _stealthSendFn;
  _FreeStringDart? _freeStringFn;

  final RelayClient _relayClient = RelayClient();
  String? _myInboxQueueId;
  String? _lastSinceId;

  final StreamController<MessageSummary> _messageStreamController = StreamController<MessageSummary>.broadcast();
  StreamSubscription<RelayMessage>? _queueSubscription;

  @override
  Stream<MessageSummary> get messageStream => _messageStreamController.stream;

  bool _isNativeLoaded = false;
  final List<ConversationSummary> _fallbackConversations = [];
  final Map<String, List<MessageSummary>> _fallbackMessages = {};
  final Map<String, int> _fallbackDisappearingTimers = {};
  String _fallbackContactLink = '';

  NativeSimpleSignalBridge() {
    _loadLibrary();
  }

  void _loadLibrary() {
    try {
      if (Platform.isAndroid) {
        _dylib = DynamicLibrary.open('libsimple_signal_core.so');
      } else if (Platform.isLinux) {
        _dylib = DynamicLibrary.open('libsimple_signal_core.so');
      } else if (Platform.isWindows) {
        _dylib = DynamicLibrary.open('simple_signal_core.dll');
      } else if (Platform.isMacOS) {
        _dylib = DynamicLibrary.open('libsimple_signal_core.dylib');
      }

      if (_dylib != null) {
        _initFn = _dylib!.lookupFunction<_InitC, _InitDart>('simple_signal_init');
        _getLinkFn = _dylib!.lookupFunction<_GetLinkC, _GetLinkDart>('simple_signal_get_contact_link');
        _getQueueFn = _dylib!.lookupFunction<_GetQueueC, _GetQueueDart>('simple_signal_get_inbox_queue');
        _processPayloadFn = _dylib!.lookupFunction<_ProcessPayloadC, _ProcessPayloadDart>('simple_signal_process_payload');
        _importLinkFn = _dylib!.lookupFunction<_ImportLinkC, _ImportLinkDart>('simple_signal_import_contact_link');
        _sendMessageFn = _dylib!.lookupFunction<_SendMessageC, _SendMessageDart>('simple_signal_send_message');
        _listConvFn = _dylib!.lookupFunction<_ListConvC, _ListConvDart>('simple_signal_list_conversations');
        _listMsgFn = _dylib!.lookupFunction<_ListMsgC, _ListMsgDart>('simple_signal_list_messages');
        _deleteMsgFn = _dylib!.lookupFunction<_DeleteMsgC, _DeleteMsgDart>('simple_signal_delete_message');
        _updateStatusFn = _dylib!.lookupFunction<_UpdateStatusC, _UpdateStatusDart>('simple_signal_update_message_status');
        _setTimerFn = _dylib!.lookupFunction<_SetTimerC, _SetTimerDart>('simple_signal_set_disappearing_timer');
        _purgeFn = _dylib!.lookupFunction<_PurgeC, _PurgeDart>('simple_signal_purge_expired_messages');
        _recvMsgFn = _dylib!.lookupFunction<_RecvMsgC, _RecvMsgDart>('simple_signal_receive_messages');
        _getSenderAuthFn = _dylib!.lookupFunction<_GetSenderAuthC, _GetSenderAuthDart>('simple_signal_get_conversation_sender_auth');
        _setTransportModeFn = _dylib!.lookupFunction<_SetTransportModeC, _SetTransportModeDart>('simple_signal_set_transport_mode');
        _getTransportModeFn = _dylib!.lookupFunction<_GetTransportModeC, _GetTransportModeDart>('simple_signal_get_transport_mode');
        _startStealthFn = _dylib!.lookupFunction<_StartStealthC, _StartStealthDart>('simple_signal_start_stealth_bootstrap');
        _startStealthBridgesFn = _dylib!.lookupFunction<_StartStealthBridgesC, _StartStealthBridgesDart>('simple_signal_start_stealth_bootstrap_with_bridges');
        _getStealthStatusFn = _dylib!.lookupFunction<_GetStealthStatusC, _GetStealthStatusDart>('simple_signal_get_stealth_status');
        _stealthSendFn = _dylib!.lookupFunction<_StealthSendC, _StealthSendDart>('simple_signal_stealth_send');
        _freeStringFn = _dylib!.lookupFunction<_FreeStringC, _FreeStringDart>('simple_signal_free_string');

        _isNativeLoaded = true;
        AppLogger.ffi('DynamicLibrary.open', 'libsimple_signal_core native library linked successfully');
      }
    } catch (e) {
      // DynamicLibrary load failed or running in non-native environment; fallback gracefully
      _isNativeLoaded = false;
      AppLogger.warning('FFI', 'Native library load failed, using fallback engine: $e');
    }
  }

  bool get isNativeLoaded => _isNativeLoaded;

  Map<String, dynamic> _parseAndFree(Pointer<Utf8> ptr) {
    if (ptr == nullptr) return {'success': false, 'error': 'Null pointer from native core'};
    final str = ptr.toDartString();
    if (_freeStringFn != null) {
      _freeStringFn!(ptr);
    }
    return jsonDecode(str) as Map<String, dynamic>;
  }

  @override
  Future<void> initApp({required String dbPath, required String masterPin}) async {
    AppLogger.ffi('simple_signal_init', 'Starting crypto session at $dbPath (Native: $_isNativeLoaded)');
    if (_isNativeLoaded && _initFn != null) {
      final cDbPath = dbPath.toNativeUtf8();
      final cPin = masterPin.toNativeUtf8();
      try {
        final resPtr = _initFn!(cDbPath, cPin);
        final json = _parseAndFree(resPtr);
        if (json['success'] != true) {
          throw Exception(json['error'] ?? 'Initialization failed in native core');
        }
        if (_getQueueFn != null) {
          final qPtr = _getQueueFn!();
          final qJson = _parseAndFree(qPtr);
          if (qJson['success'] == true && qJson['queue_id'] != null) {
            _myInboxQueueId = qJson['queue_id'] as String;
            AppLogger.info('Relay', 'Local inbox queue initialized: $_myInboxQueueId');
            _startInboxStream();
          }
        }
      } finally {
        calloc.free(cDbPath);
        calloc.free(cPin);
      }
    } else {
      // Phase 0 (Issue 27): NO fabricated fallback link. The old code
      // invented a contact link here with shared, hardcoded keys when
      // the native library failed to load. A contact link is an
      // identity — inventing one silently is worse than failing.
      // getMyContactLink() throws in this state (see below).
      await Future.delayed(const Duration(milliseconds: 200));
      _fallbackContactLink = '';
      AppLogger.warning('FFI', 'Native library not loaded — no contact link available');
    }
  }

  void _startInboxStream() {
    _queueSubscription?.cancel();
    if (_myInboxQueueId == null || !_isNativeLoaded || _processPayloadFn == null) return;

    AppLogger.info('Relay', 'Connecting persistent SSE stream to inbox queue: $_myInboxQueueId');
    _queueSubscription = _relayClient
        .listenToQueue(queueId: _myInboxQueueId!, sinceId: _lastSinceId)
        .listen((rMsg) {
      _lastSinceId = rMsg.id;
      if (rMsg.payloadHex.isNotEmpty && _processPayloadFn != null) {
        final cHex = rMsg.payloadHex.toNativeUtf8();
        try {
          final procPtr = _processPayloadFn!(cHex);
          final procJson = _parseAndFree(procPtr);
          if (procJson['success'] == true && procJson['message'] != null) {
            final msg = MessageSummary.fromJson(procJson['message'] as Map<String, dynamic>);
            AppLogger.info('Relay', 'Received real-time message via SSE stream for conv: ${msg.conversationId}');
            _messageStreamController.add(msg);
            _relayClient.ackMessage(queueId: _myInboxQueueId!, messageId: rMsg.id);
          } else if (procJson['success'] == true) {
            AppLogger.info('Relay', 'Processed incoming handshake frame via SSE stream');
            _relayClient.ackMessage(queueId: _myInboxQueueId!, messageId: rMsg.id);
          }
        } catch (e) {
          AppLogger.warning('Relay', 'Error processing streamed payload: $e');
        } finally {
          calloc.free(cHex);
        }
      }
    }, onError: (e) {
      AppLogger.warning('Relay', 'SSE inbox stream error: $e');
    });
  }

  @override
  Future<String?> getMyInboxQueueId() async {
    if (_myInboxQueueId != null) {
      if (_queueSubscription == null) _startInboxStream();
      return _myInboxQueueId;
    }
    if (_isNativeLoaded && _getQueueFn != null) {
      final qPtr = _getQueueFn!();
      final qJson = _parseAndFree(qPtr);
      if (qJson['success'] == true && qJson['queue_id'] != null) {
        _myInboxQueueId = qJson['queue_id'] as String;
        _startInboxStream();
      }
    }
    return _myInboxQueueId;
  }

  /// Partner sender-auth key (hex) for a conversation, for private-relay
  /// POSTs. Returns null if the native function is unavailable.
  String? _senderAuthForConversation(String conversationId) {
    if (!_isNativeLoaded || _getSenderAuthFn == null) return null;
    final cConvId = conversationId.toNativeUtf8();
    try {
      final resPtr = _getSenderAuthFn!(cConvId);
      final json = _parseAndFree(resPtr);
      if (json['success'] == true && json['sender_auth_hex'] != null) {
        return json['sender_auth_hex'] as String;
      }
      return null;
    } catch (_) {
      return null;
    } finally {
      calloc.free(cConvId);
    }
  }

  /// Starts the embedded Arti bootstrap in the background (Ticket 26.3).
  /// Returns immediately; poll [getStealthStatus] for progress. Never
  /// routes traffic through Flutter HttpClient SOCKS — Stealth traffic
  /// goes through the Rust Arti client only.
  Future<bool> startStealthBootstrap({required String dataDir, String? onionAddress, String? bridgeLines}) async {
    if (!_isNativeLoaded) return false;
    final cDir = dataDir.toNativeUtf8();
    final cOnion = (onionAddress ?? '').toNativeUtf8();
    final cBridges = (bridgeLines ?? '').toNativeUtf8();
    try {
      final resPtr = (bridgeLines != null && bridgeLines.isNotEmpty && _startStealthBridgesFn != null)
          ? _startStealthBridgesFn!(cDir, cOnion, cBridges)
          : _startStealthFn != null
              ? _startStealthFn!(cDir, cOnion)
              : null;
      if (resPtr == null) return false;
      final json = _parseAndFree(resPtr);
      return json['success'] == true;
    } catch (_) {
      return false;
    } finally {
      calloc.free(cDir);
      calloc.free(cOnion);
      calloc.free(cBridges);
    }
  }

  /// Current Stealth bootstrap status, or null if unavailable.
  Map<String, dynamic>? getStealthStatus() {
    if (!_isNativeLoaded || _getStealthStatusFn == null) return null;
    final resPtr = _getStealthStatusFn!();
    final json = _parseAndFree(resPtr);
    if (json['success'] == true) return json;
    return null;
  }

  @override
  Future<bool> setTransportMode(String mode) async {
    if (!_isNativeLoaded || _setTransportModeFn == null) return false;
    final cMode = mode.toNativeUtf8();
    try {
      final resPtr = _setTransportModeFn!(cMode);
      final json = _parseAndFree(resPtr);
      return json['success'] == true;
    } catch (_) {
      return false;
    } finally {
      calloc.free(cMode);
    }
  }

  @override
  Future<String?> getTransportMode() async {
    if (!_isNativeLoaded || _getTransportModeFn == null) return null;
    final resPtr = _getTransportModeFn!();
    final json = _parseAndFree(resPtr);
    if (json['success'] == true && json['mode'] != null) {
      return json['mode'] as String;
    }
    return null;
  }

  @override
  Future<bool> stealthSend({required String queueId, required String senderAuthHex, required String payloadHex}) async {
    if (!_isNativeLoaded || _stealthSendFn == null) return false;
    final cQueue = queueId.toNativeUtf8();
    final cAuth = senderAuthHex.toNativeUtf8();
    final cPayload = payloadHex.toNativeUtf8();
    try {
      final resPtr = _stealthSendFn!(cQueue, cAuth, cPayload);
      final json = _parseAndFree(resPtr);
      return json['success'] == true;
    } catch (_) {
      return false;
    } finally {
      calloc.free(cQueue);
      calloc.free(cAuth);
      calloc.free(cPayload);
    }
  }

  @override
  Future<String> getMyContactLink() async {
    AppLogger.ffi('simple_signal_get_contact_link', 'Requesting identity public contact URI');
    if (_isNativeLoaded && _getLinkFn != null) {
      final resPtr = _getLinkFn!();
      final json = _parseAndFree(resPtr);
      if (json['success'] == true && json['uri'] != null) {
        return json['uri'] as String;
      }
      throw Exception(json['error'] ?? 'Failed to retrieve contact link');
    }
    // Phase 0 (Issue 27): never return a fabricated link.
    throw Exception('Native core not loaded — no contact link available');
  }

  @override
  Future<ConversationSummary> importContactLink(String uri) async {
    AppLogger.ffi('simple_signal_import_contact_link', 'Importing contact link: $uri');
    if (_isNativeLoaded && _importLinkFn != null) {
      final cUri = uri.toNativeUtf8();
      try {
        final resPtr = _importLinkFn!(cUri);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['conversation'] != null) {
          // If handshake payload returned, send to partner queue over HTTPS relay
          final partnerQueue = json['partner_queue_id'] as String?;
          final handshakeHex = json['handshake_hex'] as String?;
          if (partnerQueue != null && partnerQueue.isNotEmpty && handshakeHex != null && handshakeHex.isNotEmpty) {
            try {
              final convId = (json['conversation'] as Map<String, dynamic>)['id'] as String?;
              final senderAuth = convId != null ? _senderAuthForConversation(convId) : null;
              final mode = await getTransportMode();
              final bool ok;
              if (mode == 'stealth' && senderAuth != null) {
                ok = await stealthSend(queueId: partnerQueue, senderAuthHex: senderAuth, payloadHex: handshakeHex);
              } else {
                ok = await _relayClient.sendPayload(queueId: partnerQueue, payloadHex: handshakeHex, senderAuthHex: senderAuth);
              }
              if (ok) {
                AppLogger.info('Relay', 'Handshake payload dispatched to relay queue: $partnerQueue');
              } else {
                AppLogger.warning('Relay', 'Relay returned non-200 for handshake to: $partnerQueue');
              }
            } catch (e) {
              AppLogger.warning('Relay', 'Failed to dispatch handshake to relay: $e');
            }
          }
          return ConversationSummary.fromJson(json['conversation'] as Map<String, dynamic>);
        }
        throw Exception(json['error'] ?? 'Failed to import contact link');
      } finally {
        calloc.free(cUri);
      }
    } else {
      final newConv = ConversationSummary(
        id: 'conv_partner_${DateTime.now().millisecondsSinceEpoch % 1000}',
        sendQueueUri: uri,
        receiveQueueId: 'q_local_${DateTime.now().millisecondsSinceEpoch}',
        createdAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      );
      _fallbackConversations.insert(0, newConv);
      return newConv;
    }
  }

  @override
  Future<MessageSummary> sendChatMessage({required String conversationId, required String text}) async {
    AppLogger.ffi('simple_signal_send_message', 'Outbound message for conv: $conversationId');
    if (_isNativeLoaded && _sendMessageFn != null) {
      final cConvId = conversationId.toNativeUtf8();
      final cText = text.toNativeUtf8();
      try {
        final resPtr = _sendMessageFn!(cConvId, cText);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['message'] != null) {
          // Dispatch encrypted ciphertext payload to partner queue over HTTPS relay
          final partnerQueue = json['partner_queue_id'] as String?;
          final payloadHex = json['payload_hex'] as String?;
          if (partnerQueue != null && partnerQueue.isNotEmpty && payloadHex != null && payloadHex.isNotEmpty) {
            try {
              final senderAuth = _senderAuthForConversation(conversationId);
              final mode = await getTransportMode();
              final bool ok;
              if (mode == 'stealth' && senderAuth != null) {
                // Stealth send via Rust Arti (onion). Blocks the isolate
                // for the Tor request — Stealth sends are expected to be
                // slow; do not fall back to clearnet on failure.
                ok = await stealthSend(queueId: partnerQueue, senderAuthHex: senderAuth, payloadHex: payloadHex);
              } else {
                ok = await _relayClient.sendPayload(queueId: partnerQueue, payloadHex: payloadHex, senderAuthHex: senderAuth);
              }
              if (ok) {
                AppLogger.info('Relay', 'Encrypted payload dispatched to relay queue: $partnerQueue');
              } else {
                AppLogger.warning('Relay', 'Relay returned non-200 for message to: $partnerQueue');
              }
            } catch (e) {
              AppLogger.warning('Relay', 'Failed to dispatch payload to relay: $e');
            }
          }
          return MessageSummary.fromJson(json['message'] as Map<String, dynamic>);
        }
        throw Exception(json['error'] ?? 'Failed to send message via native core');
      } finally {
        calloc.free(cConvId);
        calloc.free(cText);
      }
    } else {
      final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final timer = _fallbackDisappearingTimers[conversationId] ?? 0;
      final msg = MessageSummary(
        id: 'msg_${DateTime.now().millisecondsSinceEpoch}',
        conversationId: conversationId,
        direction: 'outbound',
        bodyText: text,
        timestamp: now,
        status: 'sent',
        expiresAt: timer > 0 ? now + timer : null,
      );
      _fallbackMessages.putIfAbsent(conversationId, () => []).add(msg);
      return msg;
    }
  }

  @override
  Future<List<ConversationSummary>> listConversations() async {
    if (_isNativeLoaded && _listConvFn != null) {
      final resPtr = _listConvFn!();
      final json = _parseAndFree(resPtr);
      if (json['success'] == true && json['conversations'] != null) {
        final list = json['conversations'] as List<dynamic>;
        return list.map((c) => ConversationSummary.fromJson(c as Map<String, dynamic>)).toList();
      }
      return [];
    }
    return List.from(_fallbackConversations);
  }

  @override
  Future<List<MessageSummary>> listMessages(String conversationId) async {
    if (_isNativeLoaded && _listMsgFn != null) {
      final cConvId = conversationId.toNativeUtf8();
      try {
        final resPtr = _listMsgFn!(cConvId);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['messages'] != null) {
          final list = json['messages'] as List<dynamic>;
          return list.map((m) => MessageSummary.fromJson(m as Map<String, dynamic>)).toList();
        }
        return [];
      } finally {
        calloc.free(cConvId);
      }
    }
    return List.from(_fallbackMessages[conversationId] ?? []);
  }

  @override
  Future<bool> deleteMessage(String messageId) async {
    AppLogger.ffi('simple_signal_delete_message', 'Deleting message: $messageId');
    if (_isNativeLoaded && _deleteMsgFn != null) {
      final cMsgId = messageId.toNativeUtf8();
      try {
        final resPtr = _deleteMsgFn!(cMsgId);
        final json = _parseAndFree(resPtr);
        return json['success'] == true && json['deleted'] == true;
      } finally {
        calloc.free(cMsgId);
      }
    }
    for (final list in _fallbackMessages.values) {
      final idx = list.indexWhere((m) => m.id == messageId);
      if (idx != -1) {
        list.removeAt(idx);
        return true;
      }
    }
    return false;
  }

  @override
  Future<bool> updateMessageStatus({required String messageId, required String status}) async {
    AppLogger.ffi('simple_signal_update_message_status', 'Updating message $messageId to $status');
    if (_isNativeLoaded && _updateStatusFn != null) {
      final cMsgId = messageId.toNativeUtf8();
      final cStatus = status.toNativeUtf8();
      try {
        final resPtr = _updateStatusFn!(cMsgId, cStatus);
        final json = _parseAndFree(resPtr);
        return json['success'] == true && json['updated'] == true;
      } finally {
        calloc.free(cMsgId);
        calloc.free(cStatus);
      }
    }
    for (final list in _fallbackMessages.values) {
      final idx = list.indexWhere((m) => m.id == messageId);
      if (idx != -1) {
        final old = list[idx];
        list[idx] = MessageSummary(
          id: old.id,
          conversationId: old.conversationId,
          direction: old.direction,
          bodyText: old.bodyText,
          timestamp: old.timestamp,
          status: status,
        );
        return true;
      }
    }
    return false;
  }

  @override
  Future<bool> setDisappearingTimer({required String conversationId, required int durationSeconds}) async {
    AppLogger.ffi('simple_signal_set_disappearing_timer', 'Setting timer for $conversationId to ${durationSeconds}s');
    _fallbackDisappearingTimers[conversationId] = durationSeconds;
    if (_isNativeLoaded && _setTimerFn != null) {
      final cConvId = conversationId.toNativeUtf8();
      try {
        final resPtr = _setTimerFn!(cConvId, durationSeconds);
        final json = _parseAndFree(resPtr);
        return json['success'] == true;
      } finally {
        calloc.free(cConvId);
      }
    }
    return true;
  }

  @override
  Future<int> purgeExpiredMessages() async {
    int nativeCount = 0;
    if (_isNativeLoaded && _purgeFn != null) {
      final resPtr = _purgeFn!();
      final json = _parseAndFree(resPtr);
      if (json['success'] == true && json['purged_count'] != null) {
        nativeCount = json['purged_count'] as int;
      }
    }
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    for (final list in _fallbackMessages.values) {
      list.removeWhere((m) => m.expiresAt != null && m.expiresAt! <= now);
    }
    return nativeCount;
  }

  @override
  Future<List<MessageSummary>> receiveMessages({String? conversationId}) async {
    final List<MessageSummary> newlyReceived = [];

    // 1. If native core is loaded, poll the HTTPS relay queue for this device's inbox
    if (_isNativeLoaded && _processPayloadFn != null) {
      if (_myInboxQueueId == null && _getQueueFn != null) {
        final qPtr = _getQueueFn!();
        final qJson = _parseAndFree(qPtr);
        if (qJson['success'] == true && qJson['queue_id'] != null) {
          _myInboxQueueId = qJson['queue_id'] as String;
        }
      }

      if (_myInboxQueueId != null) {
        if (_queueSubscription == null) {
          _startInboxStream();
        }
        try {
          final relayMessages = await _relayClient.pollQueue(
            queueId: _myInboxQueueId!,
            sinceId: _lastSinceId,
          );
          for (final rMsg in relayMessages) {
            _lastSinceId = rMsg.id;
            if (rMsg.payloadHex.isNotEmpty) {
              final cHex = rMsg.payloadHex.toNativeUtf8();
              try {
                final procPtr = _processPayloadFn!(cHex);
                final procJson = _parseAndFree(procPtr);
                if (procJson['success'] == true && procJson['message'] != null) {
                  final msg = MessageSummary.fromJson(procJson['message'] as Map<String, dynamic>);
                  _messageStreamController.add(msg);
                  if (conversationId == null || conversationId.isEmpty || msg.conversationId == conversationId) {
                    newlyReceived.add(msg);
                  }
                  await _relayClient.ackMessage(queueId: _myInboxQueueId!, messageId: rMsg.id);
                }
              } finally {
                calloc.free(cHex);
              }
            }
          }
        } catch (e) {
          AppLogger.warning('Relay', 'Error polling relay queue $_myInboxQueueId: $e');
        }
      }
    }

    // 2. Also poll native in-memory hub (keeps existing tests & local loopback compatible)
    if (_isNativeLoaded && _recvMsgFn != null) {
      final cConvId = (conversationId ?? '').toNativeUtf8();
      try {
        final resPtr = _recvMsgFn!(cConvId);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['messages'] != null) {
          final list = json['messages'] as List<dynamic>;
          for (final item in list) {
            final msg = MessageSummary.fromJson(item as Map<String, dynamic>);
            if (!newlyReceived.any((m) => m.id == msg.id)) {
              newlyReceived.add(msg);
              _messageStreamController.add(msg);
            }
          }
        }
      } finally {
        calloc.free(cConvId);
      }
    }
    return newlyReceived;
  }

  @override
  Future<void> setDormant(bool dormant) async {}

  @override
  Future<bool> isActive() async => _isNativeLoaded;
}
