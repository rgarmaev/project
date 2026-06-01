import {
  LineChart, Line, XAxis, YAxis, Tooltip,
  ResponsiveContainer, ReferenceLine,
} from 'recharts'
import { PriceEntry } from '../types'
import { usePriceHistory, PricePoint } from '../hooks/usePriceHistory'

interface Props {
  prices: PriceEntry[]
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

// ── Chart 1: Live spread % between market pairs ─────────────────────────────

const ROUTES = [
  { label: 'Bin.S→Byb.S', buyKey: 'Binance:Spot', sellKey: 'Bybit:Spot',   color: '#00ff87' },
  { label: 'Byb.S→Bin.S', buyKey: 'Bybit:Spot',   sellKey: 'Binance:Spot', color: '#4488ff' },
  { label: 'Bin.P→Byb.P', buyKey: 'Binance:Perp', sellKey: 'Bybit:Perp',   color: '#ffaa00' },
  { label: 'Byb.P→Bin.P', buyKey: 'Bybit:Perp',   sellKey: 'Binance:Perp', color: '#ff44aa' },
]

function SpreadChart({ history }: { history: Map<string, PricePoint[]> }) {
  const allKeys = [...new Set(ROUTES.flatMap(r => [r.buyKey, r.sellKey]))]
  const minLen = Math.min(...allKeys.map(k => history.get(k)?.length ?? 0))

  if (minLen === 0) {
    return (
      <div style={PANEL}>
        <div style={LABEL}>Spread · All Routes (%)</div>
        {EMPTY}
      </div>
    )
  }

  const data = Array.from({ length: minLen }, (_, i) => {
    const point: Record<string, number | string> = { time: history.get(allKeys[0])![i].time }
    for (const r of ROUTES) {
      const buy  = history.get(r.buyKey)![i]
      const sell = history.get(r.sellKey)![i]
      if (buy && sell && buy.ask > 0) {
        point[r.label] = Math.round((sell.bid - buy.ask) / buy.ask * 1_000_000) / 10_000
      }
    }
    return point
  })

  return (
    <div style={PANEL}>
      <div style={LABEL}>Spread · All Routes (%)</div>
      <ResponsiveContainer width="100%" height={150}>
        <LineChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
          <XAxis dataKey="time" tick={AXIS_TICK} tickLine={false} axisLine={false} interval="preserveStartEnd" />
          <YAxis tick={AXIS_TICK} tickLine={false} axisLine={false} width={65} domain={['auto', 'auto']}
                 tickFormatter={(v: number) => `${v.toFixed(2)}%`} />
          <Tooltip {...TOOLTIP_STYLE} formatter={(v: number) => `${v.toFixed(4)}%`} />
          <ReferenceLine y={0} stroke="#333" strokeDasharray="3 3" />
          {ROUTES.map(r => (
            <Line key={r.label} type="monotone" dataKey={r.label} stroke={r.color}
                  strokeWidth={1} dot={false} name={r.label} />
          ))}
        </LineChart>
      </ResponsiveContainer>
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
  const len = spot.length

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

export function ChartsRow({ prices }: Props) {
  const history = usePriceHistory(prices)

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
      <div style={{ gridColumn: '1 / -1' }}>
        <SpreadChart history={history} />
      </div>
      <ExchangeChart title="Binance · Bid / Ask" history={history} spotKey="Binance:Spot" perpKey="Binance:Perp" />
      <ExchangeChart title="Bybit · Bid / Ask"   history={history} spotKey="Bybit:Spot"   perpKey="Bybit:Perp" />
    </div>
  )
}
