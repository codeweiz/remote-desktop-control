import { useState, useRef, useEffect, useCallback } from 'react'
import {
  Box,
  Drawer,
  Paper,
  Typography,
  TextField,
  IconButton,
  Chip,
} from '@mui/material'
import {
  Send as SendIcon,
  SmartToy as BotIcon,
  Person as PersonIcon,
  Memory as CpuIcon,
  Close as CloseIcon,
} from '@mui/icons-material'
import ReactMarkdown from 'react-markdown'
import type { Session, AgentMessage, WsMessage } from '../lib/types'
import { useWebSocket } from '../hooks/useWebSocket'

interface AgentDrawerProps {
  open: boolean
  session: Session | null
  width: number
  onClose: () => void
}

export function AgentDrawer({ open, session, width, onClose }: AgentDrawerProps) {
  const [messages, setMessages] = useState<AgentMessage[]>([])
  const [input, setInput] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [])

  useEffect(() => {
    scrollToBottom()
  }, [messages, scrollToBottom])

  // Reset messages when session changes
  useEffect(() => {
    setMessages([])
  }, [session?.id])

  const handleAgentMessage = useCallback((msg: WsMessage) => {
    if (msg.type === 'agent_message') {
      const agentMsg: AgentMessage = {
        id: (msg.id as string) || crypto.randomUUID(),
        role: (msg.role as AgentMessage['role']) || 'assistant',
        content: (msg.content as string) || '',
        timestamp: (msg.timestamp as string) || new Date().toISOString(),
        model: msg.model as string | undefined,
      }
      setMessages(prev => [...prev, agentMsg])
    }
  }, [])

  const { send, connectionState } = useWebSocket({
    path: session ? `/ws/agent/${session.id}` : '',
    enabled: !!session && session.kind === 'agent',
    onMessage: handleAgentMessage,
  })

  const handleSend = useCallback(() => {
    if (!input.trim() || !session) return

    const userMsg: AgentMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: input.trim(),
      timestamp: new Date().toISOString(),
    }
    setMessages(prev => [...prev, userMsg])

    send({
      type: 'agent_input',
      content: input.trim(),
    })

    setInput('')
  }, [input, session, send])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }, [handleSend])

  return (
    <Drawer
      anchor="right"
      variant="persistent"
      open={open}
      sx={{
        width: open ? width : 0,
        flexShrink: 0,
        '& .MuiDrawer-paper': {
          width,
          position: 'absolute',
          border: 'none',
        },
      }}
    >
      <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        {/* Header */}
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            px: 2,
            py: 1,
            borderBottom: '1px solid',
            borderColor: 'divider',
            minHeight: 48,
          }}
        >
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Box
              sx={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                bgcolor: 'secondary.main',
                animation: 'pulse-glow 2s ease-in-out infinite',
              }}
            />
            <Typography variant="subtitle2" sx={{ fontWeight: 600, fontSize: 13 }}>
              {session ? (session.name || `Agent ${session.id.slice(0, 6)}`) : 'Agent Chat'}
            </Typography>
            {session && (
              <Chip
                size="small"
                icon={<CpuIcon sx={{ fontSize: 10 }} />}
                label="agent"
                sx={{
                  fontSize: 10,
                  fontFamily: "'JetBrains Mono', monospace",
                  height: 20,
                }}
              />
            )}
          </Box>
          <IconButton size="small" onClick={onClose}>
            <CloseIcon sx={{ fontSize: 16 }} />
          </IconButton>
        </Box>

        {/* Messages */}
        <Box sx={{ flex: 1, overflow: 'auto', p: 2, display: 'flex', flexDirection: 'column', gap: 1.5 }}>
          {!session ? (
            <Box sx={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <Box sx={{ textAlign: 'center' }}>
                <BotIcon sx={{ fontSize: 40, color: 'secondary.main', opacity: 0.3, mb: 1.5 }} />
                <Typography variant="body2" sx={{ color: 'text.secondary', fontWeight: 500 }}>
                  Select an agent session
                </Typography>
                <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                  or create one from the grid
                </Typography>
              </Box>
            </Box>
          ) : messages.length === 0 ? (
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: 120 }}>
              <Box sx={{ textAlign: 'center' }}>
                <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                  No messages yet
                </Typography>
                <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                  {connectionState === 'connected' ? 'Type a message below' : 'Connecting...'}
                </Typography>
              </Box>
            </Box>
          ) : (
            messages.map(msg => (
              <Box
                key={msg.id}
                sx={{
                  display: 'flex',
                  gap: 1,
                  justifyContent: msg.role === 'user' ? 'flex-end' : 'flex-start',
                }}
                className="animate-fade-in"
              >
                {msg.role !== 'user' && (
                  <Box
                    sx={{
                      width: 28,
                      height: 28,
                      borderRadius: 1,
                      bgcolor: 'rgba(139,92,246,0.15)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      flexShrink: 0,
                      mt: 0.25,
                    }}
                  >
                    <BotIcon sx={{ fontSize: 14, color: 'secondary.main' }} />
                  </Box>
                )}
                <Box
                  sx={{
                    maxWidth: '85%',
                    fontSize: 13,
                    ...(msg.role === 'user'
                      ? {
                          bgcolor: 'rgba(255,255,255,0.08)',
                          borderRadius: 2,
                          px: 1.5,
                          py: 1,
                        }
                      : {
                          borderLeft: '2px solid',
                          borderColor: 'secondary.main',
                          pl: 1.5,
                          py: 0.5,
                        }),
                  }}
                >
                  <Box className="markdown-content" sx={{ fontSize: 13, lineHeight: 1.5 }}>
                    <ReactMarkdown>{msg.content}</ReactMarkdown>
                  </Box>
                  {msg.model && (
                    <Typography
                      variant="caption"
                      sx={{
                        display: 'block',
                        mt: 0.5,
                        fontSize: 10,
                        fontFamily: "'JetBrains Mono', monospace",
                        color: 'text.secondary',
                      }}
                    >
                      {msg.model}
                    </Typography>
                  )}
                </Box>
                {msg.role === 'user' && (
                  <Box
                    sx={{
                      width: 28,
                      height: 28,
                      borderRadius: 1,
                      bgcolor: 'rgba(59,130,246,0.15)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      flexShrink: 0,
                      mt: 0.25,
                    }}
                  >
                    <PersonIcon sx={{ fontSize: 14, color: 'primary.main' }} />
                  </Box>
                )}
              </Box>
            ))
          )}
          <div ref={messagesEndRef} />
        </Box>

        {/* Input */}
        {session && (
          <Box
            sx={{
              borderTop: '1px solid',
              borderColor: 'divider',
              p: 1.5,
              display: 'flex',
              alignItems: 'flex-end',
              gap: 1,
            }}
          >
            <TextField
              multiline
              maxRows={4}
              size="small"
              fullWidth
              placeholder="Message agent..."
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={connectionState !== 'connected'}
              sx={{
                '& .MuiOutlinedInput-root': {
                  fontSize: 13,
                  borderRadius: 2,
                },
              }}
            />
            <IconButton
              size="small"
              onClick={handleSend}
              disabled={!input.trim() || connectionState !== 'connected'}
              sx={{
                color: 'secondary.main',
                '&:disabled': { opacity: 0.3 },
              }}
            >
              <SendIcon sx={{ fontSize: 18 }} />
            </IconButton>
          </Box>
        )}
      </Box>
    </Drawer>
  )
}
