import { useState, useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import ProfileSelector from './components/ProfileSelector'
import BucketList from './components/BucketList'
import ObjectBrowser from './components/ObjectBrowser'
import FilePreview from './components/FilePreview'
import './app.css'

const LANGS = [
  { code: 'en', label: 'EN' },
  { code: 'vi', label: 'VI' },
  { code: 'ja', label: 'JA' },
]

function migrateLocalStorage() {
  const oldPrefix = 'thathoo:';
  const newPrefix = 'buckethead:';
  for (const key of Object.keys(localStorage)) {
    if (key.startsWith(oldPrefix)) {
      const newKey = newPrefix + key.slice(oldPrefix.length);
      if (!localStorage.getItem(newKey)) {
        localStorage.setItem(newKey, localStorage.getItem(key));
      }
      localStorage.removeItem(key);
    }
  }
}
migrateLocalStorage()

const THEMES = [
  { id: 'default',        label: 'Default',        color: '#f5a623' },
  { id: 'night-owl',      label: 'Night Owl',       color: '#82aaff' },
  { id: 'solarized-dark', label: 'Solarized Dark',  color: '#268bd2' },
  { id: 'solarized-light',label: 'Solarized Light', color: '#b58900' },
  { id: 'dracula',        label: 'Dracula',         color: '#bd93f9' },
]

export default function App() {
  const { t, i18n } = useTranslation()

  const handleLangChange = (code) => {
    i18n.changeLanguage(code)
    localStorage.setItem('buckethead:lang', code)
  }

  const [stage, setStage] = useState('profile')
  const [profile, setProfile] = useState(null)
  const [buckets, setBuckets] = useState([])
  const [activeBucket, setActiveBucket] = useState(null)
  const [prefix, setPrefix] = useState('')
  const [preview, setPreview] = useState(null)
  const [addingBucket, setAddingBucket] = useState(false)
  const [newBucketName, setNewBucketName] = useState('')
  const [theme, setTheme] = useState(
    () => localStorage.getItem('theme') || 'night-owl'
  )
  const [updateAvailable, setUpdateAvailable] = useState(null)
  const [updateInstalling, setUpdateInstalling] = useState(false)
  const [previewWidth, setPreviewWidth] = useState(380)
  // Task 8: resizable sidebar
  const [sidebarWidth, setSidebarWidth] = useState(
    () => Number(localStorage.getItem('buckethead:sidebar-width')) || 220
  )
  // Task 7: prefix navigation history
  const [prefixHistory, setPrefixHistory] = useState([''])
  const [historyIdx, setHistoryIdx] = useState(0)

  const previewDragging = useRef(false)
  const sidebarDragging = useRef(false)

  useEffect(() => {
    const t = theme === 'default' ? null : theme
    if (t) document.documentElement.dataset.theme = t
    else delete document.documentElement.dataset.theme
    localStorage.setItem('theme', theme)
  }, [theme])

  // Auto-update check — runs once on mount, production only
  useEffect(() => {
    if (!globalThis.__TAURI__) return
    const checkUpdate = async () => {
      try {
        const { check } = await import('@tauri-apps/plugin-updater')
        const update = await check()
        if (update) {
          setUpdateAvailable({ version: update.version, body: update.body })
        }
      } catch (e) {
        // Update check failure is non-fatal — silently ignore
        console.log('Update check failed:', e)
      }
    }
    checkUpdate()
  }, [])

  const handleInstallUpdate = async () => {
    if (!updateAvailable) return
    setUpdateInstalling(true)
    try {
      const { check } = await import('@tauri-apps/plugin-updater')
      const update = await check()
      if (update) {
        await update.downloadAndInstall()
        const { relaunch } = await import('@tauri-apps/plugin-process')
        await relaunch()
      }
    } catch (e) {
      console.error('Update install failed:', e)
      setUpdateInstalling(false)
    }
  }

  // Task 8: persist sidebar width
  useEffect(() => {
    localStorage.setItem('buckethead:sidebar-width', sidebarWidth)
  }, [sidebarWidth])

  // Task 7: keyboard shortcuts for back/forward — inline logic to avoid stale closure
  useEffect(() => {
    const onKeyDown = (e) => {
      if (stage !== 'browser') return
      if (e.metaKey && e.key === '[') {
        e.preventDefault()
        if (historyIdx <= 0) return
        const newIdx = historyIdx - 1
        setHistoryIdx(newIdx)
        setPrefix(prefixHistory[newIdx])
      }
      if (e.metaKey && e.key === ']') {
        e.preventDefault()
        if (historyIdx >= prefixHistory.length - 1) return
        const newIdx = historyIdx + 1
        setHistoryIdx(newIdx)
        setPrefix(prefixHistory[newIdx])
      }
    }
    globalThis.addEventListener('keydown', onKeyDown)
    return () => globalThis.removeEventListener('keydown', onKeyDown)
  }, [stage, historyIdx, prefixHistory])

  const storageKey = (p) => `buckethead:manual-buckets:${p}`

  const loadManualBuckets = (p) => {
    try { return JSON.parse(localStorage.getItem(storageKey(p)) || '[]') }
    catch { return [] }
  }

  const saveManualBuckets = (p, names) =>
    localStorage.setItem(storageKey(p), JSON.stringify(names))

  const handleConnected = ({ profile, buckets }) => {
    const autoNames = new Set(buckets.map(b => b.name))
    const manual = loadManualBuckets(profile)
      .filter(n => !autoNames.has(n))
      .map(name => ({ name, created: null }))
    setProfile(profile)
    setBuckets([...buckets, ...manual])
    setStage('browser')
  }

  // Task 7: navigating to a new prefix pushes history
  const navigateToPrefix = (newPrefix) => {
    setPrefix(newPrefix)
    setPrefixHistory(prev => {
      const sliced = prev.slice(0, historyIdx + 1)
      return [...sliced, newPrefix]
    })
    setHistoryIdx(i => i + 1)
  }

  const handleBucketSelect = (bucket) => {
    setActiveBucket(bucket)
    setPrefix('')
    setPreview(null)
    // reset history for new bucket
    setPrefixHistory([''])
    setHistoryIdx(0)
  }

  // Task 7: back/forward handlers
  const handleBack = () => {
    if (historyIdx <= 0) return
    const newIdx = historyIdx - 1
    setHistoryIdx(newIdx)
    setPrefix(prefixHistory[newIdx])
  }

  const handleForward = () => {
    if (historyIdx >= prefixHistory.length - 1) return
    const newIdx = historyIdx + 1
    setHistoryIdx(newIdx)
    setPrefix(prefixHistory[newIdx])
  }

  const canGoBack = historyIdx > 0
  const canGoForward = historyIdx < prefixHistory.length - 1

  const handleAddBucket = () => {
    const name = newBucketName.trim()
    if (!name || buckets.some(b => (b.name || b) === name)) return
    const manual = loadManualBuckets(profile)
    if (!manual.includes(name)) saveManualBuckets(profile, [...manual, name])
    setBuckets(prev => [...prev, { name, created: null }])
    setNewBucketName('')
    setAddingBucket(false)
    handleBucketSelect(name)
  }

  const handleRemoveBucket = (name) => {
    const manual = loadManualBuckets(profile).filter(n => n !== name)
    saveManualBuckets(profile, manual)
    setBuckets(prev => prev.filter(b => (b.name || b) !== name))
    if (activeBucket === name) { setActiveBucket(null); setPreview(null) }
  }

  // Preview pane resize (existing)
  const handleResizeStart = (e) => {
    e.preventDefault()
    previewDragging.current = true
    const startX = e.clientX
    const startW = previewWidth
    const onMove = (e) => {
      if (!previewDragging.current) return
      setPreviewWidth(Math.max(220, Math.min(700, startW + startX - e.clientX)))
    }
    const onUp = () => {
      previewDragging.current = false
      globalThis.removeEventListener('mousemove', onMove)
      globalThis.removeEventListener('mouseup', onUp)
    }
    globalThis.addEventListener('mousemove', onMove)
    globalThis.addEventListener('mouseup', onUp)
  }

  // Task 8: sidebar resize
  const handleSidebarResizeStart = (e) => {
    e.preventDefault()
    sidebarDragging.current = true
    const startX = e.clientX
    const startW = sidebarWidth
    const onMove = (e) => {
      if (!sidebarDragging.current) return
      setSidebarWidth(Math.max(150, Math.min(400, startW + e.clientX - startX)))
    }
    const onUp = () => {
      sidebarDragging.current = false
      globalThis.removeEventListener('mousemove', onMove)
      globalThis.removeEventListener('mouseup', onUp)
    }
    globalThis.addEventListener('mousemove', onMove)
    globalThis.addEventListener('mouseup', onUp)
  }

  const handleDisconnect = () => {
    setStage('profile')
    setProfile(null)
    setBuckets([])
    setActiveBucket(null)
    setPrefix('')
    setPreview(null)
    setPrefixHistory([''])
    setHistoryIdx(0)
  }

  return (
    <div className="app">
      {updateAvailable && (
        <div className="update-banner">
          <span>{t('app.update_available', { version: updateAvailable.version })}</span>
          <button className="btn-update" onClick={handleInstallUpdate} disabled={updateInstalling}>
            {updateInstalling ? t('app.installing') : t('app.install_restart')}
          </button>
          <button className="btn-ghost btn-update-dismiss" onClick={() => setUpdateAvailable(null)}>✕</button>
        </div>
      )}
      <main className="app-body">
        {stage === 'profile' && (
          <ProfileSelector onConnected={handleConnected} />
        )}

        {stage === 'browser' && (
          <div className="browser-layout">
            {/* Task 8: sidebar with dynamic width + resize handle */}
            <aside className="sidebar" style={{ width: sidebarWidth }}>
              <div className="sidebar-label-row">
                <span className="sidebar-label">{t('app.buckets')}</span>
                <button
                  className="sidebar-add-btn"
                  title={t('app.add_bucket_title')}
                  onClick={() => setAddingBucket(v => !v)}
                >+</button>
              </div>

              {addingBucket && (
                <div className="sidebar-add-form">
                  <input
                    className="sidebar-add-input"
                    autoFocus
                    placeholder={t('app.bucket_name_placeholder')}
                    value={newBucketName}
                    onChange={e => setNewBucketName(e.target.value)}
                    onKeyDown={e => {
                      if (e.key === 'Enter') handleAddBucket()
                      if (e.key === 'Escape') { setAddingBucket(false); setNewBucketName('') }
                    }}
                  />
                  <button className="btn-ghost" onClick={handleAddBucket}>{t('app.add')}</button>
                </div>
              )}

              <BucketList
                buckets={buckets}
                active={activeBucket}
                onSelect={handleBucketSelect}
                manualNames={new Set(loadManualBuckets(profile))}
                onRemove={handleRemoveBucket}
              />

              {/* Task 8: sidebar resize handle on right edge */}
              <button
                type="button"
                className="sidebar-resize-handle"
                aria-label={t('app.resize_sidebar')}
                onMouseDown={handleSidebarResizeStart}
                onKeyDown={(e) => {
                  if (e.key === 'ArrowLeft') setSidebarWidth(w => Math.max(150, w - 10))
                  if (e.key === 'ArrowRight') setSidebarWidth(w => Math.min(400, w + 10))
                }}
              />
            </aside>

            <section className="content">
              {/* Task 9: differentiated empty states */}
              {activeBucket ? (
                <ObjectBrowser
                  bucket={activeBucket}
                  prefix={prefix}
                  onPrefixChange={navigateToPrefix}
                  onPreview={setPreview}
                  onBack={handleBack}
                  onForward={handleForward}
                  canGoBack={canGoBack}
                  canGoForward={canGoForward}
                />
              ) : (
                <div className="empty-state">
                  <span className="empty-icon">◈</span>
                  <p>{t('app.select_bucket')}</p>
                </div>
              )}
            </section>

            {preview && (
              <>
                <button
                  type="button"
                  className="preview-resize-handle"
                  aria-label={t('app.resize_preview')}
                  onMouseDown={handleResizeStart}
                  onKeyDown={(e) => {
                    if (e.key === 'ArrowLeft') setPreviewWidth(w => Math.min(700, w + 10))
                    if (e.key === 'ArrowRight') setPreviewWidth(w => Math.max(220, w - 10))
                  }}
                />
                <FilePreview
                  preview={preview}
                  onClose={() => setPreview(null)}
                  width={previewWidth}
                />
              </>
            )}
          </div>
        )}
      </main>

      <footer className="app-header">
        <div className="app-logo">
          <span className="logo-mark">◈</span>
          <span className="logo-text">BUCKETHEAD</span>
        </div>
        <div className="lang-switcher">
          {LANGS.map(lng => (
            <button
              key={lng.code}
              type="button"
              className={`lang-btn${i18n.language === lng.code ? ' active' : ''}`}
              onClick={() => handleLangChange(lng.code)}
            >{lng.label}</button>
          ))}
        </div>
        <div className="theme-swatches">
          {THEMES.map(thm => (
            <button
              key={thm.id}
              type="button"
              className={`theme-swatch${theme === thm.id ? ' active' : ''}`}
              style={{ background: thm.color }}
              title={thm.label}
              aria-label={t('app.theme_label', { label: thm.label })}
              aria-pressed={theme === thm.id}
              onClick={() => setTheme(thm.id)}
            />
          ))}
        </div>
        {profile && (
          <div className="header-right">
            <span className="profile-badge">
              <span className="profile-dot" />
              {profile}
            </span>
            <button className="btn-ghost" onClick={handleDisconnect}>
              {t('app.disconnect')}
            </button>
          </div>
        )}
      </footer>
    </div>
  )
}
