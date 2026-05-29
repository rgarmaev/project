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

function PriceRow({ entry }: { entry: PriceEntry }) {
  const flashBid = useFlash(entry.bid)
  const flashAsk = useFlash(entry.ask)

  return (
    <tr style={{ borderBottom: '1px solid #1a1a1a' }}>
      <td style={{ padding: '6px 8px', color: '#888', fontSize: 12 }}>{entry.exchange}</td>
      <td style={{ padding: '6px 8px', color: '#555', fontSize: 12 }}>{entry.market}</td>
      <td style={{ padding: '6px 8px', color: flashBid ? '#ffff00' : '#00ff87', fontSize: 12, transition: 'color 200ms' }}>
        {entry.bid.toFixed(4)}
      </td>
      <td style={{ padding: '6px 8px', color: flashAsk ? '#ffff00' : '#ff4444', fontSize: 12, transition: 'color 200ms' }}>
        {entry.ask.toFixed(4)}
      </td>
      <td style={{ padding: '6px 8px', color: '#444', fontSize: 11 }}>
        {entry.spread_pct.toFixed(3)}%{entry.stale ? ' ⚠' : ''}
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
        Live Prices
      </div>
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            {['Exchange', 'Market', 'Bid', 'Ask', 'Spread'].map(h => (
              <th key={h} style={{ padding: '4px 8px', color: '#444', fontSize: 10, textAlign: 'left', textTransform: 'uppercase' }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.length === 0
            ? <tr><td colSpan={5} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>No price data</td></tr>
            : sorted.map(p => <PriceRow key={`${p.exchange}-${p.market}`} entry={p} />)
          }
        </tbody>
      </table>
    </div>
  )
}
