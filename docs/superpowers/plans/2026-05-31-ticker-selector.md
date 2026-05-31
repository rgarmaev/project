# Ticker Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a ticker dropdown to the dashboard header that lets the user switch the trading pair (e.g. SOLUSDT → BTCUSDT), saves it to config.toml, and shows a restart banner.

**Architecture:** Backend exposes `symbol` in `WsSnapshot` and `ConfigPayload`; `update_config_toml` writes the new symbol/quote to config.toml. Frontend adds a `TickerSelector` component in the header with a static list of 50 USDT pairs and a search field. On selection, App.tsx POSTs the updated config and shows a "↺ Перезапустить" banner.

**Tech Stack:** Rust (axum, serde), React 18, TypeScript

---

## File Map

| Action | Path | Change |
|---|---|---|
| Modify | `src/dashboard/state.rs` | Add `symbol: String` to `WsSnapshot`, populate from `self.config.pair()` |
| Modify | `src/dashboard/config_api.rs` | Add `symbol` to `ConfigPayload`; `read_config_toml` returns symbol; `update_config_toml` writes symbol/quote |
| Modify | `dashboard/src/types.ts` | Add `symbol: string` to `WsSnapshot` |
| Create | `dashboard/src/components/TickerSelector.tsx` | Dropdown with search, static 50-ticker list |
| Modify | `dashboard/src/App.tsx` | Add TickerSelector to header, restart banner, `changeTicker` fn |

---

### Task 1: Add symbol to Rust WsSnapshot

**Files:**
- Modify: `src/dashboard/state.rs`

- [ ] **Step 1: Add `symbol` field to WsSnapshot struct**

Find the `WsSnapshot` struct (around line 63) and add `pub symbol: String`:

```rust
#[derive(Serialize, Clone)]
pub struct WsSnapshot {
    pub metrics: crate::metrics::MetricsSnapshot,
    pub prices: Vec<PriceEntry>,
    pub recent_trades: Vec<TradeRecord>,
    pub effective_min_spread_pct: f64,
    pub symbol: String,
}
```

- [ ] **Step 2: Populate `symbol` in `build_snapshot`**

In the `build_snapshot` method, add `symbol: self.config.pair()` to the `WsSnapshot` literal:

```rust
WsSnapshot {
    metrics: self.metrics.snapshot(),
    prices,
    recent_trades: self.recent_trades(50),
    effective_min_spread_pct,
    symbol: self.config.pair(),
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo check 2>&1 | grep -E "^error"
```

Expected: no output (no errors).

- [ ] **Step 4: Commit**

```bash
git add src/dashboard/state.rs
git commit -m "feat: add symbol to WsSnapshot"
```

---

### Task 2: Add symbol to ConfigPayload (Rust)

**Files:**
- Modify: `src/dashboard/config_api.rs`

- [ ] **Step 1: Add `symbol` to `ConfigPayload` struct**

```rust
#[derive(Serialize, Deserialize)]
pub struct ConfigPayload {
    pub paper_trading: bool,
    pub trade_size_usdt: f64,
    pub min_spread_pct: f64,
    pub symbol: String,
    pub binance: ExchangeSettings,
    pub bybit: ExchangeSettings,
    pub mexc: ExchangeSettings,
}
```

- [ ] **Step 2: Add `extract_str` helper**

Add this function after the existing `extract_f64` function:

