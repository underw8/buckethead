import { useState, useEffect, useCallback } from 'react'
import { s3 } from '../bridge'

const FILE_ICONS = {
  jpg: '🖼', jpeg: '🖼', png: '🖼', gif: '🖼', webp: '🖼', svg: '🖼', ico: '🖼',
  pdf: '📄', doc: '📝', docx: '📝', xls: '📊', xlsx: '📊', ppt: '📋', pptx: '📋',
  js: '⟨⟩', ts: '⟨⟩', jsx: '⟨⟩', tsx: '⟨⟩', py: '⟨⟩', rb: '⟨⟩', go: '⟨⟩',
  json: '⟨⟩', yaml: '⟨⟩', yml: '⟨⟩', toml: '⟨⟩', xml: '⟨⟩', html: '⟨⟩', css: '⟨⟩',
  txt: '≡', md: '≡', csv: '≡', log: '≡',
  zip: '◫', gz: '◫', tar: '◫', rar: '◫', '7z': '◫',
  mp4: '▶', mov: '▶', avi: '▶', mkv: '▶', webm: '▶',
  mp3: '♪', wav: '♪', flac: '♪', ogg: '♪',
}

function getExt(key) {
  const parts = key.split('.')
  return parts.length > 1 ? parts[parts.length - 1].toLowerCase() : ''
}

function getIcon(key) {
  return FILE_ICONS[getExt(key)] || '·'
}

