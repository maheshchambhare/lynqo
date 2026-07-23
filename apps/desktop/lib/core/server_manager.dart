import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;

class ServerManager {
  static final ServerManager _instance = ServerManager._internal();
  factory ServerManager() => _instance;
  ServerManager._internal();

  Process? _process;
  bool _isStarting = false;
  final ValueNotifier<bool> isRunning = ValueNotifier<bool>(false);
  final ValueNotifier<String?> serverOutput = ValueNotifier<String?>(null);

  Future<void> start() async {
    if (isRunning.value || _isStarting) return;
    _isStarting = true;

    try {
      final binaryPath = await _findServerBinary();
      if (binaryPath == null) {
        throw Exception("Server binary not found.");
      }

      String workingDir = p.dirname(binaryPath);
      Directory dir = Directory(workingDir);
      while (dir.path != dir.parent.path) {
        final webDistDir = Directory(p.join(dir.path, 'web', 'browser-ui', 'dist'));
        if (await webDistDir.exists()) {
          workingDir = dir.path;
          break;
        }
        dir = dir.parent;
      }

      debugPrint("Spawning server from: $binaryPath with working directory: $workingDir");
      _process = await Process.start(
        binaryPath,
        [],
        workingDirectory: workingDir,
      );

      isRunning.value = true;
      _isStarting = false;

      // Handle output streams
      _process!.stdout.transform(utf8.decoder).listen((data) {
        serverOutput.value = data;
        debugPrint("[Server STDOUT]: $data");
      });

      _process!.stderr.transform(utf8.decoder).listen((data) {
        serverOutput.value = data;
        debugPrint("[Server STDERR]: $data");
      });

      _process!.exitCode.then((code) {
        debugPrint("Server process exited with code: $code");
        isRunning.value = false;
        _process = null;
        // Attempt to auto-restart if it crashed
        if (code != 0) {
          debugPrint("Server crashed. Restarting in 2 seconds...");
          Future.delayed(const Duration(seconds: 2), start);
        }
      });
    } catch (e) {
      _isStarting = false;
      isRunning.value = false;
      debugPrint("Error starting server: $e");
    }
  }

  Future<void> stop() async {
    if (_process != null) {
      debugPrint("Stopping server process...");
      _process!.kill();
      _process = null;
      isRunning.value = false;
    }
  }

  Future<String?> _findServerBinary() async {
    // 1. Check if the binary is bundled next to the executable (standard production packaging)
    final executablePath = Platform.resolvedExecutable;
    final executableDir = p.dirname(executablePath);
    final bundledPathUnix = p.join(executableDir, 'lynqo-server');
    final bundledPathWin = p.join(executableDir, 'lynqo-server.exe');

    if (await File(bundledPathUnix).exists()) {
      return bundledPathUnix;
    }
    if (await File(bundledPathWin).exists()) {
      return bundledPathWin;
    }

    // 2. Traversal lookup: Compare release and debug binary timestamps, pick the newest
    Directory dir = Directory(executableDir);
    while (dir.path != dir.parent.path) {
      final targetDir = Directory(p.join(dir.path, 'target'));
      if (await targetDir.exists()) {
        final binary = await _getNewestBinary(targetDir.path);
        if (binary != null) return binary;
      }
      dir = dir.parent;
    }

    // 3. Fallback: Search relative to Directory.current
    Directory currentDir = Directory(Directory.current.path);
    while (currentDir.path != currentDir.parent.path) {
      final targetDir = Directory(p.join(currentDir.path, 'target'));
      if (await targetDir.exists()) {
        final binary = await _getNewestBinary(targetDir.path);
        if (binary != null) return binary;
      }
      currentDir = currentDir.parent;
    }

    return null;
  }

  Future<String?> _getNewestBinary(String targetDirPath) async {
    final releaseFile = File(p.join(targetDirPath, 'release', 'lynqo-server'));
    final debugFile = File(p.join(targetDirPath, 'debug', 'lynqo-server'));

    final releaseExists = await releaseFile.exists();
    final debugExists = await debugFile.exists();

    if (releaseExists && debugExists) {
      final releaseTime = (await releaseFile.lastModified()).millisecondsSinceEpoch;
      final debugTime = (await debugFile.lastModified()).millisecondsSinceEpoch;
      return debugTime > releaseTime ? debugFile.path : releaseFile.path;
    } else if (releaseExists) {
      return releaseFile.path;
    } else if (debugExists) {
      return debugFile.path;
    }

    return null;
  }
}
