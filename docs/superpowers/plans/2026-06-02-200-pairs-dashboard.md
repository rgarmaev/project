# 200 Pairs + Dashboard Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand monitored pairs from 50 to ~200 and update the Top Opportunities dashboard table with search/filter and scroll.

**Architecture:** Three isolated changes: expand the `TICKERS` const in `src/tickers.rs`, raise the backend truncation cap in `build_snapshot()`, and add a search input + scroll wrapper to `TopOpportunities.tsx`. No new modules, no new API endpoints.

**Tech Stack:** Rust (tickers const), React/TypeScript (dashboard component)

---

## File Map

| Action | File |
|--------|------|
| Modify | `src/tickers.rs` — replace 50-element const with ~200-element list |
| Modify | `src/dashboard/state.rs` — `truncate(10)` → `truncate(250)` |
| Modify | `dashboard/src/components/TopOpportunities.tsx` — add search state + scroll container |

---

## Task 1: Expand TICKERS to ~200 pairs

**Files:**
- Modify: `src/tickers.rs`

- [ ] **Step 1: Replace the entire content of `src/tickers.rs`**

```rust
pub const TICKERS: &[&str] = &[
    // ── Original 50 ────────────────────────────────────────────────────────────
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT",
    "ADAUSDT", "DOGEUSDT", "AVAXUSDT", "DOTUSDT", "MATICUSDT",
    "LINKUSDT", "LTCUSDT", "UNIUSDT", "ATOMUSDT", "BCHUSDT",
    "ICPUSDT", "APTUSDT", "ARBUSDT", "OPUSDT", "FILUSDT",
    "NEARUSDT", "SANDUSDT", "MANAUSDT", "AXSUSDT", "ALGOUSDT",
    "VETUSDT", "FTMUSDT", "HBARUSDT", "ETCUSDT", "XLMUSDT",
    "TRXUSDT", "SUIUSDT", "SEIUSDT", "INJUSDT", "TIAUSDT",
    "JUPUSDT", "WIFUSDT", "BONKUSDT", "PEPEUSDT", "SHIBUSDT",
    "NOTUSDT", "TONUSDT", "STXUSDT", "RUNEUSDT", "RENDERUSDT",
    "WLDUSDT", "ENAUSDT", "ZKUSDT", "THETAUSDT", "FLOKIUSDT",
    // ── DeFi ──────────────────────────────────────────────────────────────────
    "AAVEUSDT", "CRVUSDT", "COMPUSDT", "MKRUSDT", "SNXUSDT",
    "YFIUSDT", "LDOUSDT", "GRTUSDT", "DYDXUSDT", "GMXUSDT",
    "PENDLEUSDT", "1INCHUSDT", "SUSHIUSDT", "BALUSDT", "CAKEUSDT",
    "CVXUSDT", "ANKRUSDT", "RPLUSDT", "BNTUSDT", "XVSUSDT",
    // ── Layer 1 / Smart contract ──────────────────────────────────────────────
    "KASUSDT", "EGLDUSDT", "FLOWUSDT", "MINAUSDT", "QNTUSDT",
    "ARUSDT", "CFXUSDT", "OMUSDT", "WAVESUSDT", "CELOUSDT",
    "ZECUSDT", "XTZUSDT", "EOSUSDT", "NEOUSDT", "DASHUSDT",
    "ZILUSDT", "ONTUSDT", "ICXUSDT", "KSMUSDT", "CHZUSDT",
    // ── Layer 2 / Infrastructure ──────────────────────────────────────────────
    "STRKUSDT", "MANTAUSDT", "METISUSDT", "ZRXUSDT", "SKLUSDT",
    "CELRUSDT", "POLYXUSDT", "ACAUSDT", "PERPUSDT", "SFPUSDT",
    // ── Gaming / NFT / Metaverse ──────────────────────────────────────────────
    "GALAUSDT", "APEUSDT", "IMXUSDT", "RONUSDT", "ILVSUSDT",
    "HIGHUSDT", "MAGICUSDT", "HOOKUSDT", "MTLUSDT", "RAREUSDT",
    // ── AI / Data ─────────────────────────────────────────────────────────────
    "FETUSDT", "OCEANUSDT", "NMRUSDT", "AGIXUSDT", "ACTUSDT",
    "VIRTUALUSDT", "TAIUSDT", "DEEPUSDT", "AIUSDT", "GPSUSDT",
    // ── New 2024–2025 listings ────────────────────────────────────────────────
    "ONDOUSDT", "PYTHUSDT", "WUSDT", "ZROUSDT", "LISTAUSDT",
    "BOMEUSDT", "MEMEUSDT", "TURBOUSDT", "CATIUSDT", "EIGENUSDT",
    "REZUSDT", "BBUSDT", "IOUSDT", "HMSTRUSDT", "MOVRUSDT",
    "HYPEUSDT", "PENGUUSDT", "TRUMPUSDT", "MELANIAUSDT", "DRIFTUSDT",
    // ── Misc top-volume ───────────────────────────────────────────────────────
    "HOTUSDT", "IOSTUSDT", "CTSIUSDT", "COTIUSDT", "BANDUSDT",
    "STORJUSDT", "SUPERUSDT", "AUDIOUSDT", "TRUUSDT", "ALPHAUSDT",
    "PONDUSDT", "MBOXUSDT", "TLMUSDT", "POLYUSDT", "ORNUSDT",
    "HARDUSDT", "COSUSDT", "MDTUSDT", "AMBUSDT", "ERNUSDT",
    "RADUSDT", "FORTHUSDT", "ALPACAUSDT", "OXTUSDT", "BADGERUSDT",
];
```

