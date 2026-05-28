import { useState, useEffect, useRef, useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import PropTypes from 'prop-types'
import { aws } from '../bridge'
import mascotKeypair from '../assets/mascot_keypair.png'
import mascotConsole from '../assets/mascot_console.png'
import mascotSso from '../assets/mascot_sso.png'
import mascotLogin from '../assets/mascot_login.png'

function CredentialsHelper({ onClose }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(null)
  const profileName = '<profile-name>'

  const methods = [
    {
      key: 'keypair',
      mascot: mascotKeypair,
      title: t('profile.creds_keypair_title'),
      desc: t('profile.creds_keypair_desc'),
      cmd: `aws configure --profile ${profileName}`,
    },
    {
      key: 'console_login',
      mascot: mascotConsole,
      title: t('profile.creds_console_title'),
      desc: t('profile.creds_console_desc'),
      cmd: `aws login --profile ${profileName}`,
    },
    {
      key: 'sso_config',
      mascot: mascotSso,
      title: t('profile.creds_sso_config_title'),
      desc: t('profile.creds_sso_config_desc'),
      cmd: `aws configure sso --profile ${profileName}`,
    },
  ]

  const copyTimerRef = useRef(null)

  const handleCopy = (key, cmd) => {
    navigator.clipboard.writeText(cmd).then(() => {
      setCopied(key)
      clearTimeout(copyTimerRef.current)
      copyTimerRef.current = setTimeout(() => setCopied(null), 2000)
    })
  }

  useEffect(() => () => clearTimeout(copyTimerRef.current), [])

  useEffect(() => {
    const handleKey = (e) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [onClose])

  return (
    <div className="creds-helper-overlay">
      <dialog
        className="creds-helper-modal"
        open
        aria-label={t('profile.creds_modal_title')}
      >
        <div className="creds-helper-header">
          <span className="creds-helper-title">{t('profile.creds_modal_title')}</span>
          <button className="preview-close" onClick={onClose}>✕</button>
        </div>
        <div className="creds-helper-body">
          {methods.map(m => (
            <div key={m.key} className="creds-method-card">
              <img className="creds-method-mascot" src={m.mascot} alt="" />
              <div className="creds-method-content">
                <div className="creds-method-title">{m.title}</div>
                <div className="creds-method-desc">{m.desc}</div>
                <div className="connect-error-cmd-row">
                  <code className="connect-error-cmd">{m.cmd}</code>
                  <button className="btn-copy" onClick={() => handleCopy(m.key, m.cmd)}>
                    {copied === m.key ? t('profile.copied') : t('profile.copy')}
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      </dialog>
    </div>
  )
}

CredentialsHelper.propTypes = {
  onClose: PropTypes.func.isRequired,
}

function ConnectError({ error, profile }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState(false)

  if (!error) return null

  const rawMsg = typeof error === 'string' ? error : (error.message || 'Connection failed')

  // Strip known prefixes to get the human-readable message
  let msg = rawMsg
  if (rawMsg.startsWith('SSO_EXPIRED::')) msg = rawMsg.slice('SSO_EXPIRED::'.length)
  else if (rawMsg.startsWith('CREDENTIALS_ERROR::')) msg = rawMsg.slice('CREDENTIALS_ERROR::'.length)
  else if (rawMsg.startsWith('MFA_REQUIRED::')) msg = rawMsg.slice('MFA_REQUIRED::'.length)

  const isSso = rawMsg.startsWith('SSO_EXPIRED::')
  const ssoProfile = isSso ? (msg || profile) : null
  const cmd = isSso ? `aws sso login --profile ${ssoProfile}` : null

  const handleCopy = () => {
    navigator.clipboard.writeText(cmd).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }

  return (
    <div className="connect-error-box">
      <div className="connect-error-msg">{msg}</div>
      {isSso && (
        <div className="connect-error-cmd-row">
          <code className="connect-error-cmd">{cmd}</code>
          <button className="btn-copy" onClick={handleCopy}>{copied ? t('profile.copied') : t('profile.copy')}</button>
        </div>
      )}
    </div>
  )
}

ConnectError.propTypes = {
  error: PropTypes.oneOfType([PropTypes.string, PropTypes.shape({ message: PropTypes.string })]),
  profile: PropTypes.string,
}

export default function ProfileSelector({ onConnected }) {
  const { t } = useTranslation()
  const [profiles, setProfiles] = useState([])
  const [selected, setSelected] = useState('')
  const [loading, setLoading] = useState(false)
  const [connectError, setConnectError] = useState(null)
  const [loadingProfiles, setLoadingProfiles] = useState(true)
  const [mfaRequired, setMfaRequired] = useState(false)
  const [mfaToken, setMfaToken] = useState('')
  const [mfaLoading, setMfaLoading] = useState(false)
  const [showCredsHelper, setShowCredsHelper] = useState(false)
  const handleCloseHelper = useCallback(() => setShowCredsHelper(false), [])

  useEffect(() => {
    aws.listProfiles()
      .then(p => {
        setProfiles(p)
        if (p.length > 0) {
          const last = localStorage.getItem('buckethead:last-profile')
          const found = last && p.find(x => x.name === last)
          setSelected(found ? found.name : p[0].name)
        }
      })
      .catch(() => setConnectError(t('profile.read_credentials_error')))
      .finally(() => setLoadingProfiles(false))
  }, [])

  const handleConnect = async () => {
    if (!selected) return
    setLoading(true)
    setConnectError(null)
    setMfaRequired(false)
    setMfaToken('')
    try {
      const buckets = await aws.setProfile(selected)
      localStorage.setItem('buckethead:last-profile', selected)
      onConnected({ profile: selected, buckets })
    } catch (e) {
      const msg = typeof e === 'string' ? e : (e.message || t('profile.connection_failed'))
      if (msg.startsWith('MFA_REQUIRED::')) {
        setMfaRequired(true)
      } else {
        setConnectError(msg)
      }
    } finally {
      setLoading(false)
    }
  }

  const handleProfileChange = (e) => {
    setSelected(e.target.value)
    setConnectError(null)
    setMfaRequired(false)
    setMfaToken('')
  }


  return (
    <div className="profile-screen">
      <div className="login-mascot-wrap">
        <span className="login-mascot-bubble">{t('profile.welcome_hint')}</span>
        <img className="login-mascot" src={mascotLogin} alt="" />
      </div>
      <div className="profile-card">
        <div className="profile-card-header">
          <div className="profile-card-title">{t('profile.connect_to_aws')}</div>
          <div className="profile-card-sub">{t('profile.select_profile_hint')}</div>
          <button className="creds-help-link" onClick={() => setShowCredsHelper(true)}>
            {t('profile.creds_help_link')}
          </button>
        </div>

        <div className="profile-card-body">
          <div className="form-group">
            {loadingProfiles ? (
              <div style={{ color: 'var(--text-2)', fontSize: 11, padding: '8px 0' }}>
                <span className="spinner" />{t('profile.reading_profiles')}
              </div>
            ) : (
              <>
                <label className="form-label" htmlFor="profile-select">{t('profile.aws_profile')}</label>
                <select
                  id="profile-select"
                  className="form-select"
                  value={selected}
                  onChange={handleProfileChange}
                >
                  {profiles.length === 0 && (
                    <option value="">{t('profile.no_profiles')}</option>
                  )}
                  {profiles.map(p => (
                    <option key={p.name} value={p.name}>
                      {p.name}{p.role ? ` (${p.role})` : ''}{p.mfa ? ' 🔐' : ''}{p.sso ? ' [SSO]' : ''}
                    </option>
                  ))}
                </select>
              </>
            )}
          </div>

          {connectError && !mfaRequired && (
            <ConnectError error={connectError} profile={selected} />
          )}

          {mfaRequired && (
            <div className="mfa-prompt">
              <div className="connect-error-title">{t('profile.mfa_required')}</div>
              <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                <input
                  className="form-input mfa-input"
                  type="text"
                  inputMode="numeric"
                  pattern="[0-9]{6}"
                  maxLength={6}
                  placeholder="000000"
                  value={mfaToken}
                  onChange={e => setMfaToken(e.target.value.replaceAll(/\D/g, ''))}
                  autoFocus
                />
                <button
                  className="btn-primary"
                  disabled={mfaToken.length !== 6 || mfaLoading}
                  onClick={async () => {
                    setMfaLoading(true)
                    try {
                      const buckets = await aws.setProfileMfa(selected, mfaToken)
                      localStorage.setItem('buckethead:last-profile', selected)
                      onConnected({ profile: selected, buckets })
                    } catch (e) {
                      setConnectError(typeof e === 'string' ? e : (e.message || t('profile.mfa_failed')))
                      setMfaRequired(false)
                    } finally {
                      setMfaLoading(false)
                    }
                  }}
                >{mfaLoading ? t('profile.verifying') : t('profile.submit')}</button>
              </div>
            </div>
          )}
        </div>

        <div className="profile-card-footer">
          <button
            className="btn-primary"
            onClick={handleConnect}
            disabled={loading || !selected || loadingProfiles || mfaRequired}
          >
            {loading ? t('profile.connecting') : t('profile.connect')}
          </button>
        </div>
      </div>

      {showCredsHelper && (
        <CredentialsHelper onClose={handleCloseHelper} />
      )}
    </div>
  )
}

ProfileSelector.propTypes = {
  onConnected: PropTypes.func.isRequired,
}
