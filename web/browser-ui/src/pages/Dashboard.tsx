import { useState, useMemo } from 'react';
import { Files, Clipboard, Monitor, Download } from 'lucide-react';
import { formatBytes, formatTime, getFileExt, copyToClipboard } from '../utils';
import Sparkline from '../components/Sparkline';
import type { useStore } from '../hooks/useStore';

type StoreType = ReturnType<typeof useStore>;

function buildSparkline(count: number): number[] {
  // Simulate a growing usage chart seeded on count
  const pts = 8;
  const base = Math.max(count - 4, 0);
  return Array.from({ length: pts }, (_, i) =>
    i < pts - 1 ? base + Math.floor(Math.random() * 3) : count
  );
}

export default function Dashboard({ store }: { store: StoreType }) {
  const [copied, setCopied] = useState<string | null>(null);
  const recentFiles = store.files.slice(0, 4);
  const recentClip = store.clipboard.slice(0, 5);

  // Stable sparkline seeds (recalc only when counts change)
  const filesSpark = useMemo(() => buildSparkline(store.files.length), [store.files.length]);
  const clipSpark = useMemo(() => buildSparkline(store.clipboard.length), [store.clipboard.length]);
  const devSpark = useMemo(() => buildSparkline(store.devices.length), [store.devices.length]);
  const dlSpark = useMemo(() => {
    const total = store.files.reduce((s, f) => s + f.download_count, 0);
    return buildSparkline(total);
  }, [store.files]);

  const totalDownloads = store.files.reduce((s, f) => s + f.download_count, 0);

  const handleCopy = async (content: string, id: string) => {
    const ok = await copyToClipboard(content);
    if (ok) {
      setCopied(id);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">Overview of your local network activity</p>
      </div>

      {/* Stat cards */}
      <div className="stat-grid">
        <div className="stat-card purple">
          <div className="stat-card-glow" />
          <div className="stat-icon"><Files size={16} /></div>
          <div className="stat-value">{store.files.length}</div>
          <div className="stat-label">Shared Files</div>
          <div className="sparkline-wrap">
            <Sparkline data={filesSpark} color="#818CF8" />
          </div>
        </div>
        <div className="stat-card cyan">
          <div className="stat-card-glow" />
          <div className="stat-icon"><Clipboard size={16} /></div>
          <div className="stat-value">{store.clipboard.length}</div>
          <div className="stat-label">Clipboard Items</div>
          <div className="sparkline-wrap">
            <Sparkline data={clipSpark} color="#22D3EE" />
          </div>
        </div>
        <div className="stat-card pink">
          <div className="stat-card-glow" />
          <div className="stat-icon"><Monitor size={16} /></div>
          <div className="stat-value">{store.devices.length}</div>
          <div className="stat-label">Active Devices</div>
          <div className="sparkline-wrap">
            <Sparkline data={devSpark} color="#EC4899" />
          </div>
        </div>
        <div className="stat-card green">
          <div className="stat-card-glow" />
          <div className="stat-icon"><Download size={16} /></div>
          <div className="stat-value">{totalDownloads}</div>
          <div className="stat-label">Total Downloads</div>
          <div className="sparkline-wrap">
            <Sparkline data={dlSpark} color="#10B981" />
          </div>
        </div>
      </div>

      {/* Recent content */}
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <span className="card-title">Recent Files</span>
            <span className="card-action">{store.files.length} total</span>
          </div>
          {recentFiles.length === 0 ? (
            <div className="empty-state">
              <Files size={28} />
              <p>No files shared yet</p>
            </div>
          ) : (
            <div className="file-list">
              {recentFiles.map((f) => (
                <div className="file-item" key={f.id}>
                  <div className="file-icon">{getFileExt(f.file_name)}</div>
                  <div className="file-info">
                    <div className="file-name">{f.file_name}</div>
                    <div className="file-meta">
                      {formatBytes(f.file_size)} · {formatTime(f.created_at)}
                    </div>
                  </div>
                  <a
                    className="btn btn-ghost btn-sm"
                    href={`/api/files/${f.id}`}
                    download={f.file_name}
                    title="Download"
                  >
                    <Download size={13} />
                  </a>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">Recent Clipboard</span>
            <span className="card-action">{store.clipboard.length} items</span>
          </div>
          {recentClip.length === 0 ? (
            <div className="empty-state">
              <Clipboard size={28} />
              <p>No clipboard history yet</p>
            </div>
          ) : (
            <div className="clipboard-list">
              {recentClip.map((c) => {
                const isImg = c.content_type?.startsWith('image/') || c.content.startsWith('data:image/');
                return (
                  <div
                    className={`clip-item ${isImg ? 'clip-item-image' : ''}`}
                    key={c.id}
                    onClick={() => handleCopy(c.content, c.id)}
                    title="Click to copy"
                  >
                    <div className="clip-content">
                      {isImg ? (
                        <img
                          src={c.content}
                          alt="Clipboard payload"
                          style={{ maxHeight: '110px', borderRadius: '6px', objectFit: 'contain' }}
                        />
                      ) : (
                        c.content
                      )}
                    </div>
                    <div className="clip-meta">
                      <span className="clip-source">
                        <span className={`clip-source-badge ${c.source}`}>{c.source}</span>
                      </span>
                      <span className="clip-copy-hint">
                        {copied === c.id ? '✓ copied' : 'copy'}
                      </span>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Connected devices strip */}
      {store.devices.length > 0 && (
        <div className="card section">
          <div className="card-header">
            <span className="card-title">
              <span className="status-dot" style={{ display: 'inline-block', marginRight: 8, verticalAlign: 'middle' }} />
              Connected Devices
            </span>
            <span className="card-action">{store.devices.length} online</span>
          </div>
          <div className="devices-mini">
            {store.devices.map((d) => (
              <div className="device-mini-card" key={d.id}>
                <div className="device-mini-avatar">{d.name.charAt(0).toUpperCase()}</div>
                <div>
                  <div className="device-mini-name">{d.name}</div>
                  <div className="device-mini-meta">{d.ip_address ?? 'unknown'}</div>
                </div>
                <div className="device-mini-dot" />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
