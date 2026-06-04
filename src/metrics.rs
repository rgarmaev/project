use crate::trade_store::StoredStats;
use crate::types::CompletedTrade;
use parking_lot::Mutex;
use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;
use tracing::info;

pub struct MetricsCollector {
    inner: Mutex<Inner>,
}

struct Inner {
    trades: usize,
    wins: usize,
    total_pnl: Decimal,
    total_fees: Decimal,
    total_gross_pnl: Decimal,
    peak_pnl: Decimal,
    max_drawdown: Decimal,
    total_exec_ms: u64,
    // Signal pipeline counters (order-to-trade ratio)
    signals_sent: u64,       // passed detector filters, queued to executor
    rejected_cooldown: u64,  // blocked by risk cooldown
    rejected_risk: u64,      // blocked by risk limits (position/daily loss/max open)
}

#[derive(Serialize, Clone)]
pub struct MetricsSnapshot {
    pub trades: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub total_gross_pnl: f64,
    pub peak_pnl: f64,
    pub max_drawdown: f64,
    pub avg_exec_ms: u64,
    /// fees / gross_pnl — portion of gross eaten by fees (0..1)
    pub fee_ratio: f64,
}

fn to_f64(d: Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                trades: 0,
                wins: 0,
                total_pnl: dec!(0),
                total_fees: dec!(0),
                total_gross_pnl: dec!(0),
                peak_pnl: dec!(0),
                max_drawdown: dec!(0),
                total_exec_ms: 0,
                signals_sent: 0,
                rejected_cooldown: 0,
                rejected_risk: 0,
            }),
        }
    }

    pub fn with_initial(s: StoredStats) -> Self {
        let to_dec = |v: f64| Decimal::from_str(&format!("{:.8}", v)).unwrap_or(dec!(0));
        Self {
            inner: Mutex::new(Inner {
                trades:          s.trade_count,
                wins:            s.wins,
                total_pnl:       to_dec(s.total_pnl),
                total_fees:      to_dec(s.total_fees),
                total_gross_pnl: to_dec(s.total_gross),
                peak_pnl:        to_dec(s.peak_pnl),
                max_drawdown:    to_dec(s.max_drawdown),
                total_exec_ms:   s.total_exec_ms,
                signals_sent:    0,
                rejected_cooldown: 0,
                rejected_risk:   0,
            }),
        }
    }

    pub fn record(&self, trade: &CompletedTrade) {
        let mut m = self.inner.lock();
        m.trades += 1;
        if trade.net_pnl > dec!(0) {
            m.wins += 1;
        }
        m.total_pnl += trade.net_pnl;
        m.total_fees += trade.fees;
        m.total_gross_pnl += trade.gross_pnl;
        m.total_exec_ms += trade.exec_ms;

        if m.total_pnl > m.peak_pnl {
            m.peak_pnl = m.total_pnl;
        }
        let drawdown = m.peak_pnl - m.total_pnl;
        if drawdown > m.max_drawdown {
            m.max_drawdown = drawdown;
        }

        let win_rate = if m.trades > 0 {
            Decimal::from(m.wins * 100) / Decimal::from(m.trades)
        } else {
            dec!(0)
        };

        info!(
            "Trade #{} | net_pnl={:.4} USDT | total={:.4} | win_rate={:.1}% | drawdown={:.4} | exec={}ms",
            m.trades, trade.net_pnl, m.total_pnl, win_rate, m.max_drawdown, trade.exec_ms
        );
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let m = self.inner.lock();
        let win_rate = if m.trades > 0 {
            m.wins as f64 * 100.0 / m.trades as f64
        } else {
            0.0
        };
        let avg_exec_ms = if m.trades > 0 { m.total_exec_ms / m.trades as u64 } else { 0 };
        let gross = to_f64(m.total_gross_pnl);
        let fees  = to_f64(m.total_fees);
        let fee_ratio = if gross > 0.0 { fees / gross } else { 0.0 };
        MetricsSnapshot {
            trades: m.trades,
            wins: m.wins,
            win_rate,
            total_pnl: to_f64(m.total_pnl),
            total_fees: fees,
            total_gross_pnl: gross,
            peak_pnl: to_f64(m.peak_pnl),
            max_drawdown: to_f64(m.max_drawdown),
            avg_exec_ms,
            fee_ratio,
        }
    }

    pub fn print_summary(&self) {
        let snap = self.snapshot();
        info!(
            "=== Summary === trades={} win_rate={:.1}% pnl={:.4} fees={:.4} max_dd={:.4} avg_exec={}ms",
            snap.trades, snap.win_rate, snap.total_pnl, snap.total_fees, snap.max_drawdown, snap.avg_exec_ms
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ArbitrageSignal, Exchange, MarketId, MarketType, OrderResult, Side};
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_trade(net_pnl: Decimal, exec_ms: u64) -> CompletedTrade {
        let market = MarketId::new(Exchange::Binance, MarketType::Spot);
        let signal = ArbitrageSignal {
            id: Uuid::new_v4(),
            symbol: "BTCUSDT".to_string(),
            buy_market: market,
            sell_market: market,
            buy_ask: dec!(100),
            sell_bid: dec!(101),
            spread_pct: dec!(1),
            quantity: dec!(1),
            expected_pnl_usdt: dec!(1),
            detected_at: Utc::now(),
        };
        let order = OrderResult {
            exchange: Exchange::Binance,
            market_type: MarketType::Spot,
            order_id: "test".into(),
            side: Side::Buy,
            filled_qty: dec!(1),
            avg_price: dec!(100),
            fee_usdt: dec!(0.1),
            timestamp: Utc::now(),
        };
        CompletedTrade {
            id: Uuid::new_v4(),
            signal,
            buy_order: order.clone(),
            sell_order: order,
            gross_pnl: net_pnl + dec!(0.2),
            fees: dec!(0.2),
            net_pnl,
            exec_ms,
            completed_at: Utc::now(),
        }
    }

    #[test]
    fn avg_exec_ms_computed_correctly() {
        let mc = MetricsCollector::new();
        mc.record(&make_trade(dec!(1), 100));
        mc.record(&make_trade(dec!(1), 200));
        let snap = mc.snapshot();
        assert_eq!(snap.avg_exec_ms, 150);
    }

    #[test]
    fn win_rate_zero_on_no_trades() {
        let mc = MetricsCollector::new();
        assert_eq!(mc.snapshot().win_rate, 0.0);
    }

    #[test]
    fn with_initial_seeds_counters() {
        use crate::trade_store::StoredStats;
        let stats = StoredStats {
            trade_count: 100, wins: 98,
            total_pnl: 50.0, total_fees: 5.0, total_gross: 55.0,
            total_exec_ms: 5000, peak_pnl: 52.0, max_drawdown: 3.0,
        };
        let mc = MetricsCollector::with_initial(stats);
        let snap = mc.snapshot();
        assert_eq!(snap.trades, 100);
        assert!((snap.total_pnl  - 50.0).abs() < 1e-4);
        assert!((snap.peak_pnl   - 52.0).abs() < 1e-4);
    }
}
