import { useState } from 'react';
import { Clipboard, Send, Check } from 'lucide-react';
import { formatTime, copyToClipboard } from '../utils';
import type { useStore } from '../hooks/useStore';

type StoreType = ReturnType<typeof useStore>;

export default function ClipboardPage({
  store,
  send,
}: {
  store: StoreType;
  send: (data: object) => void;
}) {
  const [text, setText] = useState('');
  const [copied, setCopied] = useState<string | null>(null);

  const handleCopy = async (content: string, id: string) => {
    const ok = await copyToClipboard(content);
    if (ok) {
      setCopied(id);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  const handlePush = () => {
    if (!text.trim()) return;
    send({ type: 'clipboard_push', text });
    store.pushClipboard(text);
    setText('');
  };

  return (
    <div className="page-container">
      {/* Page Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">
            <Clipboard size={22} style={{ color: 'var(--accent-apple-blue)' }} />
            Clipboard Timeline
          </h2>
          <p className="page-subtitle">Real-time Clipboard Sync Across LAN Nodes</p>
        </div>
      </div>

      {/* Push to desktop card */}
      <div style={{ background: 'var(--bg-card)', padding: '1.25rem', borderRadius: '16px', border: '1px solid var(--border-subtle)', marginBottom: '1.5rem', boxShadow: 'var(--shadow-card)' }}>
        <h3 style={{ fontSize: '0.85rem', fontWeight: 600, color: 'var(--text-primary)', marginBottom: '10px' }}>Broadcast to Clipboard</h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
          <textarea
            placeholder="Type or paste text payload to broadcast to desktop clipboard..."
            value={text}
            onChange={(e) => setText(e.target.value)}
            style={{
              width: '100%',
              minHeight: '70px',
              padding: '10px 12px',
              borderRadius: '10px',
              border: '1px solid var(--border-subtle)',
              background: 'var(--bg-card-subtle)',
              color: 'var(--text-primary)',
              fontSize: '0.85rem',
              resize: 'vertical',
            }}
          />
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button
              className="btn-apple-primary"
              onClick={handlePush}
              disabled={!text.trim()}
              style={{ opacity: text.trim() ? 1 : 0.5 }}
            >
              <Send size={14} /> Send to Desktop
            </button>
          </div>
        </div>
      </div>

      {/* History Grid */}
      <div className="folder-section-title">Timeline History ({store.clipboard.length})</div>
      {store.clipboard.length === 0 ? (
        <div className="empty-state-box">
          <Clipboard size={44} style={{ margin: '0 auto 0.75rem auto', opacity: 0.35, color: 'var(--text-muted)' }} />
          <p className="empty-state-title">No clipboard history yet</p>
          <p className="empty-state-sub">Copy text or images on any device to view them in real time here</p>
        </div>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '1rem' }}>
          {store.clipboard.map((c) => {
            const isImg = c.content_type?.startsWith('image/') || c.content.startsWith('data:image/');
            return (
              <div
                key={c.id}
                onClick={() => handleCopy(c.content, c.id)}
                style={{
                  background: 'var(--bg-card)',
                  padding: '1rem',
                  borderRadius: '14px',
                  border: '1px solid var(--border-subtle)',
                  boxShadow: 'var(--shadow-card)',
                  cursor: 'pointer',
                  display: 'flex',
                  flexDirection: 'column',
                  justifyContent: 'space-between',
                  gap: '10px',
                  transition: 'transform 0.2s ease',
                }}
              >
                <div>
                  {isImg ? (
                    <img
                      src={c.content}
                      alt="Clipboard item"
                      style={{ maxHeight: '140px', width: '100%', objectFit: 'contain', borderRadius: '8px' }}
                    />
                  ) : (
                    <p style={{ fontSize: '0.85rem', color: 'var(--text-primary)', fontFamily: 'monospace', wordBreak: 'break-word', display: '-webkit-box', WebkitLineClamp: 4, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>
                      {c.content}
                    </p>
                  )}
                </div>

                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingTop: '8px', borderTop: '1px solid var(--border-subtle)' }}>
                  <span style={{ fontSize: '0.7rem', color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <span style={{ background: c.source === 'desktop' ? 'var(--accent-apple-blue)' : 'var(--accent-folder-yellow)', color: '#FFF', padding: '2px 6px', borderRadius: '4px', fontWeight: 600, textTransform: 'uppercase', fontSize: '0.65rem' }}>
                      {c.source}
                    </span>
                    {formatTime(c.created_at)}
                  </span>
                  <span style={{ fontSize: '0.75rem', fontWeight: 600, color: copied === c.id ? '#34C759' : 'var(--accent-apple-blue)', display: 'flex', alignItems: 'center', gap: '4px' }}>
                    {copied === c.id ? <><Check size={12} /> Copied</> : 'Click to Copy'}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