```rust
fn extract_str(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(key) && line.contains('=') {
            if let Some(val) = line.split('=').nth(1) {
                return Some(val.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 3: Update `read_config_toml` to return symbol**

Replace the function signature and body:

```rust
fn read_config_toml() -> (bool, f64, f64, bool, bool, String) {
    let Ok(content) = std::fs::read_to_string("config.toml") else {
        return (true, 200.0, 0.00005, false, false, "SOLUSDT".to_string());
    };
    let paper = content.contains("paper_trading = true");
    let size   = extract_f64(&content, "trade_size_usdt").unwrap_or(200.0);
    let spread = extract_f64(&content, "min_spread_pct").unwrap_or(0.00005);
    let bin_testnet   = section_bool(&content, "[binance]", "testnet = true");
    let bybit_testnet = section_bool(&content, "[bybit]",   "testnet = true");
    let sym   = extract_str(&content, "symbol").unwrap_or("SOL".to_string());
    let quote = extract_str(&content, "quote").unwrap_or("USDT".to_string());
    (paper, size, spread, bin_testnet, bybit_testnet, format!("{}{}", sym, quote))
}
```

- [ ] **Step 4: Update `get_config` to destructure and return symbol**

```rust
pub async fn get_config(
    State(_state): State<Arc<DashboardState>>,
) -> Json<ConfigPayload> {
    let env_path = ".env";
    let env_vars = read_env_file(env_path);

    let binance_key    = env_vars.get("BINANCE_API_KEY").cloned().unwrap_or_default();
    let binance_secret = env_vars.get("BINANCE_API_SECRET").cloned().unwrap_or_default();
    let bybit_key      = env_vars.get("BYBIT_API_KEY").cloned().unwrap_or_default();
    let bybit_secret   = env_vars.get("BYBIT_API_SECRET").cloned().unwrap_or_default();
    let mexc_key       = env_vars.get("MEXC_API_KEY").cloned().unwrap_or_default();
    let mexc_secret    = env_vars.get("MEXC_API_SECRET").cloned().unwrap_or_default();

    let (paper, size, spread, bin_testnet, bybit_testnet, symbol) = read_config_toml();

    Json(ConfigPayload {
        paper_trading: paper,
        trade_size_usdt: size,
        min_spread_pct: spread,
        symbol,
        binance: ExchangeSettings {
            api_key:    mask(&binance_key),
            api_secret: mask(&binance_secret),
            testnet:    bin_testnet,
        },
        bybit: ExchangeSettings {
            api_key:    mask(&bybit_key),
            api_secret: mask(&bybit_secret),
            testnet:    bybit_testnet,
        },
        mexc: ExchangeSettings {
            api_key:    mask(&mexc_key),
            api_secret: mask(&mexc_secret),
            testnet:    false,
        },
    })
}
```

- [ ] **Step 5: Update `update_config_toml` to write symbol and quote**

In the `update_config_toml` function, add parsing of symbol/quote and update the line-matching loop:

```rust
fn update_config_toml(payload: &ConfigPayload) -> anyhow::Result<()> {
    let content = std::fs::read_to_string("config.toml")?;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();

    let (sym, quote) = payload.symbol.strip_suffix("USDT")
        .map(|s| (s.to_string(), "USDT".to_string()))
        .unwrap_or(("SOL".to_string(), "USDT".to_string()));

    for line in &mut lines {
        let t = line.trim();
        if t.starts_with("paper_trading") {
            *line = format!("paper_trading = {}", payload.paper_trading);
        } else if t.starts_with("trade_size_usdt") {
            *line = format!("trade_size_usdt  = \"{}\"", payload.trade_size_usdt);
        } else if t.starts_with("min_spread_pct") {
            *line = format!("min_spread_pct   = \"{}\"", payload.min_spread_pct);
        } else if t.starts_with("symbol") {
            *line = format!("symbol = \"{}\"", sym);
        } else if t.starts_with("quote") {
            *line = format!("quote  = \"{}\"", quote);
        }
    }

    let mut section = String::new();
    for line in &mut lines {
        let t = line.trim();
        if t.starts_with('[') { section = t.to_string(); }
        if t.starts_with("testnet") {
            let val = match section.as_str() {
                "[binance]" => payload.binance.testnet,
                "[bybit]"   => payload.bybit.testnet,
                _           => false,
            };
            *line = format!("testnet    = {}", val);
        }
    }

    std::fs::write("config.toml", lines.join("\n") + "\n")?;
    Ok(())
}
```

- [ ] **Step 6: Build and run tests**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo test 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/dashboard/config_api.rs
git commit -m "feat: add symbol to ConfigPayload, read/write from config.toml"
```

