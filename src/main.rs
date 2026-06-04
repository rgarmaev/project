mod arbitrage;
mod trade_store;
mod config;
mod dashboard;
mod exchanges;
mod market_scanner;
mod metrics;
mod multi_feed;
mod orderbook;
mod pricing;
mod risk;
mod tickers;
mod types;
mod doh;
mod withdrawal_status;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::info;
use tracing_subscriber::EnvFilter;

use arbitrage::{multi_detector::MultiPairDetector, executor::OrderExecutor};
use dashboard::state::DashboardState;
use exchanges::{
    binance::BinanceConnector,
    bitget::BitgetConnector,
    bybit::BybitConnector,
    gate::GateConnector,
    kucoin::KucoinConnector,
    mexc::MexcConnector,
    okx::OkxConnector,
};
use market_scanner::MarketScanner;
use metrics::MetricsCollector;
use orderbook::PriceState;
use trade_store::TradeStore;
use risk::RiskManager;
use types::MarketType;

const DASHBOARD_PORT: u16 = 3001;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("sol_arb=info")),
        )
        .init();

    let config = Arc::new(config::Config::load()?);
    info!(
        "SOL Arb starting | pair={} | paper={} | exchanges=Binance,Bybit,MEXC",
        config.pair(),
        config.trading.paper_trading
    );

    let price_state = Arc::new(PriceState::new());
    let (signal_tx, signal_rx) = mpsc::channel(1024);

    let binance = Arc::new(BinanceConnector::new(config.clone()));
    let bybit   = Arc::new(BybitConnector::new(config.clone()));
    let mexc    = Arc::new(MexcConnector::new(config.clone()));
    let okx     = Arc::new(OkxConnector::new(config.clone()));
    let bitget  = Arc::new(BitgetConnector::new(config.clone()));
    let kucoin  = Arc::new(KucoinConnector::new(config.clone()));
    let gate    = Arc::new(GateConnector::new(config.clone()));
    let risk    = Arc::new(RiskManager::new(config.risk.clone()));
    let trade_store = Arc::new(
        TradeStore::open("trades.db").expect("Failed to open trades.db")
    );
    let stored_stats = trade_store.load_stats().await
        .unwrap_or_else(|e| {
            tracing::warn!("Could not load stored stats: {}", e);
            trade_store::StoredStats {
                trade_count: 0, wins: 0,
                total_pnl: 0.0, total_fees: 0.0, total_gross: 0.0,
                total_exec_ms: 0, peak_pnl: 0.0,
            }
        });
    let metrics = Arc::new(MetricsCollector::with_initial(stored_stats));
    let scanner = MarketScanner::new();
    let multi_state    = multi_feed::new_state();
    let withdraw_state = withdrawal_status::new_status_map();

    let dash_state = DashboardState::new(
        price_state.clone(),
        metrics.clone(),
        config.clone(),
        scanner.clone(),
        multi_state.clone(),
        withdraw_state.clone(),
    );

    let detector = Arc::new(MultiPairDetector::new(
        config.clone(),
        multi_state.clone(),
        signal_tx,
    ));

    let executor = Arc::new(OrderExecutor::new(
        config.clone(),
        price_state.clone(),
        binance.clone(),
        bybit.clone(),
        mexc.clone(),
        okx.clone(),
        bitget.clone(),
        kucoin.clone(),
        gate.clone(),
        risk.clone(),
        metrics.clone(),
        dash_state.clone(),
    ));

    let mut set = JoinSet::new();

    // ── Price feeds ──────────────────────────────────────────────────────────
    {
        let (b, ps) = (binance.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Spot, ps).await });
    }
    {
        let (b, ps) = (binance.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Futures, ps).await });
    }
    {
        let (b, ps) = (bybit.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Spot, ps).await });
    }
    {
        let (b, ps) = (bybit.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Futures, ps).await });
    }
    {
        let (m, ps) = (mexc.clone(), price_state.clone());
        set.spawn(async move { m.run_feed(ps).await });
    }

    // ── Multi-pair feeds ──────────────────────────────────────────────────────
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_binance_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_binance_perp(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bybit_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bybit_linear(s).await });
    }

    // ── OKX + BingX feeds ────────────────────────────────────────────────────────
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_okx_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_okx_swap(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bingx_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bingx_swap(s).await });
    }

    // ── Bitget + KuCoin + Gate feeds ─────────────────────────────────────────
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bitget_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_bitget_futures(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_kucoin_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_kucoin_futures(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_gate_spot(s).await });
    }
    {
        let s = multi_state.clone();
        set.spawn(async move { multi_feed::run_gate_futures(s).await });
    }

    // ── Arbitrage engine ─────────────────────────────────────────────────────
    {
        let d = detector.clone();
        set.spawn(async move { d.run().await });
    }
    {
        let e = executor.clone();
        set.spawn(async move { e.run(signal_rx).await });
    }

    // ── Withdrawal status ────────────────────────────────────────────────────
    {
        let ws = withdraw_state.clone();
        set.spawn(async move { withdrawal_status::run_gate_poller(ws).await });
    }

    // ── Dashboard ────────────────────────────────────────────────────────────
    {
        let s = scanner.clone();
        set.spawn(async move { s.run().await });
    }
    {
        let ds = dash_state.clone();
        set.spawn(async move {
            dashboard::ws::broadcast_loop(ds).await;
        });
    }
    {
        let ds = dash_state.clone();
        set.spawn(async move {
            if let Err(e) = dashboard::serve(ds, DASHBOARD_PORT).await {
                tracing::error!("Dashboard server error: {}", e);
            }
        });
    }

    while let Some(res) = set.join_next().await {
        if let Err(e) = res {
            tracing::error!("Task panicked: {}", e);
        }
    }

    metrics.print_summary();
    Ok(())
}
