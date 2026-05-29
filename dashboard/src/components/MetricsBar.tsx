import { MetricsSnapshot } from '../types'

interface Props {
  metrics: MetricsSnapshot
  paperTrading: boolean
}

interface CardProps {
  label: string
  value: string
  color?: string
}

function Card({ label, value, color }: CardProps) {
  return (
    <div style={{
      background: '#111',
      border: '1px solid #1f1f1f',
      borderRadius: 6,
      padding: '12px 16px',
      flex: 1,
      minWidth: 120,
    }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 6 }}>
        {label}
      </div>
      <div style={{ color: color ?? '#e0e0e0', fontSize: 20, fontWeight: 600 }}>
        {value}
      </div>
    </div>
  )
}

function pnlColor(v: number) {
  return v > 0 ? '#00ff87' : v < 0 ? '#ff4444' : '#e0e0e0'
}

function fmt(v: number, decimals = 2) {
  return v.toFixed(decimals)
}

export function MetricsBar({ metrics, paperTrading }: Props) {
  return (
    <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
      {paperTrading && (
        <div style={{
          alignSelf: 'center',
          background: '#1a1a00',
          border: '1px solid #444400',
          borderRadius: 4,
          padding: '4px 8px',
          color: '#aaaa00',
          fontSize: 11,
          whiteSpace: 'nowrap',
        }}>
          PAPER
        </div>
      )}
      <Card label="Total PnL" value={`${metrics.total_pnl >= 0 ? '+' : ''}${fmt(metrics.total_pnl, 4)} USDT`} color={pnlColor(metrics.total_pnl)} />
      <Card label="Win Rate" value={`${fmt(metrics.win_rate, 1)}%`} />
      <Card label="Max Drawdown" value={`${fmt(metrics.max_drawdown, 4)} USDT`} color={metrics.max_drawdown > 0 ? '#ff4444' : '#e0e0e0'} />
      <Card label="Trades" value={String(metrics.trades)} />
      <Card label="Fees" value={`${fmt(metrics.total_fees, 4)} USDT`} color="#888" />
      <Card label="Avg Exec" value={`${metrics.avg_exec_ms} ms`} />
    </div>
  )
}
