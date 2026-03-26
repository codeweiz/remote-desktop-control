import { useState, useEffect, useCallback, useRef } from 'react'
import { Box, Snackbar, Alert, Typography, IconButton } from '@mui/material'
import {
  SmartToy as BotIcon,
  Checklist as TaskIcon,
  Terminal as TerminalIcon,
  Info as SystemIcon,
  Close as CloseIcon,
} from '@mui/icons-material'
import type { NotificationEvent, WsMessage } from '../lib/types'
import { useWebSocket } from '../hooks/useWebSocket'

interface ToastItem {
  notification: NotificationEvent
  expiresAt: number
}

function triggerIcon(trigger: NotificationEvent['trigger']) {
  switch (trigger) {
    case 'agent': return <BotIcon sx={{ fontSize: 16, color: '#8b5cf6' }} />
    case 'task': return <TaskIcon sx={{ fontSize: 16, color: '#fbbf24' }} />
    case 'session': return <TerminalIcon sx={{ fontSize: 16, color: '#34d399' }} />
    case 'system': return <SystemIcon sx={{ fontSize: 16, color: '#22d3ee' }} />
  }
}

interface NotificationToastProps {
  onNavigateToSession?: (sessionId: string) => void
}

export function NotificationToast({ onNavigateToSession }: NotificationToastProps) {
  const [toasts, setToasts] = useState<ToastItem[]>([])
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

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
        ...prev.slice(-4),
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
    <Box
      sx={{
        position: 'fixed',
        top: 64,
        right: 16,
        zIndex: 1400,
        display: 'flex',
        flexDirection: 'column',
        gap: 1,
        maxWidth: 320,
      }}
    >
      {toasts.map(toast => (
        <Alert
          key={toast.notification.id}
          severity="info"
          icon={triggerIcon(toast.notification.trigger)}
          action={
            <IconButton
              size="small"
              onClick={(e) => {
                e.stopPropagation()
                dismissToast(toast.notification.id)
              }}
            >
              <CloseIcon sx={{ fontSize: 14 }} />
            </IconButton>
          }
          onClick={() => {
            if (toast.notification.session_id && onNavigateToSession) {
              onNavigateToSession(toast.notification.session_id)
            }
            dismissToast(toast.notification.id)
          }}
          sx={{
            cursor: 'pointer',
            backdropFilter: 'blur(12px)',
            bgcolor: 'rgba(15, 23, 42, 0.9)',
            border: '1px solid',
            borderColor: 'divider',
            '&:hover': {
              bgcolor: 'rgba(15, 23, 42, 0.95)',
            },
          }}
          className="animate-slide-in"
        >
          <Typography variant="caption" sx={{ fontSize: 12 }}>
            {toast.notification.summary}
          </Typography>
          {toast.notification.session_name && (
            <Typography
              variant="caption"
              sx={{
                display: 'block',
                fontSize: 10,
                fontFamily: "'JetBrains Mono', monospace",
                color: 'text.secondary',
                mt: 0.25,
              }}
            >
              {toast.notification.session_name}
            </Typography>
          )}
        </Alert>
      ))}
    </Box>
  )
}
