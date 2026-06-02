import { useState } from 'react'
import { OpportunityRow } from '../types'

interface Props {
  data: OpportunityRow[]
  spotData: OpportunityRow[]
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

function WithdrawBadge({ ok }: { ok: boolean | null }) {
  if (ok === null)  return <span title="Статус вывода неизвестен" style={{ color: '#444', fontSize: 10 }}>?</span>
  if (ok === true)  return <span title="Вывод открыт" style={{ color: '#00aa44', fontSize: 10 }}>↑</span>
  return <span title="Вывод/депозит закрыт — цена может не выровняться" style={{ color: '#ff4444', fontSize: 10 }}>✗</span>
}

export function TopOpportunities({ data, spotData }: Props) {
  const [search, setSearch] = useState('')
  const [tab, setTab] = useState<'all' | 'spot'>('all')
  const [hideBlocked, setHideBlocked] = useState(false)

  const source = tab === 'spot' ? spotData : data

  const filtered = source.filter(r => {
    if (search.trim() && !r.symbol.toLowerCase().includes(search.trim().toLowerCase())) return false
    if (hideBlocked && r.withdraw_ok === false) return false
    return true
  })

  const tabStyle = (active: boolean): React.CSSProperties => ({
    background: 'none', border: 'none', cursor: 'pointer',
    padding: '3px 10px', fontSize: 11, fontFamily: 'inherit',
    color: active ? '#e0e0e0' : '#444',
    borderBottom: active ? '2px solid #00ff87' : '2px solid transparent',
  })

  return (
    <div style={{ background: '#0d0d0d', border: '1px solid #1a1a1a', borderRadius: 6, padding: '12px 16px' }}>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
          <span style={{ fontSize: 11, color: '#555', textTransform: 'uppercase', letterSpacing: '0.08em', marginRight: 10 }}>
            Opportunities ({filtered.length})
          </span>
          <button style={tabStyle(tab === 'all')}  onClick={() => setTab('all')}>All</button>
          <button style={tabStyle(tab === 'spot')} onClick={() => setTab('spot')}>Spot↔Spot</button>
        </div>
        <span style={{ fontSize: 10, color: '#333' }}>live ●</span>
      </div>

      {/* Controls */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
        <input
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Поиск..."
          style={{
            flex: 1, background: '#0a0a0a', border: '1px solid #222',
            borderRadius: 4, color: '#e0e0e0', padding: '5px 8px',
            fontSize: 11, fontFamily: 'inherit', outline: 'none',
          }}
        />
        <label style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 11, color: '#555', cursor: 'pointer', whiteSpace: 'nowrap' }}>
          <input
            type="checkbox"
            checked={hideBlocked}
            onChange={e => setHideBlocked(e.target.checked)}
            style={{ accentColor: '#00ff87' }}
          />
          Скрыть закрытые
        </label>
      </div>

      {filtered.length === 0 ? (
        <div style={{ color: '#333', fontSize: 12, padding: '20px 0', textAlign: 'center' }}>
          {source.length === 0 ? 'Waiting for data…' : 'Нет совпадений'}
        </div>
      ) : (
        <div style={{ maxHeight: 480, overflowY: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
            <thead style={{ position: 'sticky', top: 0, background: '#0d0d0d' }}>
              <tr style={{ color: '#444' }}>
                <th style={{ textAlign: 'left',  padding: '4px 6px 4px 0', fontWeight: 400, width: 16 }}></th>
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
                const rowBg = row.withdraw_ok === false ? 'rgba(255,68,68,0.04)' : 'transparent'
                return (
                  <tr key={i} style={{ borderTop: '1px solid #111', background: rowBg }}>
                    <td style={{ padding: '5px 4px 5px 0', textAlign: 'center' }}>
                      <WithdrawBadge ok={row.withdraw_ok} />
                    </td>
                    <td style={{ padding: '5px 8px 5px 0', color: '#e0e0e0', fontWeight: 600 }}>
                      {row.symbol.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                    </td>
                    <td style={{ padding: '5px 8px', color: '#888', fontSize: 11 }}>{row.buy_market}</td>
                    <td style={{ padding: '5px 8px', color: '#888', fontSize: 11 }}>{row.sell_market}</td>
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

      <div style={{ marginTop: 8, display: 'flex', gap: 12, fontSize: 10, color: '#333' }}>
        <span><span style={{ color: '#00aa44' }}>↑</span> вывод открыт</span>
        <span><span style={{ color: '#ff4444' }}>✗</span> вывод закрыт</span>
        <span><span style={{ color: '#444' }}>?</span> статус неизвестен</span>
      </div>
    </div>
  )
}
