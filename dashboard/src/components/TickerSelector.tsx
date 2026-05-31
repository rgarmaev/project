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
