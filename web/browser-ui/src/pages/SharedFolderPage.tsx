import { useState, useRef, useEffect } from 'react';
import { Upload, Download, Trash2, Folder, Play, RefreshCw, HardDrive, FileText, Image as ImageIcon } from 'lucide-react';
import type { useStore } from '../hooks/useStore';
import type { SharedFolderItem } from '../types';

interface SharedFolderPageProps {
  store: ReturnType<typeof useStore>;
}

export default function SharedFolderPage({ store }: SharedFolderPageProps) {
  const { sharedFolderItems, sharedFolderConfig, uploadToSharedFolder, deleteSharedFolderFile, fetchSharedFolder } = store;
  const [dragOver, setDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [previewItem, setPreviewItem] = useState<SharedFolderItem | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchSharedFolder();
  }, [fetchSharedFolder]);

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      setUploading(true);
      await uploadToSharedFolder(e.dataTransfer.files);
      setUploading(false);
    }
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      setUploading(true);
      await uploadToSharedFolder(e.target.files);
      setUploading(false);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const getPreviewUrl = (filename: string) => {
    return `/api/shared-folder/file/${encodeURIComponent(filename)}`;
  };

  const folders = sharedFolderItems.filter((i) => i.is_dir);
  const mediaAndFiles = sharedFolderItems.filter((i) => !i.is_dir);

  return (
    <div className="page-container">
      {/* Hidden File Input for Upload */}
      <input
        type="file"
        ref={fileInputRef}
        onChange={handleFileSelect}
        style={{ display: 'none' }}
        multiple
      />

      {/* Header bar */}
      <div className="page-header">
        <div>
          <h2 className="page-title">
            <HardDrive size={22} style={{ color: 'var(--accent-folder-yellow)' }} />
            Central Storage Hub
          </h2>
          <p className="page-subtitle">
            {sharedFolderConfig?.path ? (
              <span>Host Folder: <code>{sharedFolderConfig.path}</code></span>
            ) : (
              'Central LAN Shared Directory'
            )}
          </p>
        </div>

        <div className="header-actions">
          <button className="btn-apple-secondary" onClick={() => fetchSharedFolder()}>
            <RefreshCw size={14} /> Refresh
          </button>
          <button className="btn-apple-primary" onClick={() => fileInputRef.current?.click()}>
            <Upload size={14} /> {uploading ? 'Uploading...' : 'Upload File'}
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
          {uploading ? 'Uploading files to host OS folder...' : 'Drag & drop files here to upload into central shared storage'}
        </p>
      </div>

      {/* Sub-Folders Section (if any on disk) */}
      {folders.length > 0 && (
        <>
          <div className="folder-section-title">Folders</div>
          <div className="finder-grid">
            {folders.map((folder, idx) => (
              <div key={folder.name} className="apple-folder-card">
                <div className={`folder-top-tab ${idx % 3 === 0 ? '' : idx % 3 === 1 ? 'gray' : 'blue'}`} />
                <div className="folder-card-body">
                  <div className="folder-card-title">{folder.name}</div>
                  {folder.file_size > 0 && (
                    <div className="folder-card-meta">↑{formatSize(folder.file_size)}</div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Files & Media Section */}
      <div className="folder-section-title">Shared Files & Media ({mediaAndFiles.length})</div>
      {mediaAndFiles.length === 0 ? (
        <div className="empty-state-box">
          <Folder size={44} style={{ margin: '0 auto 0.75rem auto', opacity: 0.35, color: 'var(--text-muted)' }} />
          <p className="empty-state-title">Shared storage is currently empty</p>
          <p className="empty-state-sub">Upload files using the button above or drop files into the host OS shared folder</p>
        </div>
      ) : (
        <div className="finder-grid">
          {mediaAndFiles.map((file) => {
            const isImage = file.mime_type?.startsWith('image/');
            const isVideo = file.mime_type?.startsWith('video/');

            return (
              <div key={file.name} className="apple-media-card" onClick={() => setPreviewItem(file)}>
                {isVideo && (
                  <div className="video-duration-tag">
                    <Play size={10} fill="#FFF" /> Media
                  </div>
                )}

                {isImage || isVideo ? (
                  <img src={getPreviewUrl(file.name)} alt={file.name} loading="lazy" />
                ) : (
                  <div className="generic-file-preview">
                    {file.name.endsWith('.pdf') ? (
                      <FileText size={42} style={{ color: 'var(--tag-red)' }} />
                    ) : (
                      <ImageIcon size={42} style={{ color: 'var(--accent-apple-blue)' }} />
                    )}
                    <span className="generic-file-size">{formatSize(file.file_size)}</span>
                  </div>
                )}

                <div className="media-card-overlay">
                  <div className="media-filename">
                    <span>{file.name}</span>
                  </div>
                  <span className="media-filesize-sub">{formatSize(file.file_size)}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Media Preview & Download Modal */}
      {previewItem && (
        <div className="preview-modal-backdrop" onClick={() => setPreviewItem(null)}>
          <div className="preview-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="preview-modal-header">
              <h3 className="preview-modal-title">{previewItem.name}</h3>
              <div className="preview-modal-actions">
                <a
                  href={getPreviewUrl(previewItem.name)}
                  download={previewItem.name}
                  className="btn-apple-primary"
                  style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}
                >
                  <Download size={14} /> Download ({formatSize(previewItem.file_size)})
                </a>
                <button
                  className="btn-apple-secondary"
                  style={{ color: 'var(--tag-red)' }}
                  onClick={() => {
                    if (confirm(`Delete ${previewItem.name}?`)) {
                      deleteSharedFolderFile(previewItem.name);
                      setPreviewItem(null);
                    }
                  }}
                >
                  <Trash2 size={14} />
                </button>
                <button className="preview-modal-close" onClick={() => setPreviewItem(null)}>✕</button>
              </div>
            </div>
            <div className="preview-modal-body">
              {previewItem.mime_type?.startsWith('image/') ? (
                <img src={getPreviewUrl(previewItem.name)} alt={previewItem.name} className="modal-media-element" />
              ) : previewItem.mime_type?.startsWith('video/') ? (
                <video src={getPreviewUrl(previewItem.name)} controls className="modal-media-element" />
              ) : (
                <iframe src={getPreviewUrl(previewItem.name)} title={previewItem.name} className="modal-iframe-element" />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
