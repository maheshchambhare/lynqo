import { useEffect, useState } from 'react';
import { HardDrive, Files, Clipboard, Monitor, Settings, Sun, Moon, RefreshCw } from 'lucide-react';
import { useWebSocket } from './hooks/useWebSocket';
import { useStore } from './hooks/useStore';
import Dashboard from './pages/Dashboard';
import FilesPage from './pages/FilesPage';
import SharedFolderPage from './pages/SharedFolderPage';
import ClipboardPage from './pages/ClipboardPage';
import DevicesPage from './pages/DevicesPage';
import './index.css';

type Page = 'shared_folder' | 'files' | 'clipboard' | 'devices' | 'dashboard';
const VALID_PAGES: Page[] = ['shared_folder', 'files', 'clipboard', 'devices', 'dashboard'];

function getInitialPage(): Page {
  // ponytail: native location.hash router, falls back to localStorage
  const hash = window.location.hash.replace('#', '') as Page;
  if (VALID_PAGES.includes(hash)) return hash;
  const saved = localStorage.getItem('lynqo_active_page') as Page;
  if (VALID_PAGES.includes(saved)) return saved;
  return 'shared_folder';
}

export default function App() {
  const [page, setPageInternal] = useState<Page>(getInitialPage);
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    return (localStorage.getItem('lynqo_theme') as 'light' | 'dark') || 'light';
  });
  const store = useStore();
  const { send } = useWebSocket(store.handleWsEvent);

  const setPage = (newPage: Page) => {
    setPageInternal(newPage);
    window.location.hash = newPage;
    localStorage.setItem('lynqo_active_page', newPage);
  };

  useEffect(() => {
    const handleHashChange = () => {
      const hash = window.location.hash.replace('#', '') as Page;
      if (VALID_PAGES.includes(hash)) {
        setPageInternal(hash);
        localStorage.setItem('lynqo_active_page', hash);
      }
    };

    if (!window.location.hash && page) {
      window.location.hash = page;
    }

    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, [page]);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('lynqo_theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === 'light' ? 'dark' : 'light'));
  };

  useEffect(() => {
    store.fetchAll();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pages: Record<Page, React.ReactElement> = {
    shared_folder: <SharedFolderPage store={store} />,
    files: <FilesPage store={store} />,
    clipboard: <ClipboardPage store={store} send={send} />,
    devices: <DevicesPage store={store} />,
    dashboard: <Dashboard store={store} />,
  };

  const navItems: Array<{ id: Page; label: string; icon: React.ComponentType<{ size?: number }> }> = [
    { id: 'shared_folder', label: 'Shared Storage', icon: HardDrive },
    { id: 'files', label: 'All Files', icon: Files },
    { id: 'clipboard', label: 'Clipboard', icon: Clipboard },
    { id: 'devices', label: 'Devices', icon: Monitor },
    { id: 'dashboard', label: 'Settings', icon: Settings },
  ];

  const currentPageItem = navItems.find((item) => item.id === page);

  return (
    <div className="app-shell">
      {/* Desktop Navigation Sidebar (≥ 768px) */}
      <aside className="app-sidebar">
        <div className="sidebar-brand">
          <div className="brand-logo-mark">L</div>
          <span className="brand-title">lynqo</span>
        </div>

        <nav className="sidebar-nav-list">
          {navItems.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={`sidebar-item ${page === id ? 'active' : ''}`}
              onClick={() => setPage(id)}
            >
              <Icon size={16} />
              <span>{label}</span>
              {id === 'devices' && store.devices.length > 0 && (
                <span className="nav-badge">
                  {store.devices.length}
                </span>
              )}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer">
          <span className="status-indicator-dot" />
          <span>Server Active · Port 7432</span>
        </div>
      </aside>

      {/* Main Content Column */}
      <div className="app-content-col">
        {/* Responsive Top Bar */}
        <header className="app-topbar">
          <div className="topbar-left">
            <div className="mobile-brand-wrapper">
              <div className="brand-logo-mark mobile-logo">L</div>
              <span className="mobile-brand-title">lynqo</span>
            </div>
            <span className="topbar-active-page">{currentPageItem?.label}</span>
          </div>

          <div className="topbar-right">
            <button className="topbar-icon-btn" onClick={() => store.fetchAll()} title="Refresh Data">
              <RefreshCw size={16} />
            </button>
            <button className="topbar-icon-btn" onClick={toggleTheme} title={`Switch to ${theme === 'light' ? 'Dark' : 'Light'} Mode`}>
              {theme === 'light' ? <Moon size={16} /> : <Sun size={16} />}
            </button>
          </div>
        </header>

        {/* Viewport Area */}
        <main className="app-main-viewport">
          {pages[page]}
        </main>
      </div>

      {/* Mobile Bottom Navigation Bar (< 768px) */}
      <nav className="mobile-bottom-nav">
        {navItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className={`mobile-nav-btn ${page === id ? 'active' : ''}`}
            onClick={() => setPage(id)}
          >
            <Icon size={18} />
            <span>{label.split(' ')[0]}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
