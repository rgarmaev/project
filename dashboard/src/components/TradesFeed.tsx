import { TradeRecord } from '../types'

interface Props {
  trades: TradeRecord[]
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleTimeString()
}

export function TradesFeed({ trades }: Props) {
  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Recent Trades
      </div>
      <div style={{ overflowY: 'auto', maxHeight: 480 }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead style={{ position: 'sticky', top: 0, background: '#111' }}>
            <tr>
              {['Time', 'Route', 'Spread', 'PnL', 'Exec'].map(h => (
                <th key={h} style={{ padding: '4px 6px', color: '#444', fontSize: 10, textAlign: 'left', textTransform: 'uppercase' }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {trades.length === 0 ? (
              <tr>
                <td colSpan={5} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>
                  No trades yet
                </td>
              </tr>
            ) : (
              trades.map(t => (
                <tr key={t.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                  <td style={{ padding: '5px 6px', color: '#555', fontSize: 11, whiteSpace: 'nowrap' }}>
                    {formatTime(t.time)}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#888', fontSize: 11 }}>
                    {t.buy_market} → {t.sell_market}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#555', fontSize: 11 }}>
                    {t.spread_pct.toFixed(3)}%
                  </td>
                  <td style={{ padding: '5px 6px', fontSize: 11, fontWeight: 600, color: t.net_pnl >= 0 ? '#00ff87' : '#ff4444' }}>
                    {t.net_pnl >= 0 ? '+' : ''}{t.net_pnl.toFixed(4)}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#444', fontSize: 11 }}>
                    {t.exec_ms}ms
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