---

### Task 3: Add symbol to TypeScript WsSnapshot

**Files:**
- Modify: `dashboard/src/types.ts`

- [ ] **Step 1: Add `symbol` to WsSnapshot interface**

In `dashboard/src/types.ts`, add `symbol: string` to `WsSnapshot`:

```typescript
export interface WsSnapshot {
  metrics: MetricsSnapshot
  prices: PriceEntry[]
  recent_trades: TradeRecord[]
  effective_min_spread_pct: number
  symbol: string
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/types.ts
git commit -m "feat: add symbol to TS WsSnapshot"
```

---

### Task 4: Create TickerSelector component

**Files:**
- Create: `dashboard/src/components/TickerSelector.tsx`

- [ ] **Step 1: Create the file**

```typescript
import { useState, useEffect, useRef } from 'react'

export const TICKERS = [
  'BTCUSDT',  'ETHUSDT',    'BNBUSDT',   'SOLUSDT',   'XRPUSDT',
  'ADAUSDT',  'DOGEUSDT',   'AVAXUSDT',  'DOTUSDT',   'MATICUSDT',
  'LINKUSDT', 'LTCUSDT',    'UNIUSDT',   'ATOMUSDT',  'BCHUSDT',
  'ICPUSDT',  'APTUSDT',    'ARBUSDT',   'OPUSDT',    'FILUSDT',
  'NEARUSDT', 'SANDUSDT',   'MANAUSDT',  'AXSUSDT',   'ALGOUSDT',
  'VETUSDT',  'FTMUSDT',    'HBARUSDT',  'ETCUSDT',   'XLMUSDT',
  'TRXUSDT',  'SUIUSDT',    'SEIUSDT',   'INJUSDT',   'TIAUSDT',
  'JUPUSDT',  'WIFUSDT',    'BONKUSDT',  'PEPEUSDT',  'SHIBUSDT',
  'NOTUSDT',  'TONUSDT',    'STXUSDT',   'RUNEUSDT',  'RENDERUSDT',
  'WLDUSDT',  'ENAUSDT',    'ZKUSDT',    'THETAUSDT', 'FLOKIUSDT',
]

interface Props {
  current: string
  onChange: (symbol: string) => void
}

export function TickerSelector({ current, onChange }: Props) {
  const [open, setOpen] = useState(false)
  const [search, setSearch] = useState('')
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function onClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false)
        setSearch('')
      }
    }
    document.addEventListener('mousedown', onClickOutside)
    return () => document.removeEventListener('mousedown', onClickOutside)
  }, [])

  const filtered = TICKERS.filter(t =>
    t.toLowerCase().includes(search.toLowerCase())
  )
  const base = current.replace('USDT', '')

  return (
    <div ref={ref} style={{ position: 'relative' }}>
      <button
        onClick={() => { setOpen(o => !o); setSearch('') }}
        style={{
          background: 'none', border: '1px solid #2a2a2a', borderRadius: 4,
          color: '#e0e0e0', cursor: 'pointer', padding: '4px 10px',
          fontSize: 13, fontWeight: 600, fontFamily: 'inherit',
          display: 'flex', alignItems: 'center', gap: 4,
        }}
      >
        {base}<span style={{ color: '#555' }}>/USDT</span>
        <span style={{ color: '#444', fontSize: 10, marginLeft: 2 }}>▾</span>
      </button>

      {open && (
        <div style={{
          position: 'absolute', top: 'calc(100% + 6px)', left: 0, zIndex: 100,
          background: '#111', border: '1px solid #2a2a2a', borderRadius: 6,
          width: 200, boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
        }}>
          <div style={{ padding: 8 }}>
            <input
              autoFocus
              placeholder="Поиск..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              onKeyDown={e => e.key === 'Escape' && (setOpen(false), setSearch(''))}
              style={{
                width: '100%', background: '#0a0a0a', border: '1px solid #2a2a2a',
                borderRadius: 4, color: '#e0e0e0', padding: '6px 8px',
                fontSize: 12, fontFamily: 'inherit', outline: 'none',
                boxSizing: 'border-box',
              }}
            />
          </div>
          <div style={{ maxHeight: 240, overflowY: 'auto' }}>
            {filtered.map(ticker => {
              const isActive = ticker === current
              return (
                <div
                  key={ticker}
                  onClick={() => { onChange(ticker); setOpen(false); setSearch('') }}
                  style={{
                    padding: '7px 12px', cursor: 'pointer', fontSize: 12,
                    color: isActive ? '#00ff87' : '#888',
                    background: isActive ? '#001a00' : 'transparent',
                    transition: 'background 100ms',
                  }}
                  onMouseEnter={e => {
                    if (!isActive) e.currentTarget.style.background = '#1a1a1a'
                  }}
                  onMouseLeave={e => {
                    if (!isActive) e.currentTarget.style.background = 'transparent'
                  }}
                >
                  {ticker.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                </div>
              )
            })}
            {filtered.length === 0 && (
              <div style={{ padding: '12px', color: '#333', fontSize: 12, textAlign: 'center' }}>
                Не найдено
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/components/TickerSelector.tsx
git commit -m "feat: add TickerSelector component with 50 USDT pairs"
```

