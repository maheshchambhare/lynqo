class ClipboardEntry {
  final String id;
  final String content;
  final String contentType;
  final String source;
  final int createdAt;
  final bool isFavorite;
  final String category;
  final String? ocrText;
  final String? metadataJson;
  final String hash;

  ClipboardEntry({
    required this.id,
    required this.content,
    required this.contentType,
    required this.source,
    required this.createdAt,
    required this.isFavorite,
    required this.category,
    this.ocrText,
    this.metadataJson,
    required this.hash,
  });

  factory ClipboardEntry.fromJson(Map<String, dynamic> json) {
    return ClipboardEntry(
      id: json['id'] as String,
      content: json['content'] as String,
      contentType: json['content_type'] as String? ?? 'text/plain',
      source: json['source'] as String? ?? 'desktop',
      createdAt: json['created_at'] as int,
      isFavorite: json['is_favorite'] as bool? ?? false,
      category: json['category'] as String? ?? 'text',
      ocrText: json['ocr_text'] as String?,
      metadataJson: json['metadata_json'] as String?,
      hash: json['hash'] as String? ?? '',
    );
  }
}

class SharedFile {
  final String id;
  final String filePath;
  final String fileName;
  final int fileSize;
  final String? mimeType;
  final int createdAt;
  final int? expiresAt;
  final int downloadCount;
  final bool revoked;

  SharedFile({
    required this.id,
    required this.filePath,
    required this.fileName,
    required this.fileSize,
    this.mimeType,
    required this.createdAt,
    this.expiresAt,
    required this.downloadCount,
    required this.revoked,
  });

  factory SharedFile.fromJson(Map<String, dynamic> json) {
    return SharedFile(
      id: json['id'] as String,
      filePath: json['file_path'] as String,
      fileName: json['file_name'] as String,
      fileSize: json['file_size'] as int,
      mimeType: json['mime_type'] as String?,
      createdAt: json['created_at'] as int,
      expiresAt: json['expires_at'] as int?,
      downloadCount: json['download_count'] as int? ?? 0,
      revoked: json['revoked'] as bool? ?? false,
    );
  }
}

class Device {
  final String id;
  final String name;
  final String? userAgent;
  final String? ipAddress;
  final int lastSeen;
  final bool isTrusted;
  final int createdAt;
  final int? batteryLevel;
  final int? storageRemainingBytes;
  final int? connectionQuality;
  final int? latencyMs;
  final String? colorTheme;
  final String? avatarUrl;
  final String? groupName;
  final String? roomName;

  Device({
    required this.id,
    required this.name,
    this.userAgent,
    this.ipAddress,
    required this.lastSeen,
    required this.isTrusted,
    required this.createdAt,
    this.batteryLevel,
    this.storageRemainingBytes,
    this.connectionQuality,
    this.latencyMs,
    this.colorTheme,
    this.avatarUrl,
    this.groupName,
    this.roomName,
  });

  factory Device.fromJson(Map<String, dynamic> json) {
    return Device(
      id: json['id'] as String,
      name: json['name'] as String,
      userAgent: json['user_agent'] as String?,
      ipAddress: json['ip_address'] as String?,
      lastSeen: json['last_seen'] as int,
      isTrusted: json['is_trusted'] as bool? ?? false,
      createdAt: json['created_at'] as int,
      batteryLevel: json['battery_level'] as int?,
      storageRemainingBytes: json['storage_remaining_bytes'] as int?,
      connectionQuality: json['connection_quality'] as int?,
      latencyMs: json['latency_ms'] as int?,
      colorTheme: json['color_theme'] as String?,
      avatarUrl: json['avatar_url'] as String?,
      groupName: json['group_name'] as String?,
      roomName: json['room_name'] as String?,
    );
  }
}

class TransferTask {
  final String id;
  final String? fileId;
  final String? fileName;
  final String deviceId;
  final String action; // "upload" | "download"
  final String status; // "pending" | "transferring" | "paused" | "completed" | "failed"
  final int transferredBytes;
  final int totalBytes;
  final int createdAt;

  TransferTask({
    required this.id,
    this.fileId,
    this.fileName,
    required this.deviceId,
    required this.action,
    required this.status,
    required this.transferredBytes,
    required this.totalBytes,
    required this.createdAt,
  });

  factory TransferTask.fromJson(Map<String, dynamic> json) {
    return TransferTask(
      id: json['id'] as String,
      fileId: json['file_id'] as String?,
      fileName: json['file_name'] as String?,
      deviceId: json['device_id'] as String,
      action: json['action'] as String? ?? 'download',
      status: json['status'] as String? ?? 'pending',
      transferredBytes: json['transferred_bytes'] as int? ?? 0,
      totalBytes: json['total_bytes'] as int? ?? 0,
      createdAt: json['created_at'] as int? ?? 0,
    );
  }
}

