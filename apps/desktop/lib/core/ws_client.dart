import 'dart:convert';
import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import '../models.dart';
import 'providers.dart';

const String wsUrl = 'ws://127.0.0.1:7432/ws';

class WsClient {
  final Ref _ref;
  WebSocketChannel? _channel;
  bool _shouldReconnect = true;
  Timer? _reconnectTimer;
  final _eventController = StreamController<Map<String, dynamic>>.broadcast();

  Stream<Map<String, dynamic>> get rawStream => _eventController.stream;

  WsClient(this._ref);

  void connect() {
    _shouldReconnect = true;
    _ref.read(connectionStatusProvider.notifier).setStatus(ConnectionStatus.connecting);

    try {
      _channel = WebSocketChannel.connect(Uri.parse(wsUrl));
      _channel!.stream.listen(
        (message) {
          _ref.read(connectionStatusProvider.notifier).setStatus(ConnectionStatus.connected);
          _handleMessage(message);
        },
        onError: (err) {
          _handleDisconnect();
        },
        onDone: () {
          _handleDisconnect();
        },
      );
    } catch (e) {
      _handleDisconnect();
    }
  }

  void _handleDisconnect() {
    _ref.read(connectionStatusProvider.notifier).setStatus(ConnectionStatus.disconnected);
    if (_shouldReconnect) {
      _reconnectTimer?.cancel();
      _reconnectTimer = Timer(const Duration(seconds: 2), () {
        connect();
      });
    }
  }

  void _handleMessage(dynamic rawMsg) {
    try {
      final json = jsonDecode(rawMsg) as Map<String, dynamic>;
      _eventController.add(json);

      final type = json['type'] as String?;
      if (type == null) return;

      switch (type) {
        case 'clipboard_updated':
          final items = (json['items'] as List)
              .map((x) => ClipboardEntry.fromJson(x as Map<String, dynamic>))
              .toList();
          _ref.read(clipboardHistoryProvider.notifier).setHistory(items);
          break;

        case 'file_shared':
          final file = SharedFile.fromJson(json['file'] as Map<String, dynamic>);
          _ref.read(sharedFilesProvider.notifier).addOrUpdate(file);
          break;

        case 'file_revoked':
          final id = json['id'] as String;
          _ref.read(sharedFilesProvider.notifier).remove(id);
          break;

        case 'device_joined':
        case 'device_updated':
          final device = Device.fromJson(json['device'] as Map<String, dynamic>);
          _ref.read(devicesProvider.notifier).addOrUpdate(device);
          break;

        case 'device_left':
          final deviceId = json['device_id'] as String;
          _ref.read(devicesProvider.notifier).remove(deviceId);
          break;

        case 'transfer_started':
        case 'transfer_progress':
        case 'transfer_completed':
        case 'transfer_failed':
          final task = TransferTask.fromJson(json['task'] as Map<String, dynamic>);
          _ref.read(transferTasksProvider.notifier).addOrUpdate(task);
          break;
      }
    } catch (e) {
      debugPrint("WS message error: $e");
    }
  }

  void disconnect() {
    _shouldReconnect = false;
    _reconnectTimer?.cancel();
    _channel?.sink.close();
    _channel = null;
    _ref.read(connectionStatusProvider.notifier).setStatus(ConnectionStatus.disconnected);
  }

  void send(Map<String, dynamic> data) {
    if (_channel != null) {
      _channel!.sink.add(jsonEncode(data));
    }
  }
}

final wsClientProvider = Provider<WsClient>((ref) {
  final client = WsClient(ref);
  ref.onDispose(() => client.disconnect());
  return client;
});
