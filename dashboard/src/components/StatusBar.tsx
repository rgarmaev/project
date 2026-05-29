type Status = 'connecting' | 'connected' | 'disconnected'

interface Props {
  status: Status
  lastUpdate: Date | null
}

const statusColor: Record<Status, string> = {
  connecting: '#aaaa00',
  connected: '#00ff87',
  disconnected: '#ff4444',
}

const statusLabel: Record<Status, string> = {
  connecting: '● CONNECTING',
  connected: '● LIVE',
  disconnected: '● DISCONNECTED',
}

export function StatusBar({ status, lastUpdate }: Props) {
  return (
    <div style={{
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      padding: '6px 0',
      borderTop: '1px solid #1a1a1a',
      marginTop: 8,
    }}>
      <span style={{ color: statusColor[status], fontSize: 11 }}>
        {statusLabel[status]}
      </span>
      <span style={{ color: '#333', fontSize: 11 }}>
        {lastUpdate ? `Updated ${lastUpdate.toLocaleTimeString()}` : 'Waiting...'}
      </span>
    </div>
  )
}
