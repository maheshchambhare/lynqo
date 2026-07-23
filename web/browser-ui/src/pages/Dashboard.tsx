import { useState } from 'react';
import { Files, Clipboard, Monitor, Download, Check } from 'lucide-react';
import { formatBytes, formatTime, copyToClipboard } from '../utils';
import type { useStore } from '../hooks/useStore';

type StoreType = ReturnType<typeof useStore>;

export default function Dashboard({ store }: { store: StoreType }) {
  const [copied, setCopied] = useState<string | null>(null);
  const recentFiles = store.files.slice(0, 4);
  const recentClip = store.clipboard.slice(0, 4);
  const totalDownloads = store.files.reduce((s, f) => s + f.download_count, 0);

  const handleCopy = async (content: string, id: string) => {
    const ok = await copyToClipboard(content);
    if (ok) {
      setCopied(id);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  const statCards = [
    { label: 'Shared Files', value: store.files.length.toString(), icon: Files, color: 'var(--accent-apple-blue)' },
    { label: 'Clipboard Items', value: store.clipboard.length.toString(), icon: Clipboard, color: 'var(--accent-folder-yellow)' },
    { label: 'Active Devices', value: store.devices.length.toString(), icon: Monitor, color: '#34C759' },
    { label: 'Total Downloads', value: totalDownloads.toString(), icon: Download, color: '#FF9500' },
  ];

  return (
    <div className="page-container">
      {/* Page Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">
            <Monitor size={22} style={{ color: 'var(--accent-apple-blue)' }} />
            Dashboard
          </h2>
          <p className="page-subtitle">Overview of your local network activity</p>
        </div>
      </div>

      {/* Apple macOS Stat Cards Bar */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: '1.25rem', marginBottom: '2rem' }}>
        {statCards.map(({ label, value, icon: Icon, color }) => (
          <div
            key={label}
            style={{
              background: 'var(--bg-card)',
              borderRadius: '16px',
              padding: '1.25rem',
              border: '1px solid var(--border-subtle)',
              boxShadow: 'var(--shadow-card)',
              display: 'flex',
              alignItems: 'center',
              gap: '1rem',
            }}
          >
            <div
              style={{
                width: '42px',
                height: '42px',
                borderRadius: '12px',
                background: color,
                color: '#FFFFFF',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <Icon size={20} />
            </div>
            <div>
              <div style={{ fontSize: '1.5rem', fontWeight: 800, color: 'var(--text-primary)', lineHeight: 1 }}>{value}</div>
              <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)', marginTop: '4px', fontWeight: 500 }}>{label}</div>
            </div>
          </div>
        ))}
      </div>

      {/* Grid of Recent Content */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: '1.5rem', marginBottom: '2rem' }}>
        {/* Recent Files Card */}
        <div style={{ background: 'var(--bg-card)', borderRadius: '18px', border: '1px solid var(--border-subtle)', padding: '1.25rem', boxShadow: 'var(--shadow-card)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
            <h3 style={{ fontSize: '0.95rem', fontWeight: 700, color: 'var(--text-primary)' }}>Recent Shared Files</h3>
            <span style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>{store.files.length} total</span>
          </div>

          {recentFiles.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '2rem 1rem', color: 'var(--text-muted)', fontSize: '0.85rem' }}>
              No files shared yet
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {recentFiles.map((f) => (
                <div
                  key={f.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '8px 12px',
                    borderRadius: '10px',
                    background: 'var(--bg-card-subtle)',
                    border: '1px solid var(--border-subtle)',
                  }}
                >
                  <div style={{ overflow: 'hidden', paddingRight: '12px' }}>
                    <div style={{ fontSize: '0.85rem', fontWeight: 600, color: 'var(--text-primary)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                      {f.file_name}
                    </div>
                    <div style={{ fontSize: '0.725rem', color: 'var(--text-muted)' }}>
                      {formatBytes(f.file_size)} · {formatTime(f.created_at)}
                    </div>
                  </div>
                  <a
                    href={`/api/files/${f.id}`}
                    download={f.file_name}
                    className="btn-apple-primary"
                    style={{ fontSize: '0.75rem', padding: '6px 10px' }}
                  >
                    <Download size={12} />
                  </a>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Recent Clipboard Card */}
        <div style={{ background: 'var(--bg-card)', borderRadius: '18px', border: '1px solid var(--border-subtle)', padding: '1.25rem', boxShadow: 'var(--shadow-card)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
            <h3 style={{ fontSize: '0.95rem', fontWeight: 700, color: 'var(--text-primary)' }}>Recent Clipboard</h3>
            <span style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>{store.clipboard.length} items</span>
          </div>

          {recentClip.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '2rem 1rem', color: 'var(--text-muted)', fontSize: '0.85rem' }}>
              No clipboard history yet
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {recentClip.map((c) => {
                const isImg = c.content_type?.startsWith('image/') || c.content.startsWith('data:image/');
                return (
                  <div
                    key={c.id}
                    onClick={() => handleCopy(c.content, c.id)}
                    style={{
                      padding: '10px 12px',
                      borderRadius: '10px',
                      background: 'var(--bg-card-subtle)',
                      border: '1px solid var(--border-subtle)',
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      gap: '8px',
                    }}
                  >
                    <div style={{ overflow: 'hidden', flex: 1 }}>
                      {isImg ? (
                        <img src={c.content} alt="clip" style={{ height: '36px', borderRadius: '4px', objectFit: 'contain' }} />
                      ) : (
                        <span style={{ fontSize: '0.825rem', color: 'var(--text-primary)', fontFamily: 'monospace', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', display: 'block' }}>
                          {c.content}
                        </span>
                      )}
                    </div>
                    <span style={{ fontSize: '0.7rem', fontWeight: 600, color: copied === c.id ? '#34C759' : 'var(--accent-apple-blue)', display: 'flex', alignItems: 'center', gap: '3px' }}>
                      {copied === c.id ? <><Check size={12} /> Copied</> : 'Copy'}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
