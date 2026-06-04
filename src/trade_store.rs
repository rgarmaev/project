use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;
use crate::types::CompletedTrade;
use tokio::task;

pub struct TradeStore {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

pub struct StoredStats {
    pub trade_count:   usize,
    pub wins:          usize,
    pub total_pnl:     f64,
    pub total_fees:    f64,
    pub total_gross:   f64,
    pub total_exec_ms: u64,
    pub peak_pnl:      f64,
}

impl TradeStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS trades (
                id               TEXT PRIMARY KEY,
                symbol           TEXT    NOT NULL,
                buy_exchange     TEXT    NOT NULL,
                buy_market_type  TEXT    NOT NULL,
                sell_exchange    TEXT    NOT NULL,
                sell_market_type TEXT    NOT NULL,
                buy_ask          REAL    NOT NULL,
                sell_bid         REAL    NOT NULL,
                spread_pct       REAL    NOT NULL,
                quantity         REAL    NOT NULL,
                gross_pnl        REAL    NOT NULL,
                fees             REAL    NOT NULL,
                net_pnl          REAL    NOT NULL,
                exec_ms          INTEGER NOT NULL,
                completed_at     TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trades_time   ON trades(completed_at);
            CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
        ")?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub(crate) fn insert_sync(&self,
        id: &str, symbol: &str,
        buy_exchange: &str, buy_market_type: &str,
        sell_exchange: &str, sell_market_type: &str,
        buy_ask: f64, sell_bid: f64, spread_pct: f64, quantity: f64,
        gross_pnl: f64, fees: f64, net_pnl: f64,
        exec_ms: u64, completed_at: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO trades (
                id, symbol, buy_exchange, buy_market_type,
                sell_exchange, sell_market_type,
                buy_ask, sell_bid, spread_pct, quantity,
                gross_pnl, fees, net_pnl, exec_ms, completed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            rusqlite::params![
                id, symbol, buy_exchange, buy_market_type,
                sell_exchange, sell_market_type,
                buy_ask, sell_bid, spread_pct, quantity,
                gross_pnl, fees, net_pnl, exec_ms as i64, completed_at
            ],
        )?;
        Ok(())
    }

    pub async fn insert(&self, trade: &CompletedTrade) -> Result<()> {
        let id            = trade.id.to_string();
        let symbol        = trade.signal.symbol.clone();
        let buy_ex        = trade.signal.buy_market.exchange.to_string();
        let buy_mt        = trade.signal.buy_market.market_type.to_string();
        let sell_ex       = trade.signal.sell_market.exchange.to_string();
        let sell_mt       = trade.signal.sell_market.market_type.to_string();
        let buy_ask: f64  = trade.signal.buy_ask.to_string().parse().unwrap_or(0.0);
        let sell_bid: f64 = trade.signal.sell_bid.to_string().parse().unwrap_or(0.0);
        let spread: f64   = trade.signal.spread_pct.to_string().parse().unwrap_or(0.0);
        let qty: f64      = trade.signal.quantity.to_string().parse().unwrap_or(0.0);
        let gross: f64    = trade.gross_pnl.to_string().parse().unwrap_or(0.0);
        let fees: f64     = trade.fees.to_string().parse().unwrap_or(0.0);
        let net: f64      = trade.net_pnl.to_string().parse().unwrap_or(0.0);
        let exec_ms       = trade.exec_ms;
        let completed_at  = trade.completed_at.to_rfc3339();
        let store         = self.conn.clone();
        task::spawn_blocking(move || {
            let conn = store.lock();
            conn.execute(
                "INSERT OR IGNORE INTO trades (
                    id, symbol, buy_exchange, buy_market_type,
                    sell_exchange, sell_market_type,
                    buy_ask, sell_bid, spread_pct, quantity,
                    gross_pnl, fees, net_pnl, exec_ms, completed_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                rusqlite::params![
                    id, symbol, buy_ex, buy_mt, sell_ex, sell_mt,
                    buy_ask, sell_bid, spread, qty, gross, fees, net,
                    exec_ms as i64, completed_at
                ],
            )?;
            Ok::<_, anyhow::Error>(())
        }).await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_table() {
        let store = TradeStore::open(":memory:").unwrap();
        let conn = store.conn.lock();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trades", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn insert_stores_row() {
        let store = TradeStore::open(":memory:").unwrap();
        store.insert_sync("uuid-1", "BTCUSDT", "Binance", "Spot", "OKX", "Spot",
            65000.0, 65100.0, 0.15, 0.001, 0.10, 0.02, 0.08, 42, "2026-06-04T10:00:00Z"
        ).unwrap();
        let conn = store.conn.lock();
        let net: f64 = conn.query_row(
            "SELECT net_pnl FROM trades WHERE id = 'uuid-1'", [], |r| r.get(0)
        ).unwrap();
        assert!((net - 0.08).abs() < 1e-9);
    }
}
