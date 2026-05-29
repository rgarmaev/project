import { useEffect, useRef, useState } from 'react'
import { WsSnapshot } from '../types'

type Status = 'connecting' | 'connected' | 'disconnected'

export function useWebSocket(url: string) {
  const [snapshot, setSnapshot] = useState<WsSnapshot | null>(null)
  const [status, setStatus] = useState<Status>('connecting')
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    function connect() {
      const ws = new WebSocket(url)
      wsRef.current = ws

      ws.onopen = () => setStatus('connected')

      ws.onmessage = (e) => {
        try {
          setSnapshot(JSON.parse(e.data) as WsSnapshot)
          setLastUpdate(new Date())
        } catch {
          // ignore malformed messages
        }
      }

      ws.onclose = () => {
        setStatus('disconnected')
        reconnectRef.current = setTimeout(connect, 2000)
      }

      ws.onerror = () => ws.close()
    }

    connect()

    return () => {
      wsRef.current?.close()
      if (reconnectRef.current) clearTimeout(reconnectRef.current)
    }
  }, [url])

  return { snapshot, status, lastUpdate }
}
