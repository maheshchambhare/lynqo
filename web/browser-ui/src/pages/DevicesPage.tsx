import { Monitor, Wifi, ShieldCheck, Laptop } from 'lucide-react';
import { formatTime } from '../utils';
import type { useStore } from '../hooks/useStore';

type StoreType = ReturnType<typeof useStore>;

export default function DevicesPage({ store }: { store: StoreType }) {
  return (
    <div className="page-container">
      {/* Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">
            <Monitor size={22} style={{ color: 'var(--accent-apple-blue)' }} />
            Connected LAN Devices
          </h2>
          <p className="page-subtitle">Discovered Mesh Nodes on the Local Wi-Fi Network</p>
        </div>
      </div>

      {/* Devices Grid */}
      <div className="folder-section-title">Active Mesh Nodes ({store.devices.length})</div>
      {store.devices.length === 0 ? (
        <div className="empty-state-box">
          <Wifi size={44} style={{ margin: '0 auto 0.75rem auto', opacity: 0.35, color: 'var(--text-muted)' }} />
          <p className="empty-state-title">No other devices detected</p>
          <p className="empty-state-sub">Open Lynqo on another phone, tablet, or PC on the same Wi-Fi network to connect automatically</p>
        </div>
      ) : (
        <div className="finder-grid">
          {store.devices.map((d) => (
            <div
              key={d.id}
              style={{
                background: 'var(--bg-card)',
                borderRadius: '16px',
                border: '1px solid var(--border-subtle)',
                padding: '1.25rem',
                boxShadow: 'var(--shadow-card)',
                display: 'flex',
                flexDirection: 'column',
                justifyContent: 'space-between',
                height: '160px',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div
                  style={{
                    width: '38px',
                    height: '38px',
                    borderRadius: '10px',
                    background: 'linear-gradient(135deg, #007AFF, #5856D6)',
                    color: '#FFF',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                >
                  <Laptop size={20} />
                </div>
                <span
                  style={{
                    background: 'rgba(52, 199, 89, 0.12)',
                    color: '#34C759',
                    fontSize: '0.68rem',
                    fontWeight: 700,
                    padding: '3px 8px',
                    borderRadius: '12px',
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                  }}
                >
                  <ShieldCheck size={12} /> Verified
                </span>
              </div>

              <div>
                <h3 style={{ fontSize: '0.95rem', fontWeight: 700, color: 'var(--text-primary)', marginBottom: '2px' }}>
                  {d.name}
                </h3>
                <p style={{ fontSize: '0.78rem', color: 'var(--text-muted)', fontFamily: 'monospace' }}>
                  {d.ip_address ?? 'LAN Mesh'}
                </p>
                <p style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginTop: '4px' }}>
                  Last active: {formatTime(d.last_seen)}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
