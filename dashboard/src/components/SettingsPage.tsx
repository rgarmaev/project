import { useEffect, useState } from 'react'

interface ExchangeSettings {
  api_key: string
  api_secret: string
  testnet: boolean
}

interface Config {
  paper_trading: boolean
  trade_size_usdt: number
  min_spread_pct: number
  binance: ExchangeSettings
  bybit: ExchangeSettings
  mexc: ExchangeSettings
}

const EMPTY_CONFIG: Config = {
  paper_trading: true,
  trade_size_usdt: 200,
  min_spread_pct: 0.00005,
  binance: { api_key: '', api_secret: '', testnet: false },
  bybit:   { api_key: '', api_secret: '', testnet: false },
  mexc:    { api_key: '', api_secret: '', testnet: false },
}

const EXCHANGE_LABELS: Record<string, { name: string; color: string; docsUrl: string }> = {
  binance: { name: 'Binance', color: '#f0b90b', docsUrl: 'https://www.binance.com/en/my/settings/api-management' },
  bybit:   { name: 'Bybit',   color: '#f7a600', docsUrl: 'https://www.bybit.com/app/user/api-management' },
  mexc:    { name: 'MEXC',    color: '#0768f4', docsUrl: 'https://www.mexc.com/user/openapi' },
}

const READONLY_EXCHANGES = [
  { id: 'okx',   name: 'OKX',   color: '#00d4ff', status: 'Мониторинг (spot + swap)', note: 'Исполнение ордеров не реализовано' },
  { id: 'bingx', name: 'BingX', color: '#00d4aa', status: 'Мониторинг (spot + swap)', note: 'Исполнение ордеров не реализовано' },
]

function ExchangeBlock({
  id, cfg, onChange,
}: {
  id: 'binance' | 'bybit' | 'mexc'
  cfg: ExchangeSettings
  onChange: (v: ExchangeSettings) => void
}) {
  const [showKey, setShowKey] = useState(false)
  const [showSecret, setShowSecret] = useState(false)
  const meta = EXCHANGE_LABELS[id]

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 20, marginBottom: 12 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <span style={{ color: meta.color, fontWeight: 600, fontSize: 13 }}>{meta.name}</span>
        <a href={meta.docsUrl} target="_blank" rel="noreferrer"
          style={{ color: '#444', fontSize: 11, textDecoration: 'none' }}>
          Get API keys ↗
        </a>
      </div>

      <div style={{ display: 'grid', gap: 10 }}>
        <Field label="API Key" value={cfg.api_key} show={showKey}
          onToggle={() => setShowKey(v => !v)}
          onChange={v => onChange({ ...cfg, api_key: v })} />
        <Field label="API Secret" value={cfg.api_secret} show={showSecret}
          onToggle={() => setShowSecret(v => !v)}
          onChange={v => onChange({ ...cfg, api_secret: v })} />

        {id !== 'mexc' && (
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontSize: 12, color: '#666' }}>
            <input type="checkbox" checked={cfg.testnet} onChange={e => onChange({ ...cfg, testnet: e.target.checked })}
              style={{ accentColor: '#00ff87' }} />
            Use testnet
          </label>
        )}
      </div>
    </div>
  )
}

function Field({ label, value, show, onToggle, onChange }: {
  label: string; value: string; show: boolean
  onToggle: () => void; onChange: (v: string) => void
}) {
  return (
    <div>
      <div style={{ color: '#555', fontSize: 11, marginBottom: 4, textTransform: 'uppercase' }}>{label}</div>
      <div style={{ display: 'flex', gap: 6 }}>
        <input
          type={show ? 'text' : 'password'}
          value={value}
          onChange={e => onChange(e.target.value)}
          placeholder="Не заполнено"
          style={{
            flex: 1, background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
            color: '#e0e0e0', padding: '8px 10px', fontSize: 12, fontFamily: 'inherit',
            outline: 'none',
          }}
        />
        <button onClick={onToggle} style={{
          background: '#1a1a1a', border: '1px solid #2a2a2a', borderRadius: 4,
          color: '#555', cursor: 'pointer', padding: '0 10px', fontSize: 11,
        }}>
          {show ? 'hide' : 'show'}
        </button>
      </div>
    </div>
  )
}

function NumberField({ label, value, onChange, step, note }: {
  label: string; value: number; onChange: (v: number) => void; step: number; note?: string
}) {
  return (
    <div>
      <div style={{ color: '#555', fontSize: 11, marginBottom: 4, textTransform: 'uppercase' }}>{label}</div>
      <input
        type="number" value={value} step={step}
        onChange={e => onChange(parseFloat(e.target.value) || 0)}
        style={{
          width: '100%', background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
          color: '#e0e0e0', padding: '8px 10px', fontSize: 12, fontFamily: 'inherit', outline: 'none',
        }}
      />
      {note && <div style={{ color: '#444', fontSize: 10, marginTop: 3 }}>{note}</div>}
    </div>
  )
}

