import { useState } from 'react';
import { Clipboard, Send, TrendingUp } from 'lucide-react';
import { formatTime, copyToClipboard } from '../utils';
import type { useStore } from '../hooks/useStore';
import Sparkline from '../components/Sparkline';
import { useMemo } from 'react';

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

  const clipSpark = useMemo(
    () => Array.from({ length: 8 }, (_, i) => Math.max(0, store.clipboard.length - (7 - i))),
    [store.clipboard.length]
  );

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
    <div>
      <div className="page-header">
        <h1 className="page-title">Clipboard</h1>
        <p className="page-subtitle">History synced in real-time from your desktop</p>
      </div>

      {/* Stats banner */}
      <div className="stats-banner cyan">
        <div className="stats-banner-glow" />
        <div className="stats-content">
          <div className="stats-item">
            <div className="stats-label">Total History</div>
            <div className="stats-number">{store.clipboard.length} items</div>
          </div>
        </div>
        <div className="stats-chart">
          <Sparkline data={clipSpark} color="#22D3EE" height={40} />
        </div>
        <div className="stats-badge-icon">
          <TrendingUp size={22} color="var(--cyan-2)" />
        </div>
      </div>

      {/* Push to desktop */}
      <div className="card section">
        <div className="card-header">
          <span className="card-title">Push to Desktop</span>
        </div>
        <div className="clipboard-push-form">
          <textarea
            id="clipboard-push-input"
            placeholder="Type or paste text to send to your desktop..."
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handlePush();
            }}
          />
          <button
            id="clipboard-push-btn"
            className="btn btn-primary"
            onClick={handlePush}
            disabled={!text.trim()}
          >
            <Send size={14} />
            Send to Desktop
          </button>
        </div>
      </div>

      {/* History */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">History ({store.clipboard.length})</span>
        </div>
        {store.clipboard.length === 0 ? (
          <div className="empty-state">
            <Clipboard size={28} />
            <p>No clipboard history yet</p>
            <p style={{ fontSize: 12, opacity: 0.6 }}>Copy something on your desktop to see it here</p>
          </div>
        ) : (
          <div className="clipboard-list">
            {store.clipboard.map((c) => {
              const isImg = c.content_type?.startsWith('image/') || c.content.startsWith('data:image/');
              return (
                <div
                  className={`clip-item ${isImg ? 'clip-item-image' : ''}`}
                  key={c.id}
                  id={`clip-${c.id}`}
                  onClick={() => handleCopy(c.content, c.id)}
                  title="Click to copy"
                >
                  <div className="clip-content">
                    {isImg ? (
                      <img
                        src={c.content}
                        alt="Clipboard data"
                        style={{ maxHeight: '120px', borderRadius: '6px', objectFit: 'contain' }}
                      />
                    ) : (
                      c.content
                    )}
                  </div>
                  <div className="clip-meta">
                    <span className="clip-source">
                      <span className={`clip-source-badge ${c.source}`}>{c.source}</span>
                      <span style={{ marginLeft: 6, opacity: 0.6, color: 'var(--text-muted)' }}>{formatTime(c.created_at)}</span>
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
  );
}
