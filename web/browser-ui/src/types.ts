export interface SharedFile {
  id: string;
  file_path: string;
  file_name: string;
  file_size: number;
  mime_type: string | null;
  created_at: number;
  expires_at: number | null;
  download_count: number;
  revoked: boolean;
}

export interface ClipboardEntry {
  id: string;
  content: string;
  content_type: string;
  source: string;
  created_at: number;
  is_favorite: boolean;
  category: string;
  ocr_text: string | null;
  metadata_json: string | null;
  hash: string;
}

export interface Device {
  id: string;
  name: string;
  user_agent: string | null;
  ip_address: string | null;
  last_seen: number;
  is_trusted: boolean;
  created_at: number;
  battery_level: number | null;
  storage_remaining_bytes: number | null;
  connection_quality: number | null;
  latency_ms: number | null;
  color_theme: string | null;
  avatar_url: string | null;
  group_name: string | null;
  room_name: string | null;
}

export interface TransferTask {
  id: string;
  file_id: string | null;
  file_name: string | null;
  device_id: string;
  action: string;
  status: string;
  transferred_bytes: number;
  total_bytes: number;
  created_at: number;
}

export interface SharedFolderItem {
  name: string;
  relative_path: string;
  file_size: number;
  mime_type: string | null;
  is_dir: boolean;
  modified_at: number;
}

export interface SharedFolderConfig {
  path: string | null;
  is_active: boolean;
}

export type WsEvent =
  | { type: 'clipboard_updated'; items: ClipboardEntry[] }
  | { type: 'file_shared'; file: SharedFile }
  | { type: 'file_revoked'; id: string }
  | { type: 'device_joined'; device: Device }
  | { type: 'device_left'; device_id: string }
  | { type: 'device_updated'; device: Device }
  | { type: 'transfer_started'; task: TransferTask }
  | { type: 'transfer_progress'; task: TransferTask }
  | { type: 'transfer_completed'; task: TransferTask }
  | { type: 'transfer_failed'; task: TransferTask }
  | { type: 'shared_folder_changed' }
  | { type: 'shared_folder_config_updated'; config: SharedFolderConfig }
  | { type: 'pong' };