export function SettingsPage() {
  const [config, setConfig] = useState<Config>(EMPTY_CONFIG)
  const [status, setStatus] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch('/api/config')
      .then(r => r.json())
      .then(data => { setConfig(data); setLoading(false) })
      .catch(() => setLoading(false))
  }, [])

  async function save() {
    setStatus('Сохраняю...')
    try {
      const r = await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      })
      const data = await r.json()
      setStatus(data.message ?? (data.error ? `Ошибка: ${data.error}` : 'OK'))
    } catch (e) {
      setStatus('Ошибка сети')
    }
  }

  async function restart() {
    setStatus('Перезапуск...')
    try {
      await fetch('/api/restart', { method: 'POST' })
    } catch (_) {}
    setTimeout(() => {
      window.location.reload()
    }, 3000)
  }

  if (loading) return (
    <div style={{ padding: 40, color: '#444', textAlign: 'center' }}>Загрузка...</div>
  )

  return (
    <div style={{ maxWidth: 640, margin: '0 auto', padding: '20px 20px' }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.1em', marginBottom: 20 }}>
        Настройки
      </div>

      {/* Trading params */}
      <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 20, marginBottom: 12 }}>
        <div style={{ color: '#e0e0e0', fontWeight: 600, fontSize: 13, marginBottom: 16 }}>Параметры торговли</div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
          <NumberField label="Размер сделки (USDT)" value={config.trade_size_usdt}
            onChange={v => setConfig(c => ({ ...c, trade_size_usdt: v }))} step={10}
            note="Сумма на каждую ногу арбитража" />
          <NumberField label="Мин. спред (%)" value={+(config.min_spread_pct * 100).toFixed(6)}
            onChange={v => setConfig(c => ({ ...c, min_spread_pct: v / 100 }))} step={0.001}
            note="После maker-fees и slippage" />
        </div>
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontSize: 12, color: '#666', marginTop: 14 }}>
          <input type="checkbox" checked={config.paper_trading}
            onChange={e => setConfig(c => ({ ...c, paper_trading: e.target.checked }))}
            style={{ accentColor: '#00ff87' }} />
          Бумажная торговля (paper trading) — ордера не исполняются реально
        </label>
      </div>

      {/* Exchange blocks */}
      {(['binance', 'bybit', 'mexc'] as const).map(id => (
        <ExchangeBlock key={id} id={id} cfg={config[id]}
          onChange={v => setConfig(c => ({ ...c, [id]: v }))} />
      ))}

      {/* Read-only exchange blocks (monitoring only) */}
      {READONLY_EXCHANGES.map(ex => (
        <div key={ex.id} style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 20, marginBottom: 12 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ color: ex.color, fontWeight: 600, fontSize: 13 }}>{ex.name}</span>
            <span style={{ background: '#0d1a0d', border: '1px solid #1a3a1a', borderRadius: 3, padding: '2px 8px', color: '#00aa44', fontSize: 10 }}>
              PAPER ONLY
            </span>
          </div>
          <div style={{ marginTop: 12, color: '#555', fontSize: 12 }}>{ex.status}</div>
          <div style={{ marginTop: 4, color: '#333', fontSize: 11 }}>{ex.note}</div>
        </div>
      ))}

      {/* Save + Restart buttons */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr auto', gap: 8, marginTop: 4 }}>
        <button onClick={save} style={{
          padding: '12px 0', background: '#00ff87', border: 'none',
          borderRadius: 6, color: '#000', fontWeight: 600, fontSize: 13,
          fontFamily: 'inherit', cursor: 'pointer',
        }}>
          Сохранить
        </button>
        <button onClick={restart} style={{
          padding: '12px 20px', background: 'none', border: '1px solid #333',
          borderRadius: 6, color: '#888', fontWeight: 600, fontSize: 13,
          fontFamily: 'inherit', cursor: 'pointer',
        }}>
          ↺ Перезапустить
        </button>
      </div>

      {status && (
        <div style={{
          marginTop: 12, padding: '10px 14px', borderRadius: 4,
          background: status.includes('Ошибка') ? '#1a0000' : '#001a00',
          border: `1px solid ${status.includes('Ошибка') ? '#440000' : '#004400'}`,
          color: status.includes('Ошибка') ? '#ff4444' : '#00ff87',
          fontSize: 12,
        }}>
          {status}
        </div>
      )}

      <div style={{ marginTop: 16, padding: '10px 14px', background: '#0d0d00', border: '1px solid #2a2a00', borderRadius: 4 }}>
        <div style={{ color: '#888', fontSize: 11 }}>
          API ключи сохраняются в файл <code style={{ color: '#aaaa00' }}>.env</code> в папке проекта.
          Параметры торговли — в <code style={{ color: '#aaaa00' }}>config.toml</code>.
          После сохранения ключей перезапустите бота.
        </div>
      </div>
    </div>
  )
}
