import { useEffect, useState, useCallback } from 'react'

interface TradeRow {
  id: string; symbol: string
  buy_exchange: string; buy_market_type: string
  sell_exchange: string; sell_market_type: string
  buy_ask: number; sell_bid: number
  spread_pct: number; quantity: number
  gross_pnl: number; fees: number; net_pnl: number
  exec_ms: number; completed_at: string
}

interface TradesPage { total: number; page: number; rows: TradeRow[] }

const EXCHANGES = ['Binance','Bybit','OKX','BingX','Bitget','KuCoin','Gate']

function fmt(v: number, d = 4) { return v.toFixed(d) }
function fmtTime(s: string) {
  const d = new Date(s)
  return `${d.toLocaleDateString('ru')} ${d.toLocaleTimeString('ru', { hour: '2-digit', minute: '2-digit', second: '2-digit' })}`
}

function MiniLineChart({ points }: { points: number[] }) {
  if (points.length < 2) return null
  const W = 800, H = 80, PAD = 4
  const min = Math.min(...points), max = Math.max(...points)
  const range = max - min || 1
  const xs = points.map((_, i) => PAD + (i / (points.length - 1)) * (W - PAD * 2))
  const ys = points.map(v => H - PAD - ((v - min) / range) * (H - PAD * 2))
  const d = xs.map((x, i) => `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${ys[i].toFixed(1)}`).join(' ')
  const last = points[points.length - 1]
  return (
    <svg viewBox={`0 0 ${W} ${H}`} style={{ width: '100%', height: 80 }}>
      <path d={d} fill="none" stroke="#00ff87" strokeWidth="1.5" />
      <text x={W - PAD} y={ys[ys.length - 1]} fill="#00ff87" fontSize="10" textAnchor="end" dy="-3">
        ${last.toFixed(4)}
      </text>
    </svg>
  )
}

