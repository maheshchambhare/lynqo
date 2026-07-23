import { useState, useMemo } from 'react';
import { Files, Download, Trash2, Upload, Eye, X, HardDrive } from 'lucide-react';
import { formatBytes, formatTime, getFileExt } from '../utils';
import type { useStore } from '../hooks/useStore';
import Sparkline from '../components/Sparkline';

type StoreType = ReturnType<typeof useStore>;

const isImage = (fileName: string) => {
  const ext = fileName.split('.').pop()?.toLowerCase();
  return ext ? ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(ext) : false;
};

const isVideo = (fileName: string) => {
  const ext = fileName.split('.').pop()?.toLowerCase();
  return ext ? ['mp4', 'webm', 'ogg', 'mov', 'm4v'].includes(ext) : false;
};

const isMedia = (fileName: string) => {
  const ext = fileName.split('.').pop()?.toLowerCase();
  return ext ? ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'mp4', 'webm', 'ogg', 'mov', 'm4v'].includes(ext) : false;
};

export default function FilesPage({ store }: { store: StoreType }) {
  const [dragging, setDragging] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [previewFile, setPreviewFile] = useState<any>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const toggleSelect = (id: string) => {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    setSelectedIds(next);
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === store.files.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(store.files.map((f) => f.id)));
    }
  };

  const downloadSelected = () => {
    selectedIds.forEach((id) => {
      const f = store.files.find((file) => file.id === id);
      if (!f) return;
      const link = document.createElement('a');
      link.href = `/api/files/${id}`;
      link.setAttribute('download', f.file_name);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    });
  };

  const revokeSelected = async () => {
    if (confirm(`Are you sure you want to revoke share for ${selectedIds.size} files?`)) {
      const ids = Array.from(selectedIds);
      setSelectedIds(new Set());
      for (const id of ids) {
        await store.revokeFile(id);
      }
    }
  };

  const uploadFiles = async (files: FileList) => {
    setUploading(true);
    try {
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        const formData = new FormData();
        formData.append('file', file);
        const res = await fetch('/api/files/upload', {
          method: 'POST',
          body: formData,
        });
        if (!res.ok) {
          throw new Error(`Upload failed for ${file.name}`);
        }
      }
    } catch (err) {
      console.error(err);
      alert('Failed to upload/share one or more files');
    } finally {
      setUploading(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      uploadFiles(e.dataTransfer.files);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      uploadFiles(e.target.files);
    }
  };

  const totalSize = store.files.reduce((s, f) => s + f.file_size, 0);
  const totalDownloads = store.files.reduce((s, f) => s + f.download_count, 0);
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const filesSpark = useMemo(
    () => Array.from({ length: 8 }, (_, i) => Math.max(0, store.files.length - (7 - i))),
    [store.files.length]
  );

  return (
    <div>
      <div className="page-header">
        <h1 className="page-title">Shared Files</h1>
        <p className="page-subtitle">Files shared across devices</p>
      </div>

      {/* Stats banner */}
      <div className="stats-banner purple">
        <div className="stats-banner-glow" />
        <div className="stats-content">
          <div className="stats-item">
            <div className="stats-label">Files Shared</div>
            <div className="stats-number">{store.files.length}</div>
          </div>
          <div className="stats-item">
            <div className="stats-label">Total Size</div>
            <div className="stats-number">{formatBytes(totalSize)}</div>
          </div>
          <div className="stats-item">
            <div className="stats-label">Downloads</div>
            <div className="stats-number">{totalDownloads}</div>
          </div>
        </div>
        <div className="stats-chart">
          <Sparkline data={filesSpark} color="#818CF8" height={40} />
        </div>
        <div className="stats-badge-icon">
          <HardDrive size={22} color="var(--accent-3)" />
        </div>
      </div>

      <input
        type="file"
        id="file-upload"
        multiple
        style={{ display: 'none' }}
        onChange={handleFileSelect}
      />

      <div
        className={`drop-zone ${dragging ? 'dragging' : ''} ${uploading ? 'uploading' : ''}`}
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
        onClick={() => document.getElementById('file-upload')?.click()}
        style={{ cursor: 'pointer' }}
      >
        <Upload size={32} />
        <p>{uploading ? 'Uploading/Sharing...' : 'Click or drag a file here to share'}</p>
        <p className="drop-zone-sub">Files are served directly from this network server</p>
      </div>

      {selectedIds.size > 0 && (
        <div className="bulk-action-bar">
          <div className="bulk-info">
            <input
              type="checkbox"
              checked={selectedIds.size === store.files.length}
              ref={(el) => {
                if (el) {
                  el.indeterminate = selectedIds.size > 0 && selectedIds.size < store.files.length;
                }
              }}
              onChange={toggleSelectAll}
            />
            <span>{selectedIds.size} files selected</span>
          </div>
          <div className="bulk-buttons">
            <button className="btn btn-accent btn-sm" onClick={downloadSelected}>
              <Download size={14} style={{ marginRight: 6 }} /> Download Selected
            </button>
            <button className="btn btn-danger btn-sm" onClick={revokeSelected}>
              <Trash2 size={14} style={{ marginRight: 6 }} /> Revoke Selected
            </button>
            <button className="btn btn-ghost btn-sm" onClick={() => setSelectedIds(new Set())}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {store.files.length === 0 ? (
        <div className="empty-state" style={{ marginTop: 40 }}>
          <Files size={40} />
          <p>No files are being shared</p>
          <p style={{ fontSize: 12, opacity: 0.6 }}>Select a file in the desktop app to share it here</p>
        </div>
      ) : (
        <div className="file-list">
          {store.files.map((f) => (
            <div
              className={`file-item ${selectedIds.has(f.id) ? 'selected' : ''}`}
              key={f.id}
              onClick={() => toggleSelect(f.id)}
              style={{ cursor: 'pointer' }}
            >
              <div className="file-checkbox-wrapper" onClick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  checked={selectedIds.has(f.id)}
                  onChange={() => toggleSelect(f.id)}
                />
              </div>
              <div className="file-icon" style={{ overflow: 'hidden', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                {isMedia(f.file_name) ? (
                  <img
                    src={`/api/files/${f.id}/thumbnail`}
                    alt="thumb"
                    style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: '4px' }}
                    onError={(e) => {
                      e.currentTarget.style.display = 'none';
                      const parent = e.currentTarget.parentElement;
                      if (parent && !parent.querySelector('.fallback-ext')) {
                        const txt = document.createElement('span');
                        txt.className = 'fallback-ext';
                        txt.innerText = getFileExt(f.file_name);
                        parent.appendChild(txt);
                      }
                    }}
                  />
                ) : (
                  getFileExt(f.file_name)
                )}
              </div>
              <div className="file-info">
                <div className="file-name">{f.file_name}</div>
                <div className="file-meta">
                  {formatBytes(f.file_size)}
                  {f.mime_type ? ` · ${f.mime_type}` : ''}
                  {' · '}{formatTime(f.created_at)}
                  {' · '}{f.download_count} downloads
                </div>
              </div>
              <div className="file-actions" onClick={(e) => e.stopPropagation()}>
                {(isImage(f.file_name) || isVideo(f.file_name)) && (
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => setPreviewFile(f)}
                    title="Preview"
                  >
                    <Eye size={14} />
                  </button>
                )}
                <a
                  id={`download-${f.id}`}
                  className="btn btn-ghost btn-sm"
                  href={`/api/files/${f.id}`}
                  download={f.file_name}
                  title="Download"
                >
                  <Download size={14} />
                </a>
                <button
                  id={`revoke-${f.id}`}
                  className="btn btn-danger btn-sm"
                  onClick={() => store.revokeFile(f.id)}
                  title="Revoke share"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {previewFile && (
        <div className="preview-modal" onClick={() => setPreviewFile(null)}>
          <div className="preview-content" onClick={(e) => e.stopPropagation()}>
            <button className="preview-close" onClick={() => setPreviewFile(null)}>
              <X size={20} />
            </button>
            <div className="preview-body">
              {isImage(previewFile.file_name) ? (
                <img src={`/api/files/${previewFile.id}`} alt={previewFile.file_name} />
              ) : (
                <video src={`/api/files/${previewFile.id}`} controls autoPlay />
              )}
            </div>
            <div className="preview-footer">
              <span className="preview-title">{previewFile.file_name}</span>
              <a
                className="btn btn-accent btn-sm"
                href={`/api/files/${previewFile.id}`}
                download={previewFile.file_name}
              >
                Download
              </a>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