- [ ] **Step 2: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -10
```

Expected: no errors. If any symbol causes a type error (unlikely for a string slice), remove it.

- [ ] **Step 3: Commit**

```bash
git add src/tickers.rs
git commit -m "feat: expand TICKERS from 50 to ~200 USDT pairs"
```

---

## Task 2: Raise backend truncation cap

**Files:**
- Modify: `src/dashboard/state.rs`

- [ ] **Step 1: Find and replace the truncation line in `build_snapshot()`**

Find:
```rust
opps.truncate(10);
```

Replace with:
```rust
opps.truncate(250);
```

This is a single-line change in `build_snapshot()` near the bottom of the `multi_feed` iteration block.

- [ ] **Step 2: Verify compilation and tests**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -10
~/.cargo/bin/cargo test 2>&1 | tail -5
```

Expected: no errors, 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/state.rs
git commit -m "feat: raise top opportunities cap from 10 to 250"
```

---

## Task 3: Add search + scroll to TopOpportunities

**Files:**
- Modify: `dashboard/src/components/TopOpportunities.tsx`

- [ ] **Step 1: Replace the entire file with the updated component**

```tsx
import { useState } from 'react'
import { OpportunityRow } from '../types'

interface Props {
  data: OpportunityRow[]
}

function fmt(n: number) {
  const sign = n >= 0 ? '+' : ''
  return `${sign}${n.toFixed(3)}%`
}

function fmtPrice(n: number) {
  if (n >= 1000) return n.toFixed(2)
  if (n >= 1)    return n.toFixed(4)
  return n.toFixed(6)
}

export function TopOpportunities({ data }: Props) {
  const [search, setSearch] = useState('')

  const filtered = search.trim()
    ? data.filter(r => r.symbol.toLowerCase().includes(search.trim().toLowerCase()))
    : data

  return (
    <div style={{
      background: '#0d0d0d', border: '1px solid #1a1a1a',
      borderRadius: 6, padding: '12px 16px',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
        <span style={{ fontSize: 11, color: '#555', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
          Opportunities ({filtered.length})
        </span>
        <span style={{ fontSize: 10, color: '#333' }}>live ●</span>
      </div>

      <input
        value={search}
        onChange={e => setSearch(e.target.value)}
        placeholder="Поиск..."
        style={{
          width: '100%', boxSizing: 'border-box',
          background: '#0a0a0a', border: '1px solid #222',
          borderRadius: 4, color: '#e0e0e0',
          padding: '5px 8px', fontSize: 11,
          fontFamily: 'inherit', outline: 'none',
          marginBottom: 8,
        }}
      />

      {filtered.length === 0 ? (
        <div style={{ color: '#333', fontSize: 12, padding: '20px 0', textAlign: 'center' }}>
          {data.length === 0 ? 'Waiting for data…' : 'Нет совпадений'}
        </div>
      ) : (
        <div style={{ maxHeight: 480, overflowY: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
            <thead style={{ position: 'sticky', top: 0, background: '#0d0d0d' }}>
              <tr style={{ color: '#444' }}>
                <th style={{ textAlign: 'left',  padding: '4px 8px 4px 0', fontWeight: 400 }}>Symbol</th>
                <th style={{ textAlign: 'left',  padding: '4px 8px', fontWeight: 400 }}>Buy</th>
                <th style={{ textAlign: 'left',  padding: '4px 8px', fontWeight: 400 }}>Sell</th>
                <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Ask</th>
                <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Bid</th>
                <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Spread</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((row, i) => {
                const positive = row.spread_pct > 0
                const spreadColor = positive ? '#00ff87' : '#555'
                return (
                  <tr key={i} style={{ borderTop: '1px solid #111' }}>
                    <td style={{ padding: '5px 8px 5px 0', color: '#e0e0e0', fontWeight: 600 }}>
                      {row.symbol.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                    </td>
                    <td style={{ padding: '5px 8px', color: '#888' }}>{row.buy_market}</td>
                    <td style={{ padding: '5px 8px', color: '#888' }}>{row.sell_market}</td>
                    <td style={{ padding: '5px 0 5px 8px', color: '#666', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>
                      {fmtPrice(row.ask)}
                    </td>
                    <td style={{ padding: '5px 0 5px 8px', color: '#666', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>
                      {fmtPrice(row.bid)}
                    </td>
                    <td style={{ padding: '5px 0 5px 8px', color: spreadColor, textAlign: 'right', fontWeight: positive ? 600 : 400, fontVariantNumeric: 'tabular-nums' }}>
                      {fmt(row.spread_pct)}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -8
```

Expected: successful build, no TypeScript errors.

- [ ] **Step 3: Commit**

```bash
git add dashboard/src/components/TopOpportunities.tsx
git commit -m "feat: add search filter and scroll to TopOpportunities table"
```

---

## Verification

Restart the bot and open the dashboard:

```bash
pkill -f sol-arb 2>/dev/null
RUST_LOG=sol_arb=info ~/.cargo/bin/cargo run &
sleep 10
```

Open `http://localhost:3001` and confirm:
- Top Opportunities shows more rows (up to 200 pairs when feeds are populated)
- Search input filters rows as you type (e.g. type "BTC" → shows only BTCUSDT)
- Table scrolls within its 480px container
- Header row stays sticky while scrolling
- Spread column: green `+` values, grey `-` values
