export interface MetricsSnapshot {
  trades: number
  wins: number
  win_rate: number
  total_pnl: number
  total_fees: number
  peak_pnl: number
  max_drawdown: number
  avg_exec_ms: number
}

export interface PriceEntry {
  exchange: string
  market: string
  bid: number
  ask: number
  spread_pct: number
  stale: boolean
}

export interface TradeRecord {
  id: string
  buy_market: string
  sell_market: string
  spread_pct: number
  gross_pnl: number
  fees: number
  net_pnl: number
  exec_ms: number
  time: string
}

export interface WsSnapshot {
  metrics: MetricsSnapshot
  prices: PriceEntry[]
  recent_trades: TradeRecord[]
}
