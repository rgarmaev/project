mod arbitrage;
mod config;
mod dashboard;
mod exchanges;
mod metrics;
mod orderbook;
mod pricing;
mod risk;
mod types;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::info;
use tracing_subscriber::EnvFilter;

use arbitrage::{detector::ArbitrageDetector, executor::OrderExecutor};
use dashboard::state::DashboardState;
use exchanges::{binance::BinanceConnector, bybit::BybitConnector, mexc::MexcConnector};
use metrics::MetricsCollector;
use orderbook::PriceState;
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
    let (signal_tx, signal_rx) = mpsc::channel(256);

    let binance = Arc::new(BinanceConnector::new(config.clone()));
    let bybit   = Arc::new(BybitConnector::new(config.clone()));
    let mexc    = Arc::new(MexcConnector::new(config.clone()));
    let risk    = Arc::new(RiskManager::new(config.risk.clone()));
    let metrics = Arc::new(MetricsCollector::new());

    let dash_state = DashboardState::new(price_state.clone(), metrics.clone());

    let detector = Arc::new(ArbitrageDetector::new(
        config.clone(),
        price_state.clone(),
        signal_tx,
    ));

    let executor = Arc::new(OrderExecutor::new(
        config.clone(),
        price_state.clone(),
        binance.clone(),
        bybit.clone(),
        mexc.clone(),
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

    // ── Arbitrage engine ─────────────────────────────────────────────────────
    {
        let d = detector.clone();
        set.spawn(async move { d.run().await });
    }
    {
        let e = executor.clone();
        set.spawn(async move { e.run(signal_rx).await });
    }

    // ── Dashboard ────────────────────────────────────────────────────────────
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
