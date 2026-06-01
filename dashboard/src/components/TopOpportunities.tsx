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
  return (
    <div style={{
      background: '#0d0d0d', border: '1px solid #1a1a1a',
      borderRadius: 6, padding: '12px 16px',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
        <span style={{ fontSize: 11, color: '#555', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
          Top Opportunities
        </span>
        <span style={{ fontSize: 10, color: '#333' }}>live ●</span>
      </div>

      {data.length === 0 ? (
        <div style={{ color: '#333', fontSize: 12, padding: '20px 0', textAlign: 'center' }}>
          Waiting for data…
        </div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead>
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
            {data.map((row, i) => {
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
      )}
    </div>
  )
}
