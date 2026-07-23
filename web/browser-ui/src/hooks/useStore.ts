import { useState, useCallback } from 'react';
import type { SharedFile, ClipboardEntry, Device, SharedFolderItem, SharedFolderConfig, WsEvent } from '../types';

const API = '';

export function useStore() {
  const [files, setFiles] = useState<SharedFile[]>([]);
  const [clipboard, setClipboard] = useState<ClipboardEntry[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [sharedFolderItems, setSharedFolderItems] = useState<SharedFolderItem[]>([]);
  const [sharedFolderConfig, setSharedFolderConfig] = useState<SharedFolderConfig | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchSharedFolder = useCallback(async () => {
    try {
      const [filesRes, configRes] = await Promise.all([
        fetch(`${API}/api/shared-folder/files`),
        fetch(`${API}/api/shared-folder/config`),
      ]);
      if (filesRes.ok) {
        setSharedFolderItems((await filesRes.json()) as SharedFolderItem[]);
      }
      if (configRes.ok) {
        setSharedFolderConfig((await configRes.json()) as SharedFolderConfig);
      }
    } catch (e) {
      console.error('Failed to fetch shared folder data', e);
    }
  }, []);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    try {
      const [filesRes, clipRes, devicesRes] = await Promise.all([
        fetch(`${API}/api/files`),
        fetch(`${API}/api/clipboard`),
        fetch(`${API}/api/devices`),
      ]);
      setFiles((await filesRes.json()) as SharedFile[]);
      setClipboard((await clipRes.json()) as ClipboardEntry[]);
      setDevices((await devicesRes.json()) as Device[]);
      await fetchSharedFolder();
    } catch (e) {
      console.error('Failed to fetch initial data', e);
    } finally {
      setLoading(false);
    }
  }, [fetchSharedFolder]);

  const handleWsEvent = useCallback((event: WsEvent) => {
    switch (event.type) {
      case 'clipboard_updated':
        setClipboard(event.items);
        break;
      case 'file_shared':
        setFiles((prev) => [event.file, ...prev.filter((f) => f.id !== event.file.id)]);
        break;
      case 'file_revoked':
        setFiles((prev) => prev.filter((f) => f.id !== event.id));
        break;
      case 'device_joined':
      case 'device_updated':
        setDevices((prev) => [event.device, ...prev.filter((d) => d.id !== event.device.id)]);
        break;
      case 'device_left':
        setDevices((prev) => prev.filter((d) => d.id !== event.device_id));
        break;
      case 'shared_folder_changed':
        fetchSharedFolder();
        break;
      case 'shared_folder_config_updated':
        setSharedFolderConfig(event.config);
        fetchSharedFolder();
        break;
      case 'pong':
        break;
    }
  }, [fetchSharedFolder]);

  const revokeFile = useCallback(async (id: string) => {
    await fetch(`/api/files/${id}`, { method: 'DELETE' });
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }, []);

  const pushClipboard = useCallback(async (text: string) => {
    await fetch('/api/clipboard', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text }),
    });
  }, []);

  const deleteSharedFolderFile = useCallback(async (filename: string) => {
    await fetch(`/api/shared-folder/file/${encodeURIComponent(filename)}`, { method: 'DELETE' });
    fetchSharedFolder();
  }, [fetchSharedFolder]);

  const uploadToSharedFolder = useCallback(async (files: FileList | File[]) => {
    const formData = new FormData();
    for (let i = 0; i < files.length; i++) {
      formData.append('file', files[i]);
    }
    await fetch('/api/shared-folder/upload', {
      method: 'POST',
      body: formData,
    });
    fetchSharedFolder();
  }, [fetchSharedFolder]);

  return {
    files,
    clipboard,
    devices,
    sharedFolderItems,
    sharedFolderConfig,
    loading,
    fetchAll,
    fetchSharedFolder,
    handleWsEvent,
    revokeFile,
    pushClipboard,
    deleteSharedFolderFile,
    uploadToSharedFolder,
  };
}

