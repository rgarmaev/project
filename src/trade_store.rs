use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;

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
}
