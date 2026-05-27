import { useState } from 'react'
import { useTranslation } from 'react-i18next'

export default function BucketList({ buckets, active, onSelect, manualNames = new Set(), onRemove }) {
  const { t } = useTranslation()
  // Task 3: bucket filter — only shown when count > 5
  const [filter, setFilter] = useState('')

  const filteredBuckets = filter
    ? buckets.filter(b => (b.name || b.Name || '').toLowerCase().includes(filter.toLowerCase()))
    : buckets

  return (
    <div className="bucket-list">
      {buckets.length > 0 && (
        <div className="bucket-filter-wrap">
          <input
            className="bucket-filter-input"
            placeholder={t('bucket.filter_placeholder')}
            value={filter}
            onChange={e => setFilter(e.target.value)}
          />
        </div>
      )}

      {buckets.length === 0 && (
        <div style={{ padding: '12px 14px', color: 'var(--text-2)', fontSize: 11 }}>
          {t('bucket.no_buckets')}
        </div>
      )}

      {filteredBuckets.map(bucket => {
        const name = bucket.name || bucket.Name
        const isManual = manualNames.has(name)
        return (
          <div
            key={name}
            className={`bucket-item ${active === name ? 'active' : ''}`}
          >
            <button
              type="button"
              className="bucket-item-select"
              onClick={() => onSelect(name)}
              title={name}
            >
              <span className="bucket-icon">▣</span>
              <span className="bucket-item-name">{name}</span>
            </button>
            {isManual && (
              <button
                type="button"
                className="bucket-remove-btn"
                title={t('bucket.remove')}
                onClick={() => onRemove(name)}
              >×</button>
            )}
          </div>
        )
      })}
    </div>
  )
}
