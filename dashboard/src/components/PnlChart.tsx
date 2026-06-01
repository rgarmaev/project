import { useEffect, useState } from 'react'
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from 'recharts'
import { TradeRecord } from '../types'

interface ChartPoint {
  time: string
  pnl: number
}

interface Props {
  recentTrades: TradeRecord[]
}

function buildSeries(trades: TradeRecord[]): ChartPoint[] {
  const sorted = [...trades].sort((a, b) => new Date(a.time).getTime() - new Date(b.time).getTime())
  let cumulative = 0
  return sorted.map(t => {
    cumulative += t.net_pnl
    return {
      time: new Date(t.time).toLocaleTimeString(),
      pnl: Math.round(cumulative * 10000) / 10000,
    }
  })
}

export function PnlChart({ recentTrades }: Props) {
  const [history, setHistory] = useState<TradeRecord[]>([])

  useEffect(() => {
    fetch('/api/trades?limit=500')
      .then(r => r.json())
      .then((data: TradeRecord[]) => setHistory(data))
      .catch(() => {})
  }, [])

  const all = [...history, ...recentTrades.filter(t => !history.find(h => h.id === t.id))]
  const data = buildSeries(all)

  // 422 = (16+23+150+16) * 2 panels + 12 gap — matches ChartsRow height exactly
  const FIXED_H = 422

  return (
    <div style={{
      background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16,
      height: FIXED_H, display: 'flex', flexDirection: 'column',
    }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Cumulative PnL (USDT)
      </div>
      {data.length === 0 ? (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#333', fontSize: 13 }}>
          Waiting for first trade...
        </div>
      ) : (
        <div style={{ flex: 1, minHeight: 0 }}>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
              <XAxis dataKey="time" tick={{ fill: '#444', fontSize: 10 }} tickLine={false} axisLine={false} />
              <YAxis tick={{ fill: '#444', fontSize: 10 }} tickLine={false} axisLine={false} width={55} />
              <Tooltip
                contentStyle={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, fontSize: 12 }}
                labelStyle={{ color: '#888' }}
                itemStyle={{ color: '#00ff87' }}
              />
              <ReferenceLine y={0} stroke="#333" strokeDasharray="3 3" />
              <Line
                type="monotone"
                dataKey="pnl"
                stroke="#00ff87"
                strokeWidth={1.5}
                dot={false}
                activeDot={{ r: 3, fill: '#00ff87' }}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      )}
    </div>
  )
}
