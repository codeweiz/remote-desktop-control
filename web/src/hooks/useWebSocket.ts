import { useEffect, useRef, useState, useCallback } from 'react'
import type { ConnectionState, WsMessage } from '../lib/types'
import { getWsUrl } from '../lib/websocket'

interface UseWebSocketOptions {
  /** WebSocket path, e.g. '/ws/status' */
  path: string
  /** Whether to connect (allows conditional connection) */
  enabled?: boolean
  /** Called on each received message */
  onMessage?: (msg: WsMessage) => void
  /** Called when connection opens */
  onOpen?: () => void
  /** Called when connection closes */
  onClose?: () => void
}

interface UseWebSocketReturn {
  connectionState: ConnectionState
  latency: number | null
  send: (data: string | WsMessage) => void
  disconnect: () => void
}

const MAX_RECONNECT_DELAY = 30000
const INITIAL_RECONNECT_DELAY = 1000
const PING_INTERVAL = 15000

export function useWebSocket({
  path,
  enabled = true,
  onMessage,
  onOpen,
  onClose,
}: UseWebSocketOptions): UseWebSocketReturn {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected')
  const [latency, setLatency] = useState<number | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectDelayRef = useRef(INITIAL_RECONNECT_DELAY)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pingStartRef = useRef<number>(0)
  const mountedRef = useRef(true)
  const manualDisconnectRef = useRef(false)

  // Keep latest callbacks in refs to avoid re-connections on handler change
  const onMessageRef = useRef(onMessage)
  const onOpenRef = useRef(onOpen)
  const onCloseRef = useRef(onClose)
  onMessageRef.current = onMessage
  onOpenRef.current = onOpen
  onCloseRef.current = onClose

  const clearTimers = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current)
      reconnectTimerRef.current = null
    }
    if (pingTimerRef.current) {
      clearInterval(pingTimerRef.current)
      pingTimerRef.current = null
    }
  }, [])

  const send = useCallback((data: string | WsMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      const payload = typeof data === 'string' ? data : JSON.stringify(data)
      wsRef.current.send(payload)
    }
  }, [])

  const disconnect = useCallback(() => {
    manualDisconnectRef.current = true
    clearTimers()
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }
    setConnectionState('disconnected')
  }, [clearTimers])

  const connect = useCallback(() => {
    if (!mountedRef.current || !enabled) return
    manualDisconnectRef.current = false

    const url = getWsUrl(path)
    setConnectionState('connecting')

    const ws = new WebSocket(url)
    wsRef.current = ws

    ws.onopen = () => {
      if (!mountedRef.current) return
      setConnectionState('connected')
      reconnectDelayRef.current = INITIAL_RECONNECT_DELAY

      // Start ping interval for latency measurement
      pingTimerRef.current = setInterval(() => {
        if (ws.readyState === WebSocket.OPEN) {
          pingStartRef.current = performance.now()
          ws.send(JSON.stringify({ type: 'ping' }))
        }
      }, PING_INTERVAL)

      onOpenRef.current?.()
    }

    ws.onmessage = (event) => {
      if (!mountedRef.current) return
      try {
        const msg = JSON.parse(event.data) as WsMessage
        // Handle pong for latency
        if (msg.type === 'pong' && pingStartRef.current > 0) {
          setLatency(Math.round(performance.now() - pingStartRef.current))
          pingStartRef.current = 0
          return
        }
        onMessageRef.current?.(msg)
      } catch {
        // Non-JSON message, ignore
      }
    }

    ws.onerror = () => {
      if (!mountedRef.current) return
      setConnectionState('error')
    }

    ws.onclose = () => {
      if (!mountedRef.current) return
      clearTimers()
      setConnectionState('disconnected')
      onCloseRef.current?.()

      // Auto-reconnect with exponential backoff
      if (!manualDisconnectRef.current && enabled) {
        const delay = reconnectDelayRef.current
        reconnectDelayRef.current = Math.min(delay * 2, MAX_RECONNECT_DELAY)
        reconnectTimerRef.current = setTimeout(() => {
          if (mountedRef.current && !manualDisconnectRef.current) {
            connect()
          }
        }, delay)
      }
    }
  }, [path, enabled, clearTimers])

  useEffect(() => {
    mountedRef.current = true
    if (enabled) {
      connect()
    }
    return () => {
      mountedRef.current = false
      manualDisconnectRef.current = true
      clearTimers()
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, [connect, enabled, clearTimers])

  return { connectionState, latency, send, disconnect }
}
