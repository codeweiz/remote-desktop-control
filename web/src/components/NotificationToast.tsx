import { useState, useEffect, useCallback, useRef } from 'react'
import { X, Bot, ListTodo, Terminal, AlertCircle } from 'lucide-react'
import type { NotificationEvent, WsMessage } from '../lib/types'
import { useWebSocket } from '../hooks/useWebSocket'

interface ToastItem {
  notification: NotificationEvent
  expiresAt: number
}

function triggerIcon(trigger: NotificationEvent['trigger']) {
  switch (trigger) {
    case 'agent': return <Bot size={14} className="text-accent-purple" />
    case 'task': return <ListTodo size={14} className="text-accent-orange" />
    case 'session': return <Terminal size={14} className="text-accent-green" />
    case 'system': return <AlertCircle size={14} className="text-accent-blue" />
  }
}

interface NotificationToastProps {
  onNavigateToSession?: (sessionId: string) => void
}

export function NotificationToast({ onNavigateToSession }: NotificationToastProps) {
  const [toasts, setToasts] = useState<ToastItem[]>([])
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Clean up expired toasts
  useEffect(() => {
    timerRef.current = setInterval(() => {
      const now = Date.now()
      setToasts(prev => {
        const next = prev.filter(t => t.expiresAt > now)
        return next.length !== prev.length ? next : prev
      })
    }, 1000)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [])

  const handleMessage = useCallback((msg: WsMessage) => {
    if (msg.type === 'notification') {
      const notification: NotificationEvent = {
        type: 'notification',
        id: (msg.id as string) || crypto.randomUUID(),
        trigger: (msg.trigger as NotificationEvent['trigger']) || 'system',
        summary: (msg.summary as string) || '',
        session_id: msg.session_id as string | undefined,
        session_name: msg.session_name as string | undefined,
        timestamp: (msg.timestamp as string) || new Date().toISOString(),
      }
      setToasts(prev => [
        ...prev.slice(-4), // Keep max 5 toasts
        { notification, expiresAt: Date.now() + 5000 },
      ])
    }
  }, [])

  useWebSocket({
    path: '/ws/status',
    onMessage: handleMessage,
  })

  const dismissToast = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.notification.id !== id))
  }, [])

  if (toasts.length === 0) return null

  return (
    <div className="fixed top-12 right-4 z-[90] flex flex-col gap-2 max-w-[320px]">
      {toasts.map(toast => (
        <div
          key={toast.notification.id}
          className="flex items-start gap-2 bg-bg-secondary border border-border rounded-lg px-3 py-2 shadow-lg animate-slide-in cursor-pointer hover:bg-bg-tertiary transition-colors"
          onClick={() => {
            if (toast.notification.session_id && onNavigateToSession) {
              onNavigateToSession(toast.notification.session_id)
            }
            dismissToast(toast.notification.id)
          }}
        >
          <span className="shrink-0 mt-0.5">{triggerIcon(toast.notification.trigger)}</span>
          <div className="flex-1 min-w-0">
            <p className="text-xs text-text-primary">{toast.notification.summary}</p>
            {toast.notification.session_name && (
              <p className="text-[10px] text-text-secondary mt-0.5">{toast.notification.session_name}</p>
            )}
          </div>
          <button
            className="shrink-0 p-0.5 rounded hover:bg-bg-secondary text-text-secondary hover:text-text-primary transition-colors"
            onClick={(e) => {
              e.stopPropagation()
              dismissToast(toast.notification.id)
            }}
          >
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  )
}
