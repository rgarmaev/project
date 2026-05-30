import { useEffect, useRef, useState } from 'react'
import { PriceEntry } from '../types'

interface Props {
  prices: PriceEntry[]
}

function useFlash(value: number) {
  const [flash, setFlash] = useState(false)
  const prev = useRef(value)
  useEffect(() => {
    if (prev.current !== value) {
      prev.current = value
      setFlash(true)
      const t = setTimeout(() => setFlash(false), 200)
      return () => clearTimeout(t)
    }
  }, [value])
  return flash
}

function imbalanceBar(imb: number) {
  const pct = Math.abs(imb) * 100
  const color = imb > 0 ? '#00ff87' : '#ff4444'
  const direction = imb > 0 ? 'left' : 'right'
  return (
    <div style={{ position: 'relative', width: 40, height: 8, background: '#1a1a1a', borderRadius: 2 }}>
      <div style={{
        position: 'absolute',
        [direction]: 0,
        width: `${pct}%`,
        height: '100%',
        background: color,
        borderRadius: 2,
        opacity: 0.8,
      }} />
    </div>
  )
}

function sigmaColor(sigma: number) {
  if (sigma > 0.05) return '#ff4444'   // high vol  >5bp/tick
  if (sigma > 0.01) return '#aaaa00'   // medium    1-5bp/tick
  if (sigma > 0)    return '#00ff87'   // low       <1bp/tick (warmed up)
  return '#333'                         // no data
}

function PriceRow({ entry }: { entry: PriceEntry }) {
  const flashBid = useFlash(entry.bid)
  const flashAsk = useFlash(entry.ask)
  const mpDiff = entry.bid > 0 ? ((entry.microprice - entry.bid) / entry.bid) * 10000 : 0

  return (
    <tr style={{ borderBottom: '1px solid #1a1a1a' }}>
      <td style={{ padding: '6px 8px', color: '#888', fontSize: 12 }}>{entry.exchange}</td>
      <td style={{ padding: '6px 8px', color: '#555', fontSize: 12 }}>{entry.market}</td>
      <td style={{ padding: '6px 8px', color: flashBid ? '#ffff00' : '#00ff87', fontSize: 12, transition: 'color 200ms' }}>
        {entry.bid.toFixed(3)}
      </td>
      <td style={{ padding: '6px 8px', color: flashAsk ? '#ffff00' : '#ff4444', fontSize: 12, transition: 'color 200ms' }}>
        {entry.ask.toFixed(3)}
      </td>
      <td style={{ padding: '6px 8px', fontSize: 11 }}>
        <span style={{ color: mpDiff > 0 ? '#00ff87' : mpDiff < 0 ? '#ff4444' : '#555' }}>
          {entry.microprice.toFixed(3)}
        </span>
        <span style={{ color: '#333', fontSize: 10, marginLeft: 2 }}>
          ({mpDiff > 0 ? '+' : ''}{mpDiff.toFixed(1)} bp)
        </span>
      </td>
      <td style={{ padding: '6px 8px', verticalAlign: 'middle' }}>
        {imbalanceBar(entry.imbalance)}
        <span style={{ color: '#444', fontSize: 10, marginLeft: 4 }}>
          {entry.imbalance > 0 ? '+' : ''}{(entry.imbalance * 100).toFixed(0)}%
        </span>
      </td>
      <td style={{ padding: '6px 8px', color: sigmaColor(entry.sigma_pct), fontSize: 11 }}
          title={entry.sigma_pct > 0 ? `EWMA σ = ${entry.sigma_pct.toFixed(5)}%/tick` : 'Warming up...'}>
        {entry.sigma_pct > 0 ? `${(entry.sigma_pct * 100).toFixed(2)}bp` : '…'}
        {entry.stale ? ' ⚠' : ''}
      </td>
    </tr>
  )
}

const MARKET_ORDER = ['Binance:Spot', 'Binance:Perp', 'Bybit:Spot', 'Bybit:Perp', 'MEXC:Spot']

export function PriceTable({ prices }: Props) {
  const sorted = [...prices].sort((a, b) => {
    const ia = MARKET_ORDER.indexOf(`${a.exchange}:${a.market}`)
    const ib = MARKET_ORDER.indexOf(`${b.exchange}:${b.market}`)
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib)
  })

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Live Prices · Microprice · Imbalance · σ (EWMA)
      </div>
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            {['Exchange', 'Market', 'Bid', 'Ask', 'Microprice', 'Imbalance', 'σ/tick'].map(h => (
              <th key={h} style={{ padding: '4px 8px', color: '#444', fontSize: 10, textAlign: 'left', textTransform: 'uppercase' }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.length === 0
            ? <tr><td colSpan={7} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>No price data</td></tr>
            : sorted.map(p => <PriceRow key={`${p.exchange}-${p.market}`} entry={p} />)
          }
        </tbody>
      </table>
    </div>
  )
}
