import { useState, useCallback } from 'react';
import type { SharedFile, ClipboardEntry, Device, WsEvent } from '../types';

const API = '';

export function useStore() {
  const [files, setFiles] = useState<SharedFile[]>([]);
  const [clipboard, setClipboard] = useState<ClipboardEntry[]>([]);
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);

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
    } catch (e) {
      console.error('Failed to fetch initial data', e);
    } finally {
      setLoading(false);
    }
  }, []);

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
      case 'pong':
        break;
    }
  }, []);

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

  return { files, clipboard, devices, loading, fetchAll, handleWsEvent, revokeFile, pushClipboard };
}
