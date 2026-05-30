import { useEffect, useState } from 'react'
import {
  LineChart, Line, XAxis, YAxis, Tooltip,
  ResponsiveContainer,
} from 'recharts'
import { PriceEntry, TradeRecord } from '../types'
import { usePriceHistory, PricePoint } from '../hooks/usePriceHistory'

interface Props {
  prices: PriceEntry[]
  trades: TradeRecord[]
}

const PANEL: React.CSSProperties = {
  background: '#111',
  border: '1px solid #1f1f1f',
  borderRadius: 6,
  padding: 16,
}

const LABEL: React.CSSProperties = {
  color: '#666',
  fontSize: 11,
  textTransform: 'uppercase',
  letterSpacing: '0.05em',
  marginBottom: 12,
}

const AXIS_TICK = { fill: '#444', fontSize: 10 }

const TOOLTIP_STYLE = {
  contentStyle: { background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, fontSize: 12 },
  labelStyle: { color: '#888' },
}

const EMPTY = (
  <div style={{ color: '#333', textAlign: 'center' as const, padding: '40px 0', fontSize: 13 }}>
    Waiting for data…
  </div>
)

// ── Chart 1: Buy / Sell prices from completed trades ────────────────────────

function BuySellChart({ trades }: { trades: TradeRecord[] }) {
  const [history, setHistory] = useState<TradeRecord[]>([])

  useEffect(() => {
    fetch('/api/trades?limit=500')
      .then(r => r.json())
      .then((data: TradeRecord[]) => setHistory(data))
      .catch(() => {})
  }, [])

  const all = [...history, ...trades.filter(t => !history.find(h => h.id === t.id))]
  const data = [...all]
    .sort((a, b) => new Date(a.time).getTime() - new Date(b.time).getTime())
    .map(t => ({
      time: new Date(t.time).toLocaleTimeString(),
      buy: Math.round(t.buy_ask * 1000) / 1000,
      sell: Math.round(t.sell_bid * 1000) / 1000,
    }))

  return (
    <div style={PANEL}>
      <div style={LABEL}>Buy / Sell Prices · All Routes</div>
      {data.length === 0 ? EMPTY : (
        <ResponsiveContainer width="100%" height={150}>
          <LineChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
            <XAxis dataKey="time" tick={AXIS_TICK} tickLine={false} axisLine={false} interval="preserveStartEnd" />
            <YAxis tick={AXIS_TICK} tickLine={false} axisLine={false} width={65} domain={['auto', 'auto']} />
            <Tooltip {...TOOLTIP_STYLE} />
            <Line type="monotone" dataKey="buy" stroke="#ff4444" strokeWidth={1.5} dot={{ r: 2, fill: '#ff4444' }} name="Buy ask" />
            <Line type="monotone" dataKey="sell" stroke="#00ff87" strokeWidth={1.5} dot={{ r: 2, fill: '#00ff87' }} name="Sell bid" />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  )
}

// ── Charts 2–4: Live bid/ask per exchange ───────────────────────────────────

function ExchangeChart({
  title,
  history,
  spotKey,
  perpKey,
}: {
  title: string
  history: Map<string, PricePoint[]>
  spotKey: string
  perpKey?: string
}) {
  const spot = history.get(spotKey) ?? []
  const perp = perpKey ? (history.get(perpKey) ?? []) : []
  const len = perpKey ? Math.min(spot.length, perp.length) : spot.length

  const data = Array.from({ length: len }, (_, i) => ({
    time: spot[i].time,
    spot_bid: spot[i].bid,
    spot_ask: spot[i].ask,
    ...(perpKey && perp[i] ? { perp_bid: perp[i].bid, perp_ask: perp[i].ask } : {}),
  }))

  return (
    <div style={PANEL}>
      <div style={LABEL}>{title}</div>
      {data.length === 0 ? EMPTY : (
        <ResponsiveContainer width="100%" height={150}>
          <LineChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
            <XAxis dataKey="time" tick={AXIS_TICK} tickLine={false} axisLine={false} interval="preserveStartEnd" />
            <YAxis tick={AXIS_TICK} tickLine={false} axisLine={false} width={65} domain={['auto', 'auto']} />
            <Tooltip {...TOOLTIP_STYLE} />
            <Line type="monotone" dataKey="spot_bid" stroke="#00ff87" strokeWidth={1.5} dot={false} name="Spot bid" />
            <Line type="monotone" dataKey="spot_ask" stroke="#ff4444" strokeWidth={1.5} dot={false} name="Spot ask" />
            {perpKey && (
              <>
                <Line type="monotone" dataKey="perp_bid" stroke="#00cc6a" strokeWidth={1.5} strokeDasharray="4 2" dot={false} name="Perp bid" />
                <Line type="monotone" dataKey="perp_ask" stroke="#cc3333" strokeWidth={1.5} strokeDasharray="4 2" dot={false} name="Perp ask" />
              </>
            )}
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  )
}

// ── Container ───────────────────────────────────────────────────────────────

export function ChartsRow({ prices, trades }: Props) {
  const history = usePriceHistory(prices)

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
      <BuySellChart trades={trades} />
      <ExchangeChart title="Binance · Bid / Ask" history={history} spotKey="Binance:Spot" perpKey="Binance:Perp" />
      <ExchangeChart title="Bybit · Bid / Ask"   history={history} spotKey="Bybit:Spot"   perpKey="Bybit:Perp" />
      <ExchangeChart title="MEXC · Bid / Ask"    history={history} spotKey="MEXC:Spot" />
    </div>
  )
}
