import { useEffect, useState } from 'react'
import { MarketRow, MarketSnapshot } from '../types'

type SortKey = keyof Pick<MarketRow,
  'symbol' | 'binance_ask' | 'bybit_bid' | 'spread_ab' | 'bybit_ask' | 'binance_bid' | 'spread_ba'
>

const EMPTY_SNAPSHOT: MarketSnapshot = { spot: [], perp: [] }

function fmt(v: number): string {
  if (v === 0) return '—'
  if (v >= 1000) return v.toLocaleString('en', { maximumSignificantDigits: 6 })
  return v.toPrecision(6)
}

function fmtSpread(v: number): string {
  return `${v >= 0 ? '+' : ''}${v.toFixed(4)}%`
}

const COL_HEADER: React.CSSProperties = {
  padding: '6px 10px',
  color: '#444',
  fontSize: 10,
  textAlign: 'left' as const,
  textTransform: 'uppercase' as const,
  cursor: 'pointer',
  userSelect: 'none' as const,
  whiteSpace: 'nowrap' as const,
}

export function MarketScanner() {
  const [data, setData] = useState<MarketSnapshot>(EMPTY_SNAPSHOT)
  const [activeTab, setActiveTab] = useState<'spot' | 'perp'>('spot')
  const [sortKey, setSortKey] = useState<SortKey>('spread_ab')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    function load() {
      fetch('/api/market')
        .then(r => r.json())
        .then((d: MarketSnapshot) => { setData(d); setLoading(false) })
        .catch(() => {})
    }
    load()
    const id = setInterval(load, 3000)
    return () => clearInterval(id)
  }, [])

  function toggleSort(key: SortKey) {
    if (key === sortKey) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  const rows = data[activeTab]
  const sorted = [...rows].sort((a, b) => {
    const av = a[sortKey]
    const bv = b[sortKey]
    const cmp = typeof av === 'string'
      ? av.localeCompare(bv as string)
      : (av as number) - (bv as number)
    return sortDir === 'asc' ? cmp : -cmp
  })

  function arrow(key: SortKey) {
    if (key !== sortKey) return ' '
    return sortDir === 'asc' ? ' ▲' : ' ▼'
  }

  const columns: { label: string; key: SortKey; align?: 'right' }[] = [
    { label: 'Пара',       key: 'symbol' },
    { label: 'Bin Ask',    key: 'binance_ask',  align: 'right' },
    { label: 'Byb Bid',    key: 'bybit_bid',    align: 'right' },
    { label: 'B→Y Спред',  key: 'spread_ab',    align: 'right' },
    { label: 'Byb Ask',    key: 'bybit_ask',    align: 'right' },
    { label: 'Bin Bid',    key: 'binance_bid',  align: 'right' },
    { label: 'Y→B Спред',  key: 'spread_ba',    align: 'right' },
  ]

  function tabBtn(label: string, key: 'spot' | 'perp') {
    const active = activeTab === key
    return (
      <button
        key={key}
        onClick={() => setActiveTab(key)}
        style={{
          background: 'none',
          border: `1px solid ${active ? '#00ff87' : '#2a2a2a'}`,
          borderRadius: 4,
          color: active ? '#00ff87' : '#444',
          cursor: 'pointer',
          padding: '3px 12px',
          fontSize: 11,
          fontFamily: 'inherit',
          fontWeight: active ? 600 : 400,
        }}
      >
        {label}
      </button>
    )
  }

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
        <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          Спред по рынку · Binance vs Bybit
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          {tabBtn('Spot', 'spot')}
          {tabBtn('Perp', 'perp')}
        </div>
      </div>

      {loading ? (
        <div style={{ color: '#333', textAlign: 'center', padding: '20px 0', fontSize: 13 }}>
          Загрузка...
        </div>
      ) : (
        <div style={{ overflowY: 'auto', maxHeight: 400 }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead style={{ position: 'sticky', top: 0, background: '#111' }}>
              <tr>
                {columns.map(col => (
                  <th
                    key={col.key}
                    style={{ ...COL_HEADER, textAlign: col.align ?? 'left' }}
                    onClick={() => toggleSort(col.key)}
                  >
                    {col.label}{arrow(col.key)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {sorted.map(row => (
                <tr key={row.symbol} style={{ borderBottom: '1px solid #1a1a1a' }}>
                  <td style={{ padding: '5px 10px', color: '#888', fontSize: 12 }}>
                    {row.symbol.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.binance_ask)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.bybit_bid)}
                  </td>
                  <td style={{ padding: '5px 10px', fontSize: 11, textAlign: 'right', fontWeight: 600,
                    color: row.spread_ab > 0 ? '#00ff87' : '#555' }}>
                    {fmtSpread(row.spread_ab)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.bybit_ask)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.binance_bid)}
                  </td>
                  <td style={{ padding: '5px 10px', fontSize: 11, textAlign: 'right', fontWeight: 600,
                    color: row.spread_ba > 0 ? '#00ff87' : '#555' }}>
                    {fmtSpread(row.spread_ba)}
                  </td>
                </tr>
              ))}
              {sorted.length === 0 && (
                <tr>
                  <td colSpan={7} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>
                    Нет данных
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
