import { MetricsSnapshot } from '../types'

interface Props {
  metrics: MetricsSnapshot
  paperTrading: boolean
  effectiveMinSpreadPct: number
}

interface CardProps {
  label: string
  value: string
  color?: string
}

function Card({ label, value, color, title }: CardProps & { title?: string }) {
  return (
    <div title={title} style={{
      background: '#111',
      border: '1px solid #1f1f1f',
      borderRadius: 6,
      padding: '12px 16px',
      flex: 1,
      minWidth: 120,
      cursor: title ? 'help' : 'default',
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

export function MetricsBar({ metrics, paperTrading, effectiveMinSpreadPct }: Props) {
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
      <Card label="Gross PnL" value={`+${fmt(metrics.total_gross_pnl, 4)} USDT`} color="#888" />
      <Card label="Fees" value={`-${fmt(metrics.total_fees, 4)} USDT`} color="#666" />
      <Card label="Avg Exec" value={`${metrics.avg_exec_ms} ms`} />
      <Card
        label="Fee Ratio"
        value={`${fmt(metrics.fee_ratio * 100, 1)}%`}
        color={
          metrics.fee_ratio > 0.9  ? '#ff4444' :   // bad: >90% of gross eaten
          metrics.fee_ratio > 0.6  ? '#aaaa00' :   // marginal: 60-90%
          metrics.fee_ratio > 0    ? '#00ff87' :   // good: <60%
          '#444'
        }
      />
      <Card
        label="AS Min Spread"
        value={`${fmt(effectiveMinSpreadPct, 4)}%`}
        color={effectiveMinSpreadPct > 0.15 ? '#ff4444' : effectiveMinSpreadPct > 0.11 ? '#aaaa00' : '#00ff87'}
        title={`AS-2008: base_min + γ·σ²·τ = ${fmt(effectiveMinSpreadPct, 6)}%\nОбновляется с config.toml при перезапуске`}
      />
    </div>
  )
}
