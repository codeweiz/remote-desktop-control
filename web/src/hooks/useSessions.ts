import { useState, useEffect, useCallback } from 'react'
import type { Session, SessionCreateRequest, WsMessage, SessionEvent } from '../lib/types'
import { getSessions, createSession, deleteSession } from '../lib/api'
import { useWebSocket } from './useWebSocket'

export interface SessionTree {
  terminals: Session[]
  agents: Session[]
}

export function useSessions() {
  const [sessions, setSessions] = useState<Session[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // Fetch initial sessions
  const fetchSessions = useCallback(async () => {
    try {
      setLoading(true)
      const data = await getSessions()
      setSessions(data)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch sessions')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchSessions()
  }, [fetchSessions])

  // Subscribe to real-time session events via status WebSocket
  const handleStatusMessage = useCallback((msg: WsMessage) => {
    const event = msg as unknown as SessionEvent
    switch (event.type) {
      case 'session_created':
        setSessions(prev => {
          if (prev.some(s => s.id === event.session.id)) return prev
          return [...prev, event.session]
        })
        break
      case 'session_deleted':
        setSessions(prev => prev.filter(s => s.id !== event.session.id))
        break
      case 'session_updated':
        setSessions(prev =>
          prev.map(s => s.id === event.session.id ? event.session : s)
        )
        break
    }
  }, [])

  const { connectionState: statusConnection } = useWebSocket({
    path: '/ws/status',
    onMessage: handleStatusMessage,
  })

  // Build a tree: separate terminals and agents, handle parent_id
  const tree: SessionTree = {
    terminals: sessions.filter(s => s.kind === 'terminal'),
    agents: sessions.filter(s => s.kind === 'agent'),
  }

  // Actions
  const addSession = useCallback(async (req: SessionCreateRequest = {}) => {
    try {
      const session = await createSession(req)
      // Optimistic update: add immediately (WS event will deduplicate)
      setSessions(prev => {
        if (prev.some(s => s.id === session.id)) return prev
        return [...prev, session]
      })
      return session
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create session')
      throw err
    }
  }, [])

  const removeSession = useCallback(async (id: string) => {
    try {
      await deleteSession(id)
      // Optimistic removal
      setSessions(prev => prev.filter(s => s.id !== id))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete session')
      throw err
    }
  }, [])

  return {
    sessions,
    tree,
    loading,
    error,
    statusConnection,
    addSession,
    removeSession,
    refresh: fetchSessions,
  }
}