---

### Task 5: Wire TickerSelector into App.tsx

**Files:**
- Modify: `dashboard/src/App.tsx`

- [ ] **Step 1: Add imports**

At the top of `dashboard/src/App.tsx`, add:

```typescript
import { TickerSelector } from './components/TickerSelector'
```

- [ ] **Step 2: Add `pendingSymbol` state and `changeTicker` function**

Inside the `App()` function body, after the existing state declarations:

```typescript
const [pendingSymbol, setPendingSymbol] = useState<string | null>(null)

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
```

- [ ] **Step 3: Update the header title and add TickerSelector**

Replace the current header `<span>SOL ARB</span>` with a dynamic title and add the TickerSelector next to it:

```tsx
<div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
  <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: '0.1em', color: '#e0e0e0', marginRight: 12 }}>
    {base} ARB
  </span>
  <TickerSelector current={symbol} onChange={changeTicker} />
  <div style={{ width: 12 }} />
  <NavTab label="Дашборд" active={tab === 'dashboard'} onClick={() => setTab('dashboard')} />
  <NavTab label="Настройки" active={tab === 'settings'} onClick={() => setTab('settings')} />
</div>
```

- [ ] **Step 4: Add restart banner after MetricsBar**

In the dashboard tab JSX, insert the banner between `<MetricsBar>` and the PnlChart/ChartsRow grid:

```tsx
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
```

Note: `restart` function is already defined in `App.tsx` (added in the restart button task). If it's only in `SettingsPage.tsx`, move it to App.tsx or duplicate it:

```typescript
async function restart() {
  try { await fetch('/api/restart', { method: 'POST' }) } catch (_) {}
  setTimeout(() => window.location.reload(), 3000)
}
```

- [ ] **Step 5: Commit**

```bash
git add dashboard/src/App.tsx
git commit -m "feat: wire TickerSelector into App header with restart banner"
```

---

### Task 6: Build and push

- [ ] **Step 1: Build dashboard**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -3
```

Expected: `✓ built in Xs` with no TypeScript errors.

- [ ] **Step 2: Build Rust**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 3: Push**

```bash
git push origin master
```

- [ ] **Step 4: Restart bot and verify**

```bash
pkill -f "sol-arb"; lsof -ti:3001 | xargs kill -9 2>/dev/null; sleep 1
cd /Users/rinchin92/claude/project && RUST_LOG=sol_arb=info ./target/release/sol-arb > /tmp/sol-arb.log 2>&1 &
sleep 4 && curl -s http://localhost:3001/api/config | python3 -m json.tool | grep symbol
```

Expected: `"symbol": "SOLUSDT"` (or whatever is in config.toml).
