import { Monitor, Wifi } from 'lucide-react';
import { formatTime, getDeviceInitial } from '../utils';
import type { useStore } from '../hooks/useStore';
import Sparkline from '../components/Sparkline';
import { useMemo } from 'react';

type StoreType = ReturnType<typeof useStore>;

function getDeviceColor(name: string): string {
  const colors = [
    'var(--grad-purple)',
    'var(--grad-cyan)',
    'var(--grad-pink)',
    'var(--grad-green)',
    'var(--grad-orange)',
  ];
  const hash = name.split('').reduce((acc, c) => acc + c.charCodeAt(0), 0);
  return colors[hash % colors.length];
}

export default function DevicesPage({ store }: { store: StoreType }) {
  const activityData = useMemo(
    () => Array.from({ length: 10 }, (_, i) => Math.max(0, store.devices.length - (9 - i))),
    [store.devices.length]
  );

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Devices</h1>
        <p className="page-subtitle">All devices seen on the local network</p>
      </div>

      {/* Overview card */}
      <div className="stats-banner purple">
        <div className="stats-banner-glow" />
        <div className="stats-content">
          <div className="stats-item">
            <div className="stats-label">Network Activity</div>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
              <span className="stats-number">{store.devices.length}</span>
              <span style={{ fontSize: 13, color: 'var(--text-secondary)' }}>nodes active</span>
            </div>
          </div>
        </div>
        <div className="stats-chart">
          <Sparkline data={activityData} color="#818CF8" height={40} />
        </div>
        <div className="stats-badge-icon">
          <Wifi size={22} color="var(--accent-3)" />
        </div>
      </div>

      {store.devices.length === 0 ? (
        <div className="card">
          <div className="empty-state" style={{ paddingTop: 60, paddingBottom: 60 }}>
            <Monitor size={40} />
            <p>No devices detected yet</p>
            <p style={{ fontSize: 12, opacity: 0.6 }}>
              Open lynqo on another device on the same Wi-Fi
            </p>
          </div>
        </div>
      ) : (
        <div className="device-list">
          {store.devices.map((d) => (
            <div className="device-item" key={d.id} id={`device-${d.id}`}>
              <div
                className="device-avatar"
                style={{ background: getDeviceColor(d.name) }}
              >
                {getDeviceInitial(d.name)}
              </div>
              <div className="device-info">
                <div className="device-name">{d.name}</div>
                <div className="device-meta">
                  {d.ip_address ?? 'unknown IP'}
                  {d.user_agent ? ` · ${d.user_agent.slice(0, 60)}` : ''}
                </div>
                <div style={{ marginTop: 4, fontSize: 10.5, color: 'var(--text-muted)' }}>
                  Last seen {formatTime(d.last_seen)}
                </div>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 6, flexShrink: 0 }}>
                <div className="device-badge">● Online</div>
                <div className="online-indicator" />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