function formatSize(bytes) {
  if (bytes === 0 || bytes == null) return '—'
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

function stripPrefix(key, prefix) {
  return key.startsWith(prefix) ? key.slice(prefix.length) : key
}

export default function ObjectBrowser({ bucket, prefix, onPrefixChange, onPreview }) {
  const [items, setItems] = useState({ folders: [], objects: [] })
  const [loading, setLoading] = useState(false)
  const [nextToken, setNextToken] = useState(null)
  const [loadingMore, setLoadingMore] = useState(false)
  const [sortKey, setSortKey] = useState('name')
  const [sortDir, setSortDir] = useState(1)
  const [error, setError] = useState(null)
  const [presignError, setPresignError] = useState(null)

  const load = useCallback(async (reset = true) => {
    if (reset) { setLoading(true); setItems({ folders: [], objects: [] }); setNextToken(null) }
    else setLoadingMore(true)
    setError(null)
    try {
      const token = reset ? undefined : nextToken
      const result = await s3.listObjects(bucket, prefix, token)
      if (reset) {
        setItems({ folders: result.folders, objects: result.objects })
      } else {
        setItems(prev => ({ folders: prev.folders, objects: [...prev.objects, ...result.objects] }))
      }
      setNextToken(result.truncated ? result.next_token : null)
    } catch (e) {
      setError(typeof e === 'string' ? e : e.message || 'Failed to list objects')
    } finally {
      setLoading(false)
      setLoadingMore(false)
    }
  }, [bucket, prefix, nextToken])

  useEffect(() => { load(true) }, [bucket, prefix])

  const handleSort = (key) => {
    if (sortKey === key) setSortDir(d => d * -1)
    else { setSortKey(key); setSortDir(1) }
  }

  const sortedObjects = [...items.objects].sort((a, b) => {
    if (sortKey === 'name') return sortDir * a.key.localeCompare(b.key)
    if (sortKey === 'size') return sortDir * ((a.size || 0) - (b.size || 0))
    if (sortKey === 'date') return sortDir * (new Date(a.modified) - new Date(b.modified))
    return 0
  })

  const breadcrumbs = () => {
    const parts = prefix.split('/').filter(Boolean)
    return [
      { label: bucket, prefix: '' },
      ...parts.map((p, i) => ({ label: p, prefix: parts.slice(0, i + 1).join('/') + '/' }))
    ]
  }

  const handleFileClick = async (obj) => {
    try {
      const url = await s3.presign(bucket, obj.key)
      const ext = getExt(obj.key)
      const isImage = ['jpg','jpeg','png','gif','webp','svg'].includes(ext)
      const isPdf = ext === 'pdf'
      const isText = ['txt','md','log','csv','json','yaml','yml','html','xml','css','js','ts'].includes(ext)
      onPreview({
        bucket,
        key: obj.key,
        name: stripPrefix(obj.key, prefix),
        url,
        type: isImage ? 'image' : isPdf ? 'pdf' : isText ? 'text' : 'binary',
        size: obj.size,
        modified: obj.modified,
        ext,
      })
      setPresignError(null)
    } catch (e) {
      console.error('Presign failed', e)
      setPresignError(e.message || String(e))
    }
  }

  const crumbs = breadcrumbs()
  const totalCount = items.folders.length + items.objects.length

  return (
    <div className="object-browser">
      <div className="browser-toolbar">
        <div className="breadcrumb">
          {crumbs.map((c, i) => (
            <span key={c.prefix} style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              {i > 0 && <span className="breadcrumb-sep">/</span>}
              <span
                className={`breadcrumb-item ${i === crumbs.length - 1 ? 'active' : ''}`}
                onClick={() => i < crumbs.length - 1 && onPrefixChange(c.prefix)}
              >
                {c.label}
              </span>
            </span>
          ))}
        </div>
        <div className="toolbar-right">
          {!loading && <span className="object-count">{totalCount} items</span>}
          <button className="btn-ghost" onClick={() => load(true)}>↺ Refresh</button>
        </div>
      </div>

      {presignError && (
        <div className="presign-error-banner">
          <span className="presign-error-text">Failed to generate preview link: {presignError}</span>
          <button className="presign-error-dismiss" onClick={() => setPresignError(null)}>✕</button>
        </div>
      )}

      <div className="file-table-wrap">
        {error && <div style={{ padding: 16, color: 'var(--red)', fontSize: 12 }}>{error}</div>}

        <table className="file-table">
          <thead>
            <tr>
              <th onClick={() => handleSort('name')}>Name {sortKey === 'name' ? (sortDir === 1 ? '↑' : '↓') : ''}</th>
              <th onClick={() => handleSort('size')}>Size {sortKey === 'size' ? (sortDir === 1 ? '↑' : '↓') : ''}</th>
              <th onClick={() => handleSort('date')}>Modified {sortKey === 'date' ? (sortDir === 1 ? '↑' : '↓') : ''}</th>
              <th>Type</th>
            </tr>
          </thead>
          <tbody>
            {loading && (
              <tr className="loading-row">
                <td colSpan={4}><span className="spinner" />Loading…</td>
              </tr>
            )}

            {!loading && items.folders.map(folder => {
              const name = stripPrefix(folder, prefix).replace(/\/$/, '')
              return (
                <tr key={folder} className="folder-row" onClick={() => onPrefixChange(folder)}>
                  <td>
                    <div className="file-name-cell">
                      <span className="file-type-icon" style={{ color: 'var(--blue)' }}>▷</span>
                      <span className="file-name-text">{name}/</span>
                    </div>
                  </td>
                  <td className="file-size">—</td>
                  <td className="file-date">—</td>
                  <td className="file-ext">folder</td>
                </tr>
              )
            })}

            {!loading && sortedObjects.map(obj => {
              const name = stripPrefix(obj.key, prefix)
              if (!name) return null
              const ext = getExt(obj.key)
              return (
                <tr key={obj.key} className="file-row" onClick={() => handleFileClick(obj)}>
                  <td>
                    <div className="file-name-cell">
                      <span className="file-type-icon">{getIcon(obj.key)}</span>
                      <span className="file-name-text">{name}</span>
                    </div>
                  </td>
                  <td className="file-size">{formatSize(obj.size)}</td>
                  <td className="file-date">{formatDate(obj.modified)}</td>
                  <td className="file-ext">{ext || '—'}</td>
                </tr>
              )
            })}

            {nextToken && !loading && (
              <tr className="load-more-row">
                <td colSpan={4}>
                  <button className="btn-load-more" onClick={() => load(false)} disabled={loadingMore}>
                    {loadingMore ? <><span className="spinner" />Loading…</> : 'Load more'}
                  </button>
                </td>
              </tr>
            )}

            {!loading && totalCount === 0 && (
              <tr>
                <td colSpan={4} style={{ textAlign: 'center', padding: 40, color: 'var(--text-2)' }}>
                  Empty prefix
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
