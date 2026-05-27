export default function BucketList({ buckets, active, onSelect, manualNames = new Set(), onRemove }) {
  return (
    <div className="bucket-list">
      {buckets.length === 0 && (
        <div style={{ padding: '12px 14px', color: 'var(--text-2)', fontSize: 11 }}>
          No buckets found
        </div>
      )}
      {buckets.map(bucket => {
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
