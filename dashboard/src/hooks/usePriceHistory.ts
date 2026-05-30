import { useEffect, useRef, useState } from 'react'
import { PriceEntry } from '../types'

export interface PricePoint {
  time: string
  bid: number
  ask: number
}

export function usePriceHistory(
  prices: PriceEntry[],
  maxPoints = 300,
): Map<string, PricePoint[]> {
  const bufRef = useRef<Map<string, PricePoint[]>>(new Map())
  const [snap, setSnap] = useState<Map<string, PricePoint[]>>(new Map())

  useEffect(() => {
    if (prices.length === 0) return
    const now = new Date().toLocaleTimeString()
    const buf = bufRef.current

    for (const p of prices) {
      const key = `${p.exchange}:${p.market}`
      const prev = buf.get(key) ?? []
      const next = prev.length >= maxPoints
        ? [...prev.slice(1), { time: now, bid: p.bid, ask: p.ask }]
        : [...prev, { time: now, bid: p.bid, ask: p.ask }]
      buf.set(key, next)
    }

    setSnap(new Map(buf))
  }, [prices, maxPoints])

  return snap
}
