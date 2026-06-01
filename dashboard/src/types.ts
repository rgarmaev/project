export interface MetricsSnapshot {
  trades: number
  wins: number
  win_rate: number
  total_pnl: number
  total_fees: number
  total_gross_pnl: number
  peak_pnl: number
  max_drawdown: number
  avg_exec_ms: number
  fee_ratio: number   // fees / gross — portion eaten by fees (0..1)
}

export interface PriceEntry {
  exchange: string
  market: string
  bid: number
  ask: number
  microprice: number    // Stoikov 2018: volume-weighted fair price
  spread_pct: number
  imbalance: number     // (bid_qty - ask_qty) / (bid_qty + ask_qty) ∈ [-1, 1]
  sigma_pct: number     // EWMA σ in %, e.g. 0.05 means 0.05%/tick
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
  buy_ask: number
  sell_bid: number
}

export interface OpportunityRow {
  symbol: string
  buy_market: string
  sell_market: string
  spread_pct: number   // gross spread %, e.g. 0.127 means 0.127%
  ask: number
  bid: number
}

export interface WsSnapshot {
  metrics: MetricsSnapshot
  prices: PriceEntry[]
  recent_trades: TradeRecord[]
  effective_min_spread_pct: number  // AS-2008 vol-adjusted threshold
  symbol: string
  top_opportunities: OpportunityRow[]
}

export interface MarketRow {
  symbol: string
  binance_ask: number
  binance_bid: number
  bybit_ask: number
  bybit_bid: number
  spread_ab: number
  spread_ba: number
}

export interface MarketSnapshot {
  spot: MarketRow[]
  perp: MarketRow[]
}
