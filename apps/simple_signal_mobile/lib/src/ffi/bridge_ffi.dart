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

typedef _RecvMsgC = Pointer<Utf8> Function(Pointer<Utf8> convId);
typedef _RecvMsgDart = Pointer<Utf8> Function(Pointer<Utf8> convId);

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
  _RecvMsgDart? _recvMsgFn;
  _FreeStringDart? _freeStringFn;

  final RelayClient _relayClient = RelayClient();
  String? _myInboxQueueId;
  String? _lastSinceId;

  bool _isNativeLoaded = false;
  final List<ConversationSummary> _fallbackConversations = [];
  final Map<String, List<MessageSummary>> _fallbackMessages = {};
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
        _recvMsgFn = _dylib!.lookupFunction<_RecvMsgC, _RecvMsgDart>('simple_signal_receive_messages');
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
          }
        }
      } finally {
        calloc.free(cDbPath);
        calloc.free(cPin);
      }
    } else {
      // Fallback
      await Future.delayed(const Duration(milliseconds: 200));
      final randomSuffix = DateTime.now().millisecondsSinceEpoch % 100000;
      _fallbackContactLink =
          'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_anon_$randomSuffix&auth=1111111111111111111111111111111111111111111111111111111111111111&ik=5696058f86bd9b645b49ec1a996ca25f45ee70a5ed02f98e6f3390857d06acac&idhk=c446bd64a0b22a5c4576ca6ea83f3331c1c6eccc6e811c2773d67ac987b6b465&spkid=1&spk=0c340b19acef708d94106244aba6d02948ba34fb97a2a94dcb3f8521e29f3e1f&sig=e4ddb33ddfefeaebc622f6171f1ce4f7bcb1e7979e814a3caece10b2ca9e7cc04dc210201416a9c4443913a6973ba899ca5d7f2b9e49f8cec77ca611edb1cc0d&otpkid=100&otpk=3040c59ce434f11046eb34b658c68e17a9fc6be6693b556bf7d76e036bf8c95b';
    }
  }

  @override
  Future<String?> getMyInboxQueueId() async {
    if (_myInboxQueueId != null) return _myInboxQueueId;
    if (_isNativeLoaded && _getQueueFn != null) {
      final qPtr = _getQueueFn!();
      final qJson = _parseAndFree(qPtr);
      if (qJson['success'] == true && qJson['queue_id'] != null) {
        _myInboxQueueId = qJson['queue_id'] as String;
      }
    }
    return _myInboxQueueId;
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
    return _fallbackContactLink;
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
              final ok = await _relayClient.sendPayload(queueId: partnerQueue, payloadHex: handshakeHex);
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
              final ok = await _relayClient.sendPayload(queueId: partnerQueue, payloadHex: payloadHex);
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
      final msg = MessageSummary(
        id: 'msg_${DateTime.now().millisecondsSinceEpoch}',
        conversationId: conversationId,
        direction: 'outbound',
        bodyText: text,
        timestamp: DateTime.now().millisecondsSinceEpoch ~/ 1000,
        status: 'sent',
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
                  if (conversationId == null || conversationId.isEmpty || msg.conversationId == conversationId) {
                    newlyReceived.add(msg);
                  }
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
