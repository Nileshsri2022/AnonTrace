import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'bridge.dart';

SimpleSignalBridge getPlatformBridge() => NativeSimpleSignalBridge();

typedef _InitC = Pointer<Utf8> Function(Pointer<Utf8> dbPath, Pointer<Utf8> passphrase);
typedef _InitDart = Pointer<Utf8> Function(Pointer<Utf8> dbPath, Pointer<Utf8> passphrase);

typedef _GetLinkC = Pointer<Utf8> Function();
typedef _GetLinkDart = Pointer<Utf8> Function();

typedef _ImportLinkC = Pointer<Utf8> Function(Pointer<Utf8> uri);
typedef _ImportLinkDart = Pointer<Utf8> Function(Pointer<Utf8> uri);

typedef _SendMessageC = Pointer<Utf8> Function(Pointer<Utf8> convId, Pointer<Utf8> text);
typedef _SendMessageDart = Pointer<Utf8> Function(Pointer<Utf8> convId, Pointer<Utf8> text);

typedef _ListConvC = Pointer<Utf8> Function();
typedef _ListConvDart = Pointer<Utf8> Function();

typedef _ListMsgC = Pointer<Utf8> Function(Pointer<Utf8> convId);
typedef _ListMsgDart = Pointer<Utf8> Function(Pointer<Utf8> convId);

typedef _FreeStringC = Void Function(Pointer<Utf8> ptr);
typedef _FreeStringDart = void Function(Pointer<Utf8> ptr);

class NativeSimpleSignalBridge implements SimpleSignalBridge {
  DynamicLibrary? _dylib;
  _InitDart? _initFn;
  _GetLinkDart? _getLinkFn;
  _ImportLinkDart? _importLinkFn;
  _SendMessageDart? _sendMessageFn;
  _ListConvDart? _listConvFn;
  _ListMsgDart? _listMsgFn;
  _FreeStringDart? _freeStringFn;

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
        _importLinkFn = _dylib!.lookupFunction<_ImportLinkC, _ImportLinkDart>('simple_signal_import_contact_link');
        _sendMessageFn = _dylib!.lookupFunction<_SendMessageC, _SendMessageDart>('simple_signal_send_message');
        _listConvFn = _dylib!.lookupFunction<_ListConvC, _ListConvDart>('simple_signal_list_conversations');
        _listMsgFn = _dylib!.lookupFunction<_ListMsgC, _ListMsgDart>('simple_signal_list_messages');
        _freeStringFn = _dylib!.lookupFunction<_FreeStringC, _FreeStringDart>('simple_signal_free_string');

        _isNativeLoaded = true;
      }
    } catch (_) {
      // DynamicLibrary load failed or running in non-native environment; fallback gracefully
      _isNativeLoaded = false;
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
    if (_isNativeLoaded && _initFn != null) {
      final cDbPath = dbPath.toNativeUtf8();
      final cPin = masterPin.toNativeUtf8();
      try {
        final resPtr = _initFn!(cDbPath, cPin);
        final json = _parseAndFree(resPtr);
        if (json['success'] != true) {
          throw Exception(json['error'] ?? 'Initialization failed in native core');
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
          'simplex:/contact#/?v=1&relay=vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd.onion&queue=q_anon_$randomSuffix';
    }
  }

  @override
  Future<String> getMyContactLink() async {
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
    if (_isNativeLoaded && _importLinkFn != null) {
      final cUri = uri.toNativeUtf8();
      try {
        final resPtr = _importLinkFn!(cUri);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['conversation'] != null) {
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
    if (_isNativeLoaded && _sendMessageFn != null) {
      final cConvId = conversationId.toNativeUtf8();
      final cText = text.toNativeUtf8();
      try {
        final resPtr = _sendMessageFn!(cConvId, cText);
        final json = _parseAndFree(resPtr);
        if (json['success'] == true && json['message'] != null) {
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
  Future<void> setDormant(bool dormant) async {}

  @override
  Future<bool> isActive() async => _isNativeLoaded;
}
