import { useState, useEffect } from 'react'
import { aws } from '../bridge'

function ConnectError({ error, profile }) {
  const [copied, setCopied] = useState(false)

  if (!error) return null

  const rawMsg = typeof error === 'string' ? error : (error.message || 'Connection failed')

  if (rawMsg.startsWith('SSO_EXPIRED::')) {
    const profileName = rawMsg.slice('SSO_EXPIRED::'.length) || profile
    const cmd = `aws sso login --profile ${profileName}`

    const handleCopy = () => {
      navigator.clipboard.writeText(cmd).then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
    }

    return (
      <div className="connect-error-box">
        <div className="connect-error-title">SSO token expired. Re-authenticate:</div>
        <div className="connect-error-cmd-row">
          <code className="connect-error-cmd">{cmd}</code>
          <button className="btn-copy" onClick={handleCopy}>{copied ? 'Copied!' : 'Copy'}</button>
        </div>
      </div>
    )
  }

  const displayMsg = rawMsg.startsWith('CREDENTIALS_ERROR::')
    ? rawMsg.slice('CREDENTIALS_ERROR::'.length)
    : rawMsg

  return (
    <div className="connect-error-box">
      <div className="connect-error-title">Connection failed:</div>
      <div className="connect-error-msg">{displayMsg}</div>
    </div>
  )
}

export default function ProfileSelector({ onConnected }) {
  const [profiles, setProfiles] = useState([])
  const [selected, setSelected] = useState('')
  const [loading, setLoading] = useState(false)
  const [connectError, setConnectError] = useState(null)
  const [loadingProfiles, setLoadingProfiles] = useState(true)

  useEffect(() => {
    aws.listProfiles()
      .then(p => {
        setProfiles(p)
        if (p.length > 0) {
          const last = localStorage.getItem('thathoo:last-profile')
          setSelected(last && p.includes(last) ? last : p[0])
        }
      })
      .catch(() => setConnectError('Could not read ~/.aws/credentials'))
      .finally(() => setLoadingProfiles(false))
  }, [])

  const handleConnect = async () => {
    if (!selected) return
    setLoading(true)
    setConnectError(null)
    try {
      const buckets = await aws.setProfile(selected)
      localStorage.setItem('thathoo:last-profile', selected)
      onConnected({ profile: selected, buckets })
    } catch (e) {
      setConnectError(typeof e === 'string' ? e : e.message || 'Connection failed')
    } finally {
      setLoading(false)
    }
  }

  const handleProfileChange = (e) => {
    setSelected(e.target.value)
    setConnectError(null)
  }

  return (
    <div className="profile-screen">
      <div className="profile-card">
        <div className="profile-card-header">
          <div className="profile-card-title">Connect to AWS</div>
          <div className="profile-card-sub">Select a local profile from ~/.aws/credentials</div>
        </div>

        <div className="profile-card-body">
          <div className="form-group">
            <label className="form-label">AWS Profile</label>
            {loadingProfiles ? (
              <div style={{ color: 'var(--text-2)', fontSize: 11, padding: '8px 0' }}>
                <span className="spinner" />Reading profiles…
              </div>
            ) : (
              <select
                className="form-select"
                value={selected}
                onChange={handleProfileChange}
              >
                {profiles.length === 0 && (
                  <option value="">No profiles found</option>
                )}
                {profiles.map(p => (
                  <option key={p} value={p}>{p}</option>
                ))}
              </select>
            )}
          </div>

          {connectError && (
            <ConnectError error={connectError} profile={selected} />
          )}
        </div>

        <div className="profile-card-footer">
          <button
            className="btn-primary"
            onClick={handleConnect}
            disabled={loading || !selected || loadingProfiles}
          >
            {loading ? 'Connecting…' : 'Connect →'}
          </button>
        </div>
      </div>
    </div>
  )
}
