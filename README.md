# lynqo

> A local-first productivity platform that turns any desktop into a secure local server for instant file sharing, clipboard synchronization, and device communication over the same Wi-Fi network.

---

## Features

- **Local File Sharing**: Select a file in the desktop app, and it will generate a streamable local URL (served directly from disk, no file duplication/copying).
- **Clipboard History**: Watches the system clipboard on desktop and serves it to all connected local clients.
- **Browser-to-Desktop Paste**: Clients can push text directly from their browsers to the host desktop's system clipboard.
- **mDNS Auto-Discovery**: Registers `_lynqo._tcp.local` so clients can connect without typing IP addresses.
- **Clean Local Dashboard**: Served from the embedded Rust binary at `/`.

---

## Getting Started (Immediate Run)

We have compiled the high-performance release server and created a double-clickable launcher script in the project root:

1. Open Finder at: `/Users/mahesh/Desktop/Projects/backend/lynqo`
2. Double-click the file: **`run-lynqo.command`**

This will:
- Spin up the local-first Rust server.
- Automatically open your default browser to the control panel at **`http://localhost:7432`**.
- Keep running in the background to sync your clipboard history and serve files.

---

## Developer Guide

If you install Xcode in the future, you can build the native macOS desktop container:

```bash
cd apps/desktop
flutter run
```
