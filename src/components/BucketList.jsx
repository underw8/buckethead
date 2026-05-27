import { useState } from 'react'

export default function BucketList({ buckets, active, onSelect, manualNames = new Set(), onRemove }) {
  // Task 3: bucket filter — only shown when count > 5
  const [filter, setFilter] = useState('')

  const filteredBuckets = filter
    ? buckets.filter(b => (b.name || b.Name || '').toLowerCase().includes(filter.toLowerCase()))
    : buckets

  return (
    <div className="bucket-list">
      {/* Task 3: filter input above list, only when > 5 buckets */}
      {buckets.length > 5 && (
        <div className="bucket-filter-wrap">
          <input
            className="bucket-filter-input"
            placeholder="Filter buckets…"
            value={filter}
            onChange={e => setFilter(e.target.value)}
          />
        </div>
      )}

      {buckets.length === 0 && (
        <div style={{ padding: '12px 14px', color: 'var(--text-2)', fontSize: 11 }}>
          No buckets found
        </div>
      )}

      {filteredBuckets.map(bucket => {
        const name = bucket.name || bucket.Name
        const isManual = manualNames.has(name)
        return (
          <div
            key={name}
            className={`bucket-item ${active === name ? 'active' : ''}`}
            onClick={() => onSelect(name)}
            title={name}
          >
            <span className="bucket-icon">▣</span>
            <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{name}</span>
            {isManual && (
              <button
                className="bucket-remove-btn"
                title="Remove"
                onClick={e => { e.stopPropagation(); onRemove(name) }}
              >×</button>
            )}
          </div>
        )
      })}
    </div>
  )
}
