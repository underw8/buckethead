import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import PropTypes from 'prop-types'
import { s3, aws } from '../bridge'

const LOCALE_MAP = { en: 'en-US', vi: 'vi-VN', ja: 'ja-JP' }

function formatSize(bytes) {
  if (!bytes && bytes !== 0) return '—'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`
}

function formatDate(d, lang) {
  if (!d) return '—'
  return new Date(d).toLocaleString(LOCALE_MAP[lang] || 'en-US', {
    year: 'numeric', month: 'short', day: 'numeric',
    hour: '2-digit', minute: '2-digit', hour12: false,
  })
}

function indentXml(xml) {
  let result = ''
  let depth = 0
  const lines = xml.replaceAll(/>\s*</g, '>\n<').split('\n')
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
  const { t, i18n } = useTranslation()
  const { bucket, key, name, url, type, size, modified, ext } = preview
  const [textContent, setTextContent] = useState(null)
  const [textLoading, setTextLoading] = useState(false)
  const [textError, setTextError] = useState(null)
  const [highlighted, setHighlighted] = useState(null)
  // Task 5: image dimensions
  const [imgDims, setImgDims] = useState(null)
  // HeadObject metadata
  const [meta, setMeta] = useState(null)
  const [metaLoading, setMetaLoading] = useState(false)
  const [metaExpanded, setMetaExpanded] = useState(false)

  // Task 5: reset dims when file changes
  useEffect(() => { setImgDims(null) }, [key])

  useEffect(() => {
    if (type !== 'text') return
    setTextContent(null)
    setTextError(null)
    setTextLoading(true)
    s3.getObjectText(bucket, key)
      .then(raw => setTextContent(ext === 'xml' ? indentXml(raw) : raw))
      .catch(e => setTextError(typeof e === 'string' ? e : e.message || t('preview.failed_to_load')))
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

  // Fetch HeadObject metadata whenever bucket/key changes
  useEffect(() => {
    setMeta(null)
    setMetaExpanded(false)
    setMetaLoading(true)
    aws.headObject(bucket, key)
      .then(m => setMeta(m))
      .catch(() => setMeta(null))
      .finally(() => setMetaLoading(false))
  }, [bucket, key])

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

  const expandIcon = metaExpanded ? '▾' : '▸'

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
        {type === 'text' && (() => {
          if (textLoading) return <div style={{ padding: 20, color: 'var(--text-2)', fontSize: 11 }}><span className="spinner" />{t('preview.loading')}</div>
          if (textError) return <div style={{ padding: 16, color: 'var(--red)', fontSize: 11 }}>{textError}</div>
          if (highlighted) return <div className="preview-highlighted" dangerouslySetInnerHTML={{ __html: highlighted }} />
          return <pre className="preview-text">{textContent}</pre>
        })()}
        {type === 'binary' && (
          <div className="preview-unsupported">
            <div className="preview-unsupported-icon">◫</div>
            <div style={{ fontSize: 12, color: 'var(--text-1)' }}>{t('preview.cannot_preview', { ext })}</div>
            <div style={{ fontSize: 11 }}>{t('preview.download_hint')}</div>
          </div>
        )}
      </div>

      <div className="preview-meta">
        <div className="meta-row">
          <span className="meta-key">{t('preview.meta_size')}</span>
          <span className="meta-val">{formatSize(size)}</span>
        </div>
        <div className="meta-row">
          <span className="meta-key">{t('preview.meta_modified')}</span>
          <span className="meta-val">{formatDate(modified, i18n.language)}</span>
        </div>
        {/* Task 5: image dimensions row */}
        {imgDims && (
          <div className="meta-row">
            <span className="meta-key">{t('preview.meta_dimensions')}</span>
            <span className="meta-val">{t('preview.dimensions_value', { w: imgDims.w, h: imgDims.h })}</span>
          </div>
        )}
        <div className="meta-row">
          <span className="meta-key">{t('preview.meta_key')}</span>
          <span className="meta-val"
            style={{ maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', textAlign: 'right' }}
            title={key}>
            {key}
          </span>
        </div>

        {/* Object Metadata expandable section */}
        <div className="meta-section-toggle">
          <button
            className="meta-expand-btn"
            onClick={() => setMetaExpanded(v => !v)}
            disabled={metaLoading && !meta}
          >
            {metaLoading && !meta ? t('preview.metadata_loading') : `${expandIcon} ${t('preview.metadata')}`}
          </button>
        </div>

        {metaExpanded && meta && (
          <div className="meta-extra-rows">
            {meta.content_type && (
              <div className="meta-row">
                <span className="meta-key">{t('preview.meta_content_type')}</span>
                <span className="meta-val">{meta.content_type}</span>
              </div>
            )}
            {meta.storage_class && (
              <div className="meta-row">
                <span className="meta-key">{t('preview.meta_storage_class')}</span>
                <span className="meta-val">{meta.storage_class}</span>
              </div>
            )}
            {meta.cache_control && (
              <div className="meta-row">
                <span className="meta-key">{t('preview.meta_cache_control')}</span>
                <span className="meta-val">{meta.cache_control}</span>
              </div>
            )}
            {meta.content_encoding && (
              <div className="meta-row">
                <span className="meta-key">{t('preview.meta_encoding')}</span>
                <span className="meta-val">{meta.content_encoding}</span>
              </div>
            )}
            {meta.user_meta && meta.user_meta.length > 0 && meta.user_meta.map(([k, v]) => (
              <div className="meta-row" key={k}>
                <span className="meta-key" title={t('preview.meta_user_prefix', { k })}>{k}</span>
                <span className="meta-val">{v}</span>
              </div>
            ))}
          </div>
        )}
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
        <button className="btn-primary" onClick={handleDownload} style={{ flex: 1 }}>{t('preview.download')}</button>
        <button className="btn-ghost" onClick={handleOpenExternal}>{t('preview.open')}</button>
      </div>
    </div>
  )
}

FilePreview.propTypes = {
  preview: PropTypes.shape({
    bucket: PropTypes.string.isRequired,
    key: PropTypes.string.isRequired,
    name: PropTypes.string,
    url: PropTypes.string,
    type: PropTypes.string.isRequired,
    size: PropTypes.number,
    modified: PropTypes.string,
    ext: PropTypes.string,
  }).isRequired,
  onClose: PropTypes.func.isRequired,
  width: PropTypes.number,
}
