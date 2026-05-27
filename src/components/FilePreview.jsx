import { useState, useEffect } from 'react'
import { s3 } from '../bridge'

function formatSize(bytes) {
  if (!bytes && bytes !== 0) return '—'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`
}

function formatDate(d) {
  if (!d) return '—'
  return new Date(d).toLocaleString('en-US', {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', hour12: false,
  })
}

function indentXml(xml) {
  let result = ''
  let depth = 0
  const lines = xml.replace(/>\s*</g, '>\n<').split('\n')
  for (const line of lines) {
    const t = line.trim()
    if (!t) continue
    if (t.startsWith('</')) depth = Math.max(0, depth - 1)
    result += '  '.repeat(depth) + t + '\n'
    if (t.startsWith('<') && !t.startsWith('</') && !t.startsWith('<?') &&
        !t.startsWith('<!') && !t.includes('/>') && !/<.+<\/.+>/.test(t)) {
      depth++
    }
  }
  return result.trim()
}

export default function FilePreview({ preview, onClose, width }) {
  const { bucket, key, name, url, type, size, modified, ext } = preview
  const [textContent, setTextContent] = useState(null)
  const [textLoading, setTextLoading] = useState(false)
  const [textError, setTextError] = useState(null)
  const [highlighted, setHighlighted] = useState(null)
  // Task 5: image dimensions
  const [imgDims, setImgDims] = useState(null)

  // Task 5: reset dims when file changes
  useEffect(() => { setImgDims(null) }, [key])

  useEffect(() => {
    if (type !== 'text') return
    setTextContent(null)
    setTextError(null)
    setTextLoading(true)
    s3.getObjectText(bucket, key)
      .then(raw => setTextContent(ext === 'xml' ? indentXml(raw) : raw))
      .catch(e => setTextError(typeof e === 'string' ? e : e.message || 'Failed to load'))
      .finally(() => setTextLoading(false))
  }, [bucket, key, type, ext])

  // Syntax highlighting via Shiki
  useEffect(() => {
    if (!textContent) { setHighlighted(null); return }
    const fileExt = key?.split('.').pop()?.toLowerCase() || ''
    const langMap = {
      js: 'javascript', jsx: 'jsx', ts: 'typescript', tsx: 'tsx',
      json: 'json', yaml: 'yaml', yml: 'yaml', xml: 'xml',
      html: 'html', css: 'css', md: 'markdown', sh: 'bash',
      py: 'python', rs: 'rust', go: 'go', java: 'java',
    }
    const lang = langMap[fileExt]
    if (!lang) { setHighlighted(null); return }

    import('shiki').then(({ createHighlighter }) => {
      createHighlighter({
        themes: ['github-dark'],
        langs: [lang],
      }).then(h => {
        const html = h.codeToHtml(textContent, { lang, theme: 'github-dark' })
        setHighlighted(html)
        h.dispose()
      }).catch(() => setHighlighted(null))
    }).catch(() => setHighlighted(null))
  }, [textContent, key])

  const [downloadProgress, setDownloadProgress] = useState(null)

  const handleDownload = async () => {
    const unlisten = await s3.onDownloadProgress(p => {
      if (p.key === key) setDownloadProgress(p)
    })
    try {
      await s3.saveObject(bucket, key)
    } finally {
      unlisten()
      setDownloadProgress(null)
    }
  }

  const handleOpenExternal = async () => {
    const unlisten = await s3.onDownloadProgress(p => {
      if (p.key === key) setDownloadProgress(p)
    })
    try {
      await s3.openObject(bucket, key)
    } finally {
      unlisten()
      setDownloadProgress(null)
    }
  }

  return (
    <div className="preview-panel" style={width ? { width } : undefined}>
      <div className="preview-header">
        <span className="preview-title" title={name}>{name}</span>
        <button className="preview-close" onClick={onClose}>×</button>
      </div>

      <div className="preview-body">
        {type === 'image' && (
          <img
            className="preview-img"
            src={url}
            alt={name}
            onLoad={e => setImgDims({ w: e.target.naturalWidth, h: e.target.naturalHeight })}
            onError={e => { e.target.style.display = 'none' }}
          />
        )}
        {type === 'pdf' && (
          <iframe className="preview-iframe" src={url} title={name} />
        )}
        {type === 'text' && (
          textLoading ? (
            <div style={{ padding: 20, color: 'var(--text-2)', fontSize: 11 }}>
              <span className="spinner" />Loading…
            </div>
          ) : textError ? (
            <div style={{ padding: 16, color: 'var(--red)', fontSize: 11 }}>{textError}</div>
          ) : highlighted ? (
            <div className="preview-highlighted" dangerouslySetInnerHTML={{ __html: highlighted }} />
          ) : (
            <pre className="preview-text">{textContent}</pre>
          )
        )}
        {type === 'binary' && (
          <div className="preview-unsupported">
            <div className="preview-unsupported-icon">◫</div>
            <div style={{ fontSize: 12, color: 'var(--text-1)' }}>.{ext} files cannot be previewed</div>
            <div style={{ fontSize: 11 }}>Download to open locally</div>
          </div>
        )}
      </div>

      <div className="preview-meta">
        <div className="meta-row">
          <span className="meta-key">SIZE</span>
          <span className="meta-val">{formatSize(size)}</span>
        </div>
        <div className="meta-row">
          <span className="meta-key">MODIFIED</span>
          <span className="meta-val">{formatDate(modified)}</span>
        </div>
        {/* Task 5: image dimensions row */}
        {imgDims && (
          <div className="meta-row">
            <span className="meta-key">DIMENSIONS</span>
            <span className="meta-val">{imgDims.w} × {imgDims.h} px</span>
          </div>
        )}
        <div className="meta-row">
          <span className="meta-key">KEY</span>
          <span className="meta-val"
            style={{ maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', textAlign: 'right' }}
            title={key}>
            {key}
          </span>
        </div>
      </div>

      {downloadProgress && (
        <div className="download-progress">
          <div className="download-progress-bar" style={{
            width: downloadProgress.total_bytes > 0
              ? `${Math.round(downloadProgress.bytes_received / downloadProgress.total_bytes * 100)}%`
              : '100%'
          }} />
          <span className="download-progress-text">
            {downloadProgress.total_bytes > 0
              ? `${Math.round(downloadProgress.bytes_received / downloadProgress.total_bytes * 100)}%`
              : formatSize(downloadProgress.bytes_received)}
          </span>
        </div>
      )}

      <div className="preview-actions">
        <button className="btn-primary" onClick={handleDownload} style={{ flex: 1 }}>↓ Download</button>
        <button className="btn-ghost" onClick={handleOpenExternal}>↗ Open</button>
      </div>
    </div>
  )
}
