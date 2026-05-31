import { useState } from 'react'
import { TickerSelector } from './components/TickerSelector'
import { useWebSocket } from './hooks/useWebSocket'
import { MetricsBar } from './components/MetricsBar'
import { PnlChart } from './components/PnlChart'
import { PriceTable } from './components/PriceTable'
import { TradesFeed } from './components/TradesFeed'
import { StatusBar } from './components/StatusBar'
import { SettingsPage } from './components/SettingsPage'
import { ChartsRow } from './components/ChartsRow'

const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`

const EMPTY_METRICS = {
  trades: 0, wins: 0, win_rate: 0,
  total_pnl: 0, total_fees: 0, total_gross_pnl: 0,
  peak_pnl: 0, max_drawdown: 0, avg_exec_ms: 0,
  fee_ratio: 0,
}

type Tab = 'dashboard' | 'settings'

function NavTab({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} style={{
      background: 'none', border: 'none', cursor: 'pointer', padding: '6px 14px',
      color: active ? '#e0e0e0' : '#444', fontSize: 12, fontFamily: 'inherit',
      borderBottom: active ? '2px solid #00ff87' : '2px solid transparent',
    }}>
      {label}
    </button>
  )
}

export default function App() {
  const [tab, setTab] = useState<Tab>('dashboard')
  const [pendingSymbol, setPendingSymbol] = useState<string | null>(null)
  const { snapshot, status, lastUpdate } = useWebSocket(WS_URL)

  const symbol = snapshot?.symbol ?? 'SOLUSDT'
  const base = symbol.replace('USDT', '')

  async function changeTicker(newSymbol: string) {
    if (newSymbol === symbol) return
    try {
      const r = await fetch('/api/config')
      const cfg = await r.json()
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...cfg, symbol: newSymbol }),
      })
      setPendingSymbol(newSymbol)
    } catch (_) {}
  }

  async function restart() {
    try { await fetch('/api/restart', { method: 'POST' }) } catch (_) {}
    setTimeout(() => window.location.reload(), 3000)
  }

  const metrics = snapshot?.metrics ?? EMPTY_METRICS
  const prices = snapshot?.prices ?? []
  const trades = snapshot?.recent_trades ?? []
  const effectiveMinSpreadPct = snapshot?.effective_min_spread_pct ?? 0

  return (
    <div style={{ maxWidth: 1400, margin: '0 auto', padding: 20, display: 'flex', flexDirection: 'column', gap: 12 }}>

      {/* Header + Nav */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
          <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: '0.1em', color: '#e0e0e0', marginRight: 12 }}>
            {base} ARB
          </span>
          <TickerSelector current={symbol} onChange={changeTicker} />
          <div style={{ width: 12 }} />
          <NavTab label="Дашборд" active={tab === 'dashboard'} onClick={() => setTab('dashboard')} />
          <NavTab label="Настройки" active={tab === 'settings'} onClick={() => setTab('settings')} />
        </div>
        <span style={{ color: '#333', fontSize: 11 }}>
          {new Date().toLocaleTimeString()}
        </span>
      </div>

      {tab === 'settings' ? (
        <SettingsPage />
      ) : (
        <>
          <MetricsBar metrics={metrics} paperTrading={false} effectiveMinSpreadPct={effectiveMinSpreadPct} />
          {pendingSymbol && (
            <div style={{
              display: 'flex', alignItems: 'center', justifyContent: 'space-between',
              padding: '10px 14px', background: '#1a1a00',
              border: '1px solid #444400', borderRadius: 4,
            }}>
              <span style={{ color: '#aaaa00', fontSize: 12 }}>
                Тикер изменён на <strong>{pendingSymbol.replace('USDT', '')}/USDT</strong> — перезапустите бота чтобы применить
              </span>
              <button
                onClick={restart}
                style={{
                  background: 'none', border: '1px solid #666600', borderRadius: 4,
                  color: '#aaaa00', cursor: 'pointer', padding: '4px 12px',
                  fontSize: 12, fontFamily: 'inherit',
                }}
              >
                ↺ Перезапустить
              </button>
            </div>
          )}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, alignItems: 'start' }}>
            <PnlChart recentTrades={trades} />
            <ChartsRow prices={prices} />
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
            <PriceTable prices={prices} />
            <TradesFeed trades={trades} />
          </div>
          <StatusBar status={status} lastUpdate={lastUpdate} />
        </>
      )}
    </div>
  )
}