export function TradesHistory() {
  const [page, setPage]           = useState(0)
  const [data, setData]           = useState<TradesPage | null>(null)
  const [symbol, setSymbol]       = useState('')
  const [buyEx, setBuyEx]         = useState('')
  const [sellEx, setSellEx]       = useState('')
  const [from, setFrom]           = useState('')
  const [to, setTo]               = useState('')
  const [minSpread, setMinSpread] = useState('')
  const [maxSpread, setMaxSpread] = useState('')
  const [loading, setLoading]     = useState(false)

  const load = useCallback(() => {
    setLoading(true)
    const p = new URLSearchParams()
    p.set('page', String(page))
    if (symbol)    p.set('symbol',       symbol.toUpperCase())
    if (buyEx)     p.set('buy_exchange',  buyEx)
    if (sellEx)    p.set('sell_exchange', sellEx)
    if (from)      p.set('from', new Date(from).toISOString())
    if (to)        p.set('to',   new Date(to + 'T23:59:59').toISOString())
    if (minSpread) p.set('min_spread', minSpread)
    if (maxSpread) p.set('max_spread', maxSpread)
    fetch(`/api/trades?${p}`)
      .then(r => r.json())
      .then((d: TradesPage) => { setData(d); setLoading(false) })
      .catch(() => setLoading(false))
  }, [page, symbol, buyEx, sellEx, from, to, minSpread, maxSpread])

  useEffect(() => { load() }, [load])

  function reset() {
    setSymbol(''); setBuyEx(''); setSellEx('')
    setFrom(''); setTo(''); setMinSpread(''); setMaxSpread('')
    setPage(0)
  }

  const chartData = (data?.rows ?? []).slice().reverse().map((r, i, arr) => ({
    t: fmtTime(r.completed_at),
    cum: arr.slice(0, i + 1).reduce((s, x) => s + x.net_pnl, 0),
  }))

  const totalPages = data ? Math.ceil(data.total / 50) : 0

  const inputStyle: React.CSSProperties = {
    background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
    color: '#e0e0e0', padding: '4px 8px', fontSize: 11, fontFamily: 'inherit',
    outline: 'none', width: 100,
  }
  const selectStyle: React.CSSProperties = { ...inputStyle, width: 90 }

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12, alignItems: 'center' }}>
        <input value={symbol} onChange={e => { setSymbol(e.target.value); setPage(0) }}
          placeholder="Символ" style={inputStyle} />
        <select value={buyEx} onChange={e => { setBuyEx(e.target.value); setPage(0) }} style={selectStyle}>
          <option value="">Купил (все)</option>
          {EXCHANGES.map(e => <option key={e} value={e}>{e}</option>)}
        </select>
        <select value={sellEx} onChange={e => { setSellEx(e.target.value); setPage(0) }} style={selectStyle}>
          <option value="">Продал (все)</option>
          {EXCHANGES.map(e => <option key={e} value={e}>{e}</option>)}
        </select>
        <input type="date" value={from} onChange={e => { setFrom(e.target.value); setPage(0) }}
          style={{ ...inputStyle, width: 120 }} />
        <input type="date" value={to} onChange={e => { setTo(e.target.value); setPage(0) }}
          style={{ ...inputStyle, width: 120 }} />
        <input value={minSpread} onChange={e => { setMinSpread(e.target.value); setPage(0) }}
          placeholder="Спред от %" style={{ ...inputStyle, width: 80 }} />
        <input value={maxSpread} onChange={e => { setMaxSpread(e.target.value); setPage(0) }}
          placeholder="до %" style={{ ...inputStyle, width: 60 }} />
        <button onClick={reset} style={{
          background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
          color: '#666', padding: '4px 10px', fontSize: 11, cursor: 'pointer',
        }}>Сбросить</button>
        <span style={{ color: '#444', fontSize: 11, marginLeft: 'auto' }}>
          {loading ? 'Загрузка...' : `Всего: ${data?.total ?? 0}`}
        </span>
      </div>

      <div style={{ overflowX: 'auto', overflowY: 'auto', maxHeight: 380 }}>
        <table style={{ borderCollapse: 'collapse', fontSize: 11, whiteSpace: 'nowrap', width: '100%' }}>
          <thead style={{ position: 'sticky', top: 0, background: '#111', zIndex: 1 }}>
            <tr style={{ borderBottom: '1px solid #222' }}>
              {['Время','Пара','Купил','Продал','Спред%','Кол-во','Gross','Комиссии','Net P&L','Exec ms'].map(h => (
                <th key={h} style={{ padding: '5px 8px', textAlign: 'right', color: '#555',
                  fontSize: 10, textTransform: 'uppercase', fontWeight: 400,
                  ...(h === 'Время' || h === 'Пара' ? { textAlign: 'left' } : {}) }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(data?.rows ?? []).map(r => (
              <tr key={r.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                <td style={{ padding: '4px 8px', color: '#555', fontFamily: 'monospace', fontSize: 10 }}>
                  {fmtTime(r.completed_at)}
                </td>
                <td style={{ padding: '4px 8px', color: '#ccc', fontWeight: 600 }}>
                  {r.symbol.replace('USDT', '')}<span style={{ color: '#333' }}>/U</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#4488ff', fontFamily: 'monospace' }}>
                  {r.buy_exchange}<span style={{ color: '#444' }}>:{r.buy_market_type}</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#00ff87', fontFamily: 'monospace' }}>
                  {r.sell_exchange}<span style={{ color: '#444' }}>:{r.sell_market_type}</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#aaaa00', fontFamily: 'monospace' }}>
                  {fmt(r.spread_pct, 3)}%
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#555', fontFamily: 'monospace' }}>
                  {fmt(r.quantity, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#666', fontFamily: 'monospace' }}>
                  {fmt(r.gross_pnl, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#444', fontFamily: 'monospace' }}>
                  {fmt(r.fees, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', fontFamily: 'monospace',
                  color: r.net_pnl >= 0 ? '#00ff87' : '#ff4444', fontWeight: 600 }}>
                  {r.net_pnl >= 0 ? '+' : ''}{fmt(r.net_pnl, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#444', fontFamily: 'monospace' }}>
                  {r.exec_ms}
                </td>
              </tr>
            ))}
            {(data?.rows ?? []).length === 0 && !loading && (
              <tr><td colSpan={10} style={{ padding: 20, color: '#333', textAlign: 'center' }}>
                Нет данных
              </td></tr>
            )}
          </tbody>
        </table>
      </div>

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 8, alignItems: 'center' }}>
        <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0}
          style={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
            color: page === 0 ? '#333' : '#666', padding: '3px 10px', fontSize: 11, cursor: 'pointer' }}>
          ←
        </button>
        <span style={{ color: '#444', fontSize: 11 }}>
          {page + 1} / {totalPages || 1}
        </span>
        <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
          disabled={page >= totalPages - 1}
          style={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
            color: page >= totalPages - 1 ? '#333' : '#666', padding: '3px 10px', fontSize: 11, cursor: 'pointer' }}>
          →
        </button>
      </div>

      {chartData.length > 1 && (
        <div style={{ marginTop: 16 }}>
          <div style={{ color: '#444', fontSize: 10, textTransform: 'uppercase',
            letterSpacing: '0.05em', marginBottom: 6 }}>
            Кумулятивный P&L (текущая страница)
          </div>
          <MiniLineChart points={chartData.map(d => d.cum)} />
        </div>
      )}
    </div>
  )
}
