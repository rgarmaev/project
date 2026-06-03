import { useEffect, useState } from 'react'

interface ExchangeQuote {
  bid: number
  ask: number
}

interface MultiSymbolRow {
  symbol: string
  binance_spot: ExchangeQuote | null
  bybit_spot:   ExchangeQuote | null
  okx_spot:     ExchangeQuote | null
  bingx_spot:   ExchangeQuote | null
  bitget_spot:  ExchangeQuote | null
  kucoin_spot:  ExchangeQuote | null
  gate_spot:    ExchangeQuote | null
  best_spread_pct: number
  best_buy:  string
  best_sell: string
}

const EXCHANGES: { key: keyof MultiSymbolRow; label: string; color: string }[] = [
  { key: 'binance_spot', label: 'Binance', color: '#f0b90b' },
  { key: 'bybit_spot',   label: 'Bybit',   color: '#f7a600' },
  { key: 'okx_spot',     label: 'OKX',     color: '#00d4ff' },
  { key: 'bingx_spot',   label: 'BingX',   color: '#00d4aa' },
  { key: 'bitget_spot',  label: 'Bitget',  color: '#00aaff' },
  { key: 'kucoin_spot',  label: 'KuCoin',  color: '#22bb66' },
  { key: 'gate_spot',    label: 'Gate',    color: '#e6b800' },
]

function fmt(v: number): string {
  if (v <= 0) return '—'
  if (v >= 1000) return v.toLocaleString('en', { maximumSignificantDigits: 6 })
  if (v >= 1)    return v.toPrecision(5)
  return v.toPrecision(4)
}

function fmtSpread(v: number): string {
  return `${v >= 0 ? '+' : ''}${v.toFixed(3)}%`
}

const TH: React.CSSProperties = {
  padding: '6px 8px',
  color: '#444',
  fontSize: 10,
  textAlign: 'right' as const,
  textTransform: 'uppercase' as const,
  whiteSpace: 'nowrap' as const,
  cursor: 'pointer',
  userSelect: 'none' as const,
}

export function MarketScanner() {
  const [rows, setRows]       = useState<MultiSymbolRow[]>([])
  const [search, setSearch]   = useState('')
  const [sortKey, setSortKey] = useState<'symbol' | 'spread'>('spread')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    function load() {
      fetch('/api/multi-snapshot')
        .then(r => r.json())
        .then((d: MultiSymbolRow[]) => { setRows(d); setLoading(false) })
        .catch(() => {})
    }
    load()
    const id = setInterval(load, 3000)
    return () => clearInterval(id)
  }, [])

  function toggleSort(key: 'symbol' | 'spread') {
    if (key === sortKey) setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    else { setSortKey(key); setSortDir('desc') }
  }

  const filtered = rows
    .filter(r => !search.trim() || r.symbol.toLowerCase().includes(search.trim().toLowerCase()))

  const sorted = [...filtered].sort((a, b) => {
    const cmp = sortKey === 'symbol'
      ? a.symbol.localeCompare(b.symbol)
      : a.best_spread_pct - b.best_spread_pct
    return sortDir === 'asc' ? cmp : -cmp
  })

  function arrow(key: 'symbol' | 'spread') {
    if (key !== sortKey) return ''
    return sortDir === 'asc' ? ' ▲' : ' ▼'
  }

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
        <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          Спред по рынку · Все биржи · Spot ({filtered.length} пар)
        </div>
        <input
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Поиск..."
          style={{
            background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
            color: '#e0e0e0', padding: '4px 8px', fontSize: 11,
            fontFamily: 'inherit', outline: 'none', width: 120,
          }}
        />
      </div>

      {loading ? (
        <div style={{ color: '#333', textAlign: 'center', padding: '20px 0', fontSize: 13 }}>
          Загрузка...
        </div>
      ) : (
        <div style={{ overflowX: 'auto', overflowY: 'auto', maxHeight: 440 }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
            <thead style={{ position: 'sticky', top: 0, background: '#111', zIndex: 1 }}>
              <tr>
                <th style={{ ...TH, textAlign: 'left', cursor: 'pointer' }} onClick={() => toggleSort('symbol')}>
                  Пара{arrow('symbol')}
                </th>
                {EXCHANGES.map(ex => (
                  <th key={ex.key} style={{ ...TH, color: ex.color }}>
                    {ex.label}
                  </th>
                ))}
                <th style={{ ...TH, cursor: 'pointer', color: '#e0e0e0' }} onClick={() => toggleSort('spread')}>
                  Лучший спред{arrow('spread')}
                </th>
                <th style={{ ...TH }}>Купить</th>
                <th style={{ ...TH }}>Продать</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map(row => {
                const spreadColor = row.best_spread_pct > 0.1 ? '#00ff87'
                  : row.best_spread_pct > 0 ? '#888' : '#444'
                return (
                  <tr key={row.symbol} style={{ borderBottom: '1px solid #1a1a1a' }}>
                    <td style={{ padding: '4px 8px', color: '#888', whiteSpace: 'nowrap' }}>
                      <span style={{ color: '#e0e0e0', fontWeight: 600 }}>
                        {row.symbol.replace('USDT', '')}
                      </span>
                      <span style={{ color: '#444' }}>/USDT</span>
                    </td>
                    {EXCHANGES.map(ex => {
                      const q = row[ex.key] as ExchangeQuote | null
                      return (
                        <td key={ex.key} style={{ padding: '4px 8px', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>
                          {q ? (
                            <span style={{ color: '#555' }}>
                              <span style={{ color: '#666' }}>{fmt(q.ask)}</span>
                              <span style={{ color: '#2a2a2a' }}> / </span>
                              <span style={{ color: '#444' }}>{fmt(q.bid)}</span>
                            </span>
                          ) : (
                            <span style={{ color: '#222' }}>—</span>
                          )}
                        </td>
                      )
                    })}
                    <td style={{ padding: '4px 8px', textAlign: 'right', fontWeight: 600, color: spreadColor, fontVariantNumeric: 'tabular-nums' }}>
                      {fmtSpread(row.best_spread_pct)}
                    </td>
                    <td style={{ padding: '4px 8px', textAlign: 'right', color: '#555', whiteSpace: 'nowrap' }}>
                      {row.best_buy || '—'}
                    </td>
                    <td style={{ padding: '4px 8px', textAlign: 'right', color: '#555', whiteSpace: 'nowrap' }}>
                      {row.best_sell || '—'}
                    </td>
                  </tr>
                )
              })}
              {sorted.length === 0 && !loading && (
                <tr>
                  <td colSpan={11} style={{ padding: 20, color: '#333', textAlign: 'center' }}>
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
