# 200 Pairs + Dashboard Search — Phase 1

**Date:** 2026-06-02
**Status:** Approved

## Goal

Expand the monitored pair list from 50 to ~200 USDT pairs and update the Top Opportunities dashboard table to show all pairs with a live search/filter input.

## Scope

Phase 1 only. Phase 2 (OKX + BingX exchange connectors) is a separate spec.

## Changes

### 1. `src/tickers.rs` — expand TICKERS to 200 pairs

Replace the 50-element const with a ~200-element list of top USDT pairs by trading volume available on both Binance and Bybit.

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
    "HIGHUSDT", "MAGICUSDT", "HOOKUSDT", "MTLUSDT", "ILVUSDT",
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
    "XVSUSDT", "HARDUSDT", "COSUSDT", "MDTUSDT", "AMBUSDT",
    "ERNUSDT", "RADUSDT", "FORTHUSDT", "ALPACAUSDT", "OXTUSDT",
];
```

### 2. `src/dashboard/state.rs` — remove top-10 truncation

In `build_snapshot()`, change:
```rust
opps.truncate(10);
```
to:
```rust
opps.truncate(250);  // cap at 250 to avoid oversized payloads
```

### 3. `dashboard/src/components/TopOpportunities.tsx` — add search + scroll

Add a search state and filter input above the table. Wrap the table in a `max-height: 480px` scroll container.

**New file content:**

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

## Files Changed

| Action | File |
|--------|------|
| Modify | `src/tickers.rs` — expand to ~200 pairs (remove duplicates) |
| Modify | `src/dashboard/state.rs` — `truncate(10)` → `truncate(250)` |
| Modify | `dashboard/src/components/TopOpportunities.tsx` — add search + scroll |

## Scope

- No changes to multi_feed WebSocket feeds (Binance/Bybit still the only sources)
- No changes to MultiPairDetector, MultiPairTick, executor, or other modules
- Frontend filter is client-side only (no new API endpoints)
- Phase 2 (OKX + BingX) is a separate spec
