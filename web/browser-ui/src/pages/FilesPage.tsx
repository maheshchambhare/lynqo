import { useState, useRef } from 'react';
import { Files, Download, Trash2, Upload, Play, RefreshCw, FileText, Image as ImageIcon } from 'lucide-react';
import type { useStore } from '../hooks/useStore';
import type { SharedFile } from '../types';

interface FilesPageProps {
  store: ReturnType<typeof useStore>;
}

export default function FilesPage({ store }: FilesPageProps) {
  const { files, revokeFile, fetchAll } = store;
  const [dragOver, setDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [previewFile, setPreviewFile] = useState<SharedFile | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const uploadFiles = async (selectedFiles: FileList | File[]) => {
    setUploading(true);
    try {
      for (let i = 0; i < selectedFiles.length; i++) {
        const file = selectedFiles[i];
        const formData = new FormData();
        formData.append('file', file);
        await fetch('/api/files/upload', {
          method: 'POST',
          body: formData,
        });
      }
      fetchAll();
    } catch (err) {
      console.error('Failed to upload file:', err);
    } finally {
      setUploading(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      uploadFiles(e.dataTransfer.files);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      uploadFiles(e.target.files);
    }
  };

  return (
    <div className="page-container">
      {/* Hidden File Input */}
      <input
        type="file"
        ref={fileInputRef}
        onChange={handleFileSelect}
        style={{ display: 'none' }}
        multiple
      />

      {/* Page Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">
            <Files size={22} style={{ color: 'var(--accent-apple-blue)' }} />
            All Shared Files
          </h2>
          <p className="page-subtitle">P2P File Shares Active Across LAN Mesh Nodes</p>
        </div>

        <div className="header-actions">
          <button className="btn-apple-secondary" onClick={() => fetchAll()}>
            <RefreshCw size={14} /> Refresh
          </button>
          <button className="btn-apple-primary" onClick={() => fileInputRef.current?.click()}>
            <Upload size={14} /> {uploading ? 'Uploading...' : 'Share File'}
          </button>
        </div>
      </div>

      {/* Drag & Drop Upload Zone */}
      <div
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
        className={`upload-drop-zone ${dragOver ? 'drag-over' : ''}`}
      >
        <Upload size={28} style={{ color: 'var(--accent-apple-blue)', marginBottom: '6px' }} />
        <p className="drop-zone-text">
          {uploading ? 'Uploading and sharing files across network...' : 'Drag & drop files here to share across connected devices'}
        </p>
      </div>

      {/* Files Grid */}
      <div className="folder-section-title">Active Network Files ({files.length})</div>
      {files.length === 0 ? (
        <div className="empty-state-box">
          <Files size={44} style={{ margin: '0 auto 0.75rem auto', opacity: 0.35, color: 'var(--text-muted)' }} />
          <p className="empty-state-title">No files shared yet</p>
          <p className="empty-state-sub">Click "Share File" above or drop files into the application to broadcast</p>
        </div>
      ) : (
        <div className="finder-grid">
          {files.map((file) => {
            const isImage = file.mime_type?.startsWith('image/');
            const isVideo = file.mime_type?.startsWith('video/');

            return (
              <div key={file.id} className="apple-media-card" onClick={() => setPreviewFile(file)}>
                {isVideo && (
                  <div className="video-duration-tag">
                    <Play size={10} fill="#FFF" /> Media
                  </div>
                )}

                {isImage || isVideo ? (
                  <img src={`/api/files/${file.id}`} alt={file.file_name} loading="lazy" />
                ) : (
                  <div className="generic-file-preview">
                    {file.file_name.endsWith('.pdf') ? (
                      <FileText size={42} style={{ color: 'var(--tag-red)' }} />
                    ) : (
                      <ImageIcon size={42} style={{ color: 'var(--accent-apple-blue)' }} />
                    )}
                    <span className="generic-file-size">{formatSize(file.file_size)}</span>
                  </div>
                )}

                <div className="media-card-overlay">
                  <div className="media-filename">
                    <span>{file.file_name}</span>
                  </div>
                  <span className="media-filesize-sub">{formatSize(file.file_size)} · {file.download_count} downloads</span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Preview Modal */}
      {previewFile && (
        <div className="preview-modal-backdrop" onClick={() => setPreviewFile(null)}>
          <div className="preview-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="preview-modal-header">
              <h3 className="preview-modal-title">{previewFile.file_name}</h3>
              <div className="preview-modal-actions">
                <button
                  className="btn-apple-secondary"
                  onClick={async () => {
                    const pwd = prompt('Optional: Set a password for this public link (leave blank for public access):');
                    const maxD = prompt('Optional: Maximum download limit (leave blank for unlimited):');
                    const expH = prompt('Optional: Expiration in hours (leave blank for no expiration):');
                    
                    try {
                      const res = await fetch(`/api/files/${previewFile.id}/public-share`, {
                        method: 'POST',
                        headers: { 'content-type': 'application/json' },
                        body: JSON.stringify({
                          password: pwd ? pwd.trim() : null,
                          max_downloads: maxD ? parseInt(maxD) : null,
                          expires_hours: expH ? parseInt(expH) : null,
                        }),
                      });
                      const data = await res.json();
                      if (data.token) {
                        const fullUrl = data.public_url || `https://share.lynqo.app/public/s/${data.token}`;
                        await navigator.clipboard.writeText(fullUrl);
                        alert(`Public link created & copied to clipboard!\n\nURL: ${fullUrl}${pwd ? '\nPassword Protected: Yes' : ''}`);
                      }
                    } catch (err) {
                      alert('Failed to generate public share link.');
                    }
                  }}
                >
                  🌐 Create Public Link
                </button>
                <a
                  href={`/api/files/${previewFile.id}`}
                  download={previewFile.file_name}
                  className="btn-apple-primary"
                  style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}
                >
                  <Download size={14} /> Download ({formatSize(previewFile.file_size)})
                </a>
                <button
                  className="btn-apple-secondary"
                  style={{ color: 'var(--tag-red)' }}
                  onClick={() => {
                    if (confirm(`Revoke share for ${previewFile.file_name}?`)) {
                      revokeFile(previewFile.id);
                      setPreviewFile(null);
                    }
                  }}
                >
                  <Trash2 size={14} /> Revoke
                </button>
                <button className="preview-modal-close" onClick={() => setPreviewFile(null)}>✕</button>
              </div>
            </div>
            <div className="preview-modal-body">
              {previewFile.mime_type?.startsWith('image/') ? (
                <img src={`/api/files/${previewFile.id}`} alt={previewFile.file_name} className="modal-media-element" />
              ) : previewFile.mime_type?.startsWith('video/') ? (
                <video src={`/api/files/${previewFile.id}`} controls className="modal-media-element" />
              ) : (
                <iframe src={`/api/files/${previewFile.id}`} title={previewFile.file_name} className="modal-iframe-element" />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
