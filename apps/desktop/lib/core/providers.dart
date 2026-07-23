import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:dio/dio.dart';
import 'dart:io';
import 'dart:convert';
import 'package:path/path.dart' as p;
import 'package:flutter/foundation.dart';
import '../models.dart';

const String serverBaseUrl = 'http://127.0.0.1:7432';

// ── Connection Status ────────────────────────────────────────────────────────

enum ConnectionStatus { connecting, connected, disconnected }

class ConnectionStatusNotifier extends StateNotifier<ConnectionStatus> {
  ConnectionStatusNotifier() : super(ConnectionStatus.disconnected);
  void setStatus(ConnectionStatus status) => state = status;
}

final connectionStatusProvider =
    StateNotifierProvider<ConnectionStatusNotifier, ConnectionStatus>((ref) {
  return ConnectionStatusNotifier();
});

// ── Local History File Helpers ───────────────────────────────────────────────

File getLocalHistoryFile() {
  final home = Platform.environment['HOME'] ?? '.';
  final dirPath = p.join(home, '.lynqo');
  final dir = Directory(dirPath);
  if (!dir.existsSync()) {
    dir.createSync(recursive: true);
  }
  return File(p.join(dirPath, 'clipboard_history.json'));
}

Future<List<ClipboardEntry>> loadLocalHistory() async {
  try {
    final file = getLocalHistoryFile();
    if (await file.exists()) {
      final content = await file.readAsString();
      final list = jsonDecode(content) as List;
      return list.map((x) => ClipboardEntry.fromJson(x as Map<String, dynamic>)).toList();
    }
  } catch (e) {
    debugPrint("Error loading local history: $e");
  }
  return [];
}

void saveLocalHistory(List<ClipboardEntry> entries) {
  try {
    final file = getLocalHistoryFile();
    final content = jsonEncode(entries.map((e) => {
      'id': e.id,
      'content': e.content,
      'content_type': e.contentType,
      'source': e.source,
      'created_at': e.createdAt,
      'is_favorite': e.isFavorite,
      'category': e.category,
      'ocr_text': e.ocrText,
      'metadata_json': e.metadataJson,
      'hash': e.hash,
    }).toList());
    file.writeAsStringSync(content);
  } catch (e) {
    debugPrint("Error saving local history: $e");
  }
}

// ── Clipboard Provider ───────────────────────────────────────────────────────

class ClipboardNotifier extends StateNotifier<List<ClipboardEntry>> {
  ClipboardNotifier() : super([]) {
    _init();
  }

  Future<void> _init() async {
    final local = await loadLocalHistory();
    if (local.isNotEmpty) {
      state = local;
    }
  }

  void setHistory(List<ClipboardEntry> items) {
    state = items;
    saveLocalHistory(items);
  }

  void addOrUpdate(ClipboardEntry item) {
    final filtered = state.where((x) => x.content != item.content && x.id != item.id).toList();
    final newHistory = [item, ...filtered];
    state = newHistory;
    saveLocalHistory(newHistory);
  }
}

final clipboardHistoryProvider =
    StateNotifierProvider<ClipboardNotifier, List<ClipboardEntry>>((ref) {
  return ClipboardNotifier();
});

// ── Shared Files Provider ────────────────────────────────────────────────────

class SharedFilesNotifier extends StateNotifier<List<SharedFile>> {
  SharedFilesNotifier() : super([]);

  void setFiles(List<SharedFile> files) {
    state = files;
  }

  void addOrUpdate(SharedFile file) {
    state = [file, ...state.where((x) => x.id != file.id)];
  }

  void remove(String id) {
    state = state.where((x) => x.id != id).toList();
  }
}

final sharedFilesProvider =
    StateNotifierProvider<SharedFilesNotifier, List<SharedFile>>((ref) {
  return SharedFilesNotifier();
});

// ── Connected Devices Provider ────────────────────────────────────────────────

class DevicesNotifier extends StateNotifier<List<Device>> {
  DevicesNotifier() : super([]);

  void setDevices(List<Device> devices) {
    state = devices;
  }

  void addOrUpdate(Device device) {
    state = [device, ...state.where((x) => x.id != device.id)];
  }

  void remove(String id) {
    state = state.where((x) => x.id != id).toList();
  }
}

final devicesProvider =
    StateNotifierProvider<DevicesNotifier, List<Device>>((ref) {
  return DevicesNotifier();
});

// ── Transfer Tasks Provider ──────────────────────────────────────────────────

class TransferTasksNotifier extends StateNotifier<List<TransferTask>> {
  TransferTasksNotifier() : super([]);

  void setTasks(List<TransferTask> tasks) {
    state = tasks;
  }

  void addOrUpdate(TransferTask task) {
    state = [task, ...state.where((x) => x.id != task.id)];
  }

  void remove(String id) {
    state = state.where((x) => x.id != id).toList();
  }
}

final transferTasksProvider =
    StateNotifierProvider<TransferTasksNotifier, List<TransferTask>>((ref) {
  return TransferTasksNotifier();
});

// ── API Operations Helper ────────────────────────────────────────────────────

final dioProvider = Provider<Dio>((ref) {
  return Dio(BaseOptions(baseUrl: serverBaseUrl));
});

final apiServiceProvider = Provider<ApiService>((ref) {
  return ApiService(ref.read(dioProvider), ref);
});

class ApiService {
  final Dio _dio;
  final Ref _ref;

  ApiService(this._dio, this._ref);

  Future<void> fetchAll() async {
    try {
      final List<dynamic> filesData = (await _dio.get('/api/files')).data;
      _ref.read(sharedFilesProvider.notifier).setFiles(
            filesData.map((x) => SharedFile.fromJson(x)).toList(),
          );

      final List<dynamic> clipData = (await _dio.get('/api/clipboard')).data;
      _ref.read(clipboardHistoryProvider.notifier).setHistory(
            clipData.map((x) => ClipboardEntry.fromJson(x)).toList(),
          );

      final List<dynamic> devicesData = (await _dio.get('/api/devices')).data;
      _ref.read(devicesProvider.notifier).setDevices(
            devicesData.map((x) => Device.fromJson(x)).toList(),
          );
    } catch (e) {
      // Server might not be ready yet
    }
  }

  Future<SharedFile?> shareFile(String path) async {
    try {
      final response = await _dio.post('/api/files/share', data: {'path': path});
      if (response.statusCode == 201) {
        final file = SharedFile.fromJson(response.data);
        _ref.read(sharedFilesProvider.notifier).addOrUpdate(file);
        return file;
      }
    } catch (e) {
      // Handle error
    }
    return null;
  }

  Future<void> revokeFile(String id) async {
    try {
      await _dio.delete('/api/files/$id');
      _ref.read(sharedFilesProvider.notifier).remove(id);
    } catch (e) {
      // Handle error
    }
  }

  Future<void> pushClipboard(String text) async {
    try {
      await _dio.post('/api/clipboard', data: {'text': text});
    } catch (e) {
      // Handle error
    }
  }
}

final selectedFileIdsProvider = StateProvider<Set<String>>((ref) => {});

enum WindowMode {
  normal,
  popup,
}

final windowModeProvider = StateProvider<WindowMode>((ref) => WindowMode.normal);

