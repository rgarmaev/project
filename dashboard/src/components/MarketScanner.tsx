import { useEffect, useState } from 'react'

interface ExchangeQuote { bid: number; ask: number }

interface MultiSymbolRow {
  symbol:       string
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

// Format price with consistent significant digits
function fmtPrice(v: number): string {
  if (v <= 0) return '—'
  if (v >= 10000) return v.toFixed(0)
  if (v >= 100)   return v.toFixed(2)
  if (v >= 1)     return v.toFixed(4)
  if (v >= 0.01)  return v.toFixed(5)
  return v.toFixed(6)
}

function fmtSpread(v: number): string {
  return `${v >= 0 ? '+' : ''}${v.toFixed(3)}%`
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

  const filtered = rows.filter(r =>
    !search.trim() || r.symbol.toLowerCase().includes(search.trim().toLowerCase())
  )
  const sorted = [...filtered].sort((a, b) => {
    const cmp = sortKey === 'symbol'
      ? a.symbol.localeCompare(b.symbol)
      : a.best_spread_pct - b.best_spread_pct
    return sortDir === 'asc' ? cmp : -cmp
  })

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
        <span style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          Спред по рынку · Все биржи ({filtered.length} пар)
        </span>
        <input
          value={search} onChange={e => setSearch(e.target.value)}
          placeholder="Поиск..."
          style={{
            background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
            color: '#e0e0e0', padding: '4px 8px', fontSize: 11,
            fontFamily: 'inherit', outline: 'none', width: 110,
          }}
        />
      </div>

      {loading ? (
        <div style={{ color: '#333', textAlign: 'center', padding: '20px 0', fontSize: 12 }}>Загрузка...</div>
      ) : (
        <div style={{ overflowX: 'auto', overflowY: 'auto', maxHeight: 440 }}>
          <table style={{ borderCollapse: 'collapse', fontSize: 11, whiteSpace: 'nowrap', width: '100%' }}>
            <colgroup>
              <col style={{ width: 70 }} />
              {EXCHANGES.map(ex => <col key={ex.key} style={{ width: 88 }} />)}
              <col style={{ width: 76 }} />
            </colgroup>
            <thead style={{ position: 'sticky', top: 0, background: '#111', zIndex: 1 }}>
              <tr style={{ borderBottom: '1px solid #222' }}>
                <th
                  onClick={() => toggleSort('symbol')}
                  style={{ padding: '5px 8px', textAlign: 'left', color: '#555', fontSize: 10,
                    textTransform: 'uppercase', cursor: 'pointer', userSelect: 'none' }}>
                  Пара{sortKey === 'symbol' ? (sortDir === 'asc' ? ' ▲' : ' ▼') : ''}
                </th>
                {EXCHANGES.map(ex => (
                  <th key={ex.key}
                    style={{ padding: '5px 8px', textAlign: 'right', fontSize: 10,
                      textTransform: 'uppercase', color: ex.color, fontWeight: 400 }}>
                    {ex.label}
                  </th>
                ))}
                <th
                  onClick={() => toggleSort('spread')}
                  style={{ padding: '5px 8px', textAlign: 'right', color: '#555', fontSize: 10,
                    textTransform: 'uppercase', cursor: 'pointer', userSelect: 'none' }}>
                  Спред{sortKey === 'spread' ? (sortDir === 'asc' ? ' ▲' : ' ▼') : ''}
                </th>
              </tr>
            </thead>
            <tbody>
              {sorted.map(row => {
                // Find min ask (best to buy) and max bid (best to sell) across all exchanges
                const asks = EXCHANGES.map(ex => {
                  const q = row[ex.key] as ExchangeQuote | null
                  return q && q.ask > 0 ? q.ask : Infinity
                })
                const bids = EXCHANGES.map(ex => {
                  const q = row[ex.key] as ExchangeQuote | null
                  return q && q.bid > 0 ? q.bid : 0
                })
                const minAsk = Math.min(...asks.filter(v => v < Infinity))
                const maxBid = Math.max(...bids)

                const spreadColor = row.best_spread_pct > 0.2 ? '#00ff87'
                  : row.best_spread_pct > 0.05 ? '#aaaa00'
                  : row.best_spread_pct > 0    ? '#555' : '#333'

                return (
                  <tr key={row.symbol} style={{ borderBottom: '1px solid #1a1a1a' }}>
                    {/* Symbol */}
                    <td style={{ padding: '4px 8px' }}>
                      <span style={{ color: '#ccc', fontWeight: 600 }}>
                        {row.symbol.replace('USDT', '')}
                      </span>
                      <span style={{ color: '#333' }}>/U</span>
                    </td>

                    {/* Exchange prices — show ask price, highlight min/max */}
                    {EXCHANGES.map((ex, idx) => {
                      const q = row[ex.key] as ExchangeQuote | null
                      if (!q) return (
                        <td key={ex.key} style={{ padding: '4px 8px', textAlign: 'right', color: '#252525' }}>—</td>
                      )
                      const isBestBuy  = q.ask === minAsk
                      const isBestSell = q.bid === maxBid
                      const color = isBestBuy ? '#00ff87' : isBestSell ? '#4488ff' : '#555'
                      const bg    = isBestBuy ? 'rgba(0,255,135,0.05)' : isBestSell ? 'rgba(68,136,255,0.05)' : 'transparent'
                      // Show ask (what you pay to buy); tooltip shows bid
                      return (
                        <td key={ex.key}
                          title={`ask: ${q.ask}  bid: ${q.bid}`}
                          style={{
                            padding: '4px 8px', textAlign: 'right',
                            fontVariantNumeric: 'tabular-nums', fontFamily: 'monospace',
                            color, background: bg,
                          }}>
                          {fmtPrice(q.ask)}
                        </td>
                      )
                    })}

                    {/* Best spread */}
                    <td style={{
                      padding: '4px 8px', textAlign: 'right',
                      fontVariantNumeric: 'tabular-nums', fontFamily: 'monospace',
                      color: spreadColor, fontWeight: row.best_spread_pct > 0.05 ? 600 : 400,
                    }}>
                      {fmtSpread(row.best_spread_pct)}
                    </td>
                  </tr>
                )
              })}
              {sorted.length === 0 && (
                <tr>
                  <td colSpan={9} style={{ padding: 20, color: '#333', textAlign: 'center' }}>
                    Нет данных
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Legend */}
      <div style={{ marginTop: 8, display: 'flex', gap: 16, fontSize: 10, color: '#333' }}>
        <span><span style={{ color: '#00ff87' }}>■</span> лучшая цена покупки (min ask)</span>
        <span><span style={{ color: '#4488ff' }}>■</span> лучшая цена продажи (max bid)</span>
        <span style={{ color: '#444' }}>Числа — ask цена (по которой покупаешь)</span>
      </div>
    </div>
  )
}
