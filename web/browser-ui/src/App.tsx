import { useEffect, useState } from 'react';
import { LayoutDashboard, Files, Clipboard, Monitor } from 'lucide-react';
import { useWebSocket } from './hooks/useWebSocket';
import { useStore } from './hooks/useStore';
import Dashboard from './pages/Dashboard';
import FilesPage from './pages/FilesPage';
import ClipboardPage from './pages/ClipboardPage';
import DevicesPage from './pages/DevicesPage';
import './index.css';

type Page = 'dashboard' | 'files' | 'clipboard' | 'devices';

const NAV = [
  { id: 'dashboard' as Page, label: 'Dashboard', icon: LayoutDashboard },
  { id: 'files' as Page, label: 'Files', icon: Files },
  { id: 'clipboard' as Page, label: 'Clipboard', icon: Clipboard },
  { id: 'devices' as Page, label: 'Devices', icon: Monitor },
];

export default function App() {
  const [page, setPage] = useState<Page>('dashboard');
  const store = useStore();
  const { send } = useWebSocket(store.handleWsEvent);

  useEffect(() => {
    store.fetchAll();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pages: Record<Page, React.ReactElement> = {
    dashboard: <Dashboard store={store} />,
    files: <FilesPage store={store} />,
    clipboard: <ClipboardPage store={store} send={send} />,
    devices: <DevicesPage store={store} />,
  };

  return (
    <div className="app">
      {/* Decorative orb field */}
      <div className="orb-field" aria-hidden="true">
        <div className="orb orb-1" />
        <div className="orb orb-2" />
        <div className="orb orb-3" />
        <div className="orb orb-4" />
      </div>

      {/* Mobile Header */}
      <header className="mobile-header">
        <div className="mobile-logo">
          <div className="logo-mark">L</div>
          <span className="logo-text">lynqo</span>
        </div>
        <div className="mobile-status">
          <span className="status-dot" />
          <span className="status-text">Active</span>
        </div>
      </header>

      {/* Desktop Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-logo">
          <div className="logo-mark">L</div>
          <span className="logo-text">lynqo</span>
        </div>

        <nav className="nav">
          {NAV.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              id={`nav-${id}`}
              className={`nav-item ${page === id ? 'active' : ''}`}
              onClick={() => setPage(id)}
            >
              <Icon size={16} />
              {label}
              {id === 'devices' && store.devices.length > 0 && (
                <span className="nav-badge">{store.devices.length}</span>
              )}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="status-row">
            <span className="status-dot" />
            <span className="status-text">Server running · lynqo.local:7432</span>
          </div>
        </div>
      </aside>

      <main className="main">{pages[page]}</main>

      {/* Mobile Bottom Tab Bar */}
      <nav className="mobile-nav">
        {NAV.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            id={`mobile-nav-${id}`}
            className={`mobile-nav-item ${page === id ? 'active' : ''}`}
            onClick={() => setPage(id)}
          >
            <Icon size={20} />
            <span className="mobile-nav-label">{label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
