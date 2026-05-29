import { useWebSocket } from './hooks/useWebSocket'
import { MetricsBar } from './components/MetricsBar'
import { PnlChart } from './components/PnlChart'
import { PriceTable } from './components/PriceTable'
import { TradesFeed } from './components/TradesFeed'
import { StatusBar } from './components/StatusBar'

const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`

const EMPTY_METRICS = {
  trades: 0, wins: 0, win_rate: 0,
  total_pnl: 0, total_fees: 0, peak_pnl: 0,
  max_drawdown: 0, avg_exec_ms: 0,
}

export default function App() {
  const { snapshot, status, lastUpdate } = useWebSocket(WS_URL)

  const metrics = snapshot?.metrics ?? EMPTY_METRICS
  const prices = snapshot?.prices ?? []
  const trades = snapshot?.recent_trades ?? []

  return (
    <div style={{ maxWidth: 1400, margin: '0 auto', padding: 20, display: 'flex', flexDirection: 'column', gap: 12 }}>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: '0.1em', color: '#e0e0e0' }}>
          SOL ARB
        </span>
        <span style={{ color: '#333', fontSize: 11 }}>
          {new Date().toLocaleTimeString()}
        </span>
      </div>

      <MetricsBar metrics={metrics} paperTrading={false} />

      <PnlChart recentTrades={trades} />

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <PriceTable prices={prices} />
        <TradesFeed trades={trades} />
      </div>

      <StatusBar status={status} lastUpdate={lastUpdate} />
    </div>
  )
}
