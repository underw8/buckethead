import { useState, useEffect } from 'react'
import { aws } from '../bridge'

export default function ProfileSelector({ onConnected }) {
  const [profiles, setProfiles] = useState([])
  const [selected, setSelected] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(null)
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
      .catch(() => setError('Could not read ~/.aws/credentials'))
      .finally(() => setLoadingProfiles(false))
  }, [])

  const handleConnect = async () => {
    if (!selected) return
    setLoading(true)
    setError(null)
    try {
      const buckets = await aws.setProfile(selected)
      localStorage.setItem('thathoo:last-profile', selected)
      onConnected({ profile: selected, buckets })
    } catch (e) {
      setError(typeof e === 'string' ? e : e.message || 'Connection failed')
    } finally {
      setLoading(false)
    }
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
                onChange={e => setSelected(e.target.value)}
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

        </div>

        <div className="profile-card-footer">
          <span className="error-msg">{error || ''}</span>
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
