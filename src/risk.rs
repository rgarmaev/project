use crate::config::RiskConfig;
use crate::types::ArbitrageSignal;
use parking_lot::Mutex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::{Duration, Instant};
use tracing::warn;

pub struct RiskManager {
    cfg: RiskConfig,
    state: Mutex<RiskState>,
}

struct RiskState {
    open_trades: usize,
    daily_pnl: Decimal,
    last_signal_at: Option<Instant>,
}

impl RiskManager {
    pub fn new(cfg: RiskConfig) -> Self {
        Self {
            cfg,
            state: Mutex::new(RiskState {
                open_trades: 0,
                daily_pnl: dec!(0),
                last_signal_at: None,
            }),
        }
    }

    /// Returns Ok(()) if the signal passes all risk checks.
    pub fn check(&self, signal: &ArbitrageSignal) -> Result<(), &'static str> {
        let state = self.state.lock();

        if state.open_trades >= self.cfg.max_open_trades {
            warn!("Risk: max open trades reached ({})", self.cfg.max_open_trades);
            return Err("max_open_trades");
        }

        if state.daily_pnl < -self.cfg.max_daily_loss_usdt {
            warn!("Risk: daily loss limit hit ({})", state.daily_pnl);
            return Err("daily_loss_limit");
        }

        if signal.expected_pnl_usdt <= dec!(0) {
            return Err("non_positive_pnl");
        }

        if let Some(last) = state.last_signal_at {
            if last.elapsed() < Duration::from_millis(self.cfg.cooldown_ms) {
                return Err("cooldown");
            }
        }

        Ok(())
    }

    pub fn on_trade_open(&self) {
        let mut s = self.state.lock();
        s.open_trades += 1;
        s.last_signal_at = Some(Instant::now());
    }

    pub fn on_trade_close(&self, net_pnl: Decimal) {
        let mut s = self.state.lock();
        s.open_trades = s.open_trades.saturating_sub(1);
        s.daily_pnl += net_pnl;
    }

    pub fn daily_pnl(&self) -> Decimal {
        self.state.lock().daily_pnl
    }
}
