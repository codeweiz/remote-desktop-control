import { useState, useRef, useEffect, useCallback } from 'react'
import {
  Box,
  Drawer,
  Typography,
  TextField,
  IconButton,
  Chip,
  Collapse,
  Alert,
} from '@mui/material'
import {
  Send as SendIcon,
  SmartToy as BotIcon,
  Person as PersonIcon,
  Memory as CpuIcon,
  Close as CloseIcon,
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
  Terminal as TerminalIcon,
  ErrorOutline as ErrorIcon,
  CheckCircleOutline as SuccessIcon,
} from '@mui/icons-material'
import ReactMarkdown from 'react-markdown'
import type { Session, WsMessage } from '../lib/types'
import { useWebSocket } from '../hooks/useWebSocket'

/** Rich chat message type matching the new agent WebSocket protocol. */
interface AgentChatMessage {
  id: string
  type: 'user' | 'text' | 'thinking' | 'tool_use' | 'tool_result' | 'progress' | 'error'
  content: string
  timestamp: string
  // tool_use specific
  toolName?: string
  toolId?: string
  toolInput?: string
  // tool_result specific
  toolOutput?: string
  isError?: boolean
  // error specific
  severity?: string
  guidance?: string
}

interface AgentDrawerProps {
  open: boolean
  session: Session | null
  width: number
  onClose: () => void
}

/** Render a thinking message as a collapsible gray box. */
function ThinkingMessage({ msg }: { msg: AgentChatMessage }) {
  const [expanded, setExpanded] = useState(false)
  return (
    <Box
      sx={{
        bgcolor: 'rgba(255,255,255,0.03)',
        border: '1px solid',
        borderColor: 'rgba(255,255,255,0.06)',
        borderRadius: 1.5,
        overflow: 'hidden',
      }}
    >
      <Box
        onClick={() => setExpanded(!expanded)}
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 0.5,
          px: 1.5,
          py: 0.75,
          cursor: 'pointer',
          '&:hover': { bgcolor: 'rgba(255,255,255,0.03)' },
        }}
      >
        {expanded ? (
          <ExpandLessIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
        ) : (
          <ExpandMoreIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
        )}
        <Typography
          variant="caption"
          sx={{ color: 'text.secondary', fontStyle: 'italic', fontSize: 11 }}
        >
          Thinking...
        </Typography>
      </Box>
      <Collapse in={expanded}>
        <Box sx={{ px: 1.5, pb: 1, pt: 0.5 }}>
          <Typography
            variant="body2"
            sx={{
              color: 'text.secondary',
              fontStyle: 'italic',
              fontSize: 12,
              lineHeight: 1.5,
              whiteSpace: 'pre-wrap',
            }}
          >
            {msg.content}
          </Typography>
        </Box>
      </Collapse>
    </Box>
  )
}

/** Render a tool_use message as a compact card. */
function ToolUseMessage({ msg }: { msg: AgentChatMessage }) {
  const [expanded, setExpanded] = useState(false)
  const inputPreview = msg.toolInput
    ? msg.toolInput.length > 80
      ? msg.toolInput.slice(0, 80) + '...'
      : msg.toolInput
    : ''

  return (
    <Box
      sx={{
        bgcolor: 'rgba(139,92,246,0.06)',
        border: '1px solid',
        borderColor: 'rgba(139,92,246,0.15)',
        borderRadius: 1.5,
        overflow: 'hidden',
      }}
    >
      <Box
        onClick={() => setExpanded(!expanded)}
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 0.75,
          px: 1.5,
          py: 0.75,
          cursor: 'pointer',
          '&:hover': { bgcolor: 'rgba(139,92,246,0.08)' },
        }}
      >
        <TerminalIcon sx={{ fontSize: 13, color: 'secondary.main' }} />
        <Chip
          size="small"
          label={msg.toolName || 'tool'}
          sx={{
            fontSize: 10,
            height: 18,
            fontFamily: "'JetBrains Mono', monospace",
            bgcolor: 'rgba(139,92,246,0.12)',
            color: 'secondary.light',
          }}
        />
        {!expanded && inputPreview && (
          <Typography
            variant="caption"
            sx={{
              color: 'text.secondary',
              fontSize: 11,
              fontFamily: "'JetBrains Mono', monospace",
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              flex: 1,
            }}
          >
            {inputPreview}
          </Typography>
        )}
        {expanded ? (
          <ExpandLessIcon sx={{ fontSize: 14, color: 'text.secondary', ml: 'auto' }} />
        ) : (
          <ExpandMoreIcon sx={{ fontSize: 14, color: 'text.secondary', ml: 'auto' }} />
        )}
      </Box>
      <Collapse in={expanded}>
        <Box
          sx={{
            px: 1.5,
            pb: 1,
            pt: 0.5,
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: 11,
            color: 'text.secondary',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            maxHeight: 200,
            overflow: 'auto',
          }}
        >
          {msg.toolInput || '{}'}
        </Box>
      </Collapse>
    </Box>
  )
}

/** Render a tool_result message as a compact card. */
function ToolResultMessage({ msg }: { msg: AgentChatMessage }) {
  const [expanded, setExpanded] = useState(false)
  const isErr = msg.isError === true
  const output = msg.toolOutput || msg.content || ''
  const preview = output.length > 100 ? output.slice(0, 100) + '...' : output

  return (
    <Box
      sx={{
        bgcolor: isErr ? 'rgba(239,68,68,0.06)' : 'rgba(34,197,94,0.06)',
        border: '1px solid',
        borderColor: isErr ? 'rgba(239,68,68,0.15)' : 'rgba(34,197,94,0.15)',
        borderRadius: 1.5,
        overflow: 'hidden',
      }}
    >
      <Box
        onClick={() => setExpanded(!expanded)}
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 0.75,
          px: 1.5,
          py: 0.75,
          cursor: 'pointer',
          '&:hover': {
            bgcolor: isErr ? 'rgba(239,68,68,0.08)' : 'rgba(34,197,94,0.08)',
          },
        }}
      >
        {isErr ? (
          <ErrorIcon sx={{ fontSize: 13, color: 'error.main' }} />
        ) : (
          <SuccessIcon sx={{ fontSize: 13, color: 'success.main' }} />
        )}
        <Typography
          variant="caption"
          sx={{
            fontSize: 11,
            color: isErr ? 'error.light' : 'success.light',
            fontWeight: 500,
          }}
        >
          {isErr ? 'Error' : 'Result'}
        </Typography>
        {!expanded && (
          <Typography
            variant="caption"
            sx={{
              color: 'text.secondary',
              fontSize: 11,
              fontFamily: "'JetBrains Mono', monospace",
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              flex: 1,
            }}
          >
            {preview}
          </Typography>
        )}
        {expanded ? (
          <ExpandLessIcon sx={{ fontSize: 14, color: 'text.secondary', ml: 'auto' }} />
        ) : (
          <ExpandMoreIcon sx={{ fontSize: 14, color: 'text.secondary', ml: 'auto' }} />
        )}
      </Box>
      <Collapse in={expanded}>
        <Box
          sx={{
            px: 1.5,
            pb: 1,
            pt: 0.5,
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: 11,
            color: isErr ? 'error.light' : 'text.secondary',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-all',
            maxHeight: 300,
            overflow: 'auto',
          }}
        >
          {output}
        </Box>
      </Collapse>
    </Box>
  )
}

/** Render a single chat message based on its type. */
function ChatMessage({ msg }: { msg: AgentChatMessage }) {
  if (msg.type === 'user') {
    return (
      <Box
        sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}
        className="animate-fade-in"
      >
        <Box
          sx={{
            maxWidth: '85%',
            fontSize: 13,
            bgcolor: 'rgba(255,255,255,0.08)',
            borderRadius: 2,
            px: 1.5,
            py: 1,
          }}
        >
          <Typography variant="body2" sx={{ fontSize: 13, lineHeight: 1.5 }}>
            {msg.content}
          </Typography>
        </Box>
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
      </Box>
    )
  }

  if (msg.type === 'thinking') {
    return (
      <Box sx={{ display: 'flex', gap: 1 }} className="animate-fade-in">
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
        <Box sx={{ maxWidth: '85%', flex: 1 }}>
          <ThinkingMessage msg={msg} />
        </Box>
      </Box>
    )
  }

  if (msg.type === 'tool_use') {
    return (
      <Box sx={{ display: 'flex', gap: 1 }} className="animate-fade-in">
        <Box sx={{ width: 28, flexShrink: 0 }} />
        <Box sx={{ maxWidth: '85%', flex: 1 }}>
          <ToolUseMessage msg={msg} />
        </Box>
      </Box>
    )
  }

  if (msg.type === 'tool_result') {
    return (
      <Box sx={{ display: 'flex', gap: 1 }} className="animate-fade-in">
        <Box sx={{ width: 28, flexShrink: 0 }} />
        <Box sx={{ maxWidth: '85%', flex: 1 }}>
          <ToolResultMessage msg={msg} />
        </Box>
      </Box>
    )
  }

  if (msg.type === 'progress') {
    return (
      <Box
        sx={{ display: 'flex', justifyContent: 'center', py: 0.25 }}
        className="animate-fade-in"
      >
        <Typography
          variant="caption"
          sx={{
            color: 'text.secondary',
            fontSize: 10,
            fontFamily: "'JetBrains Mono', monospace",
            fontStyle: 'italic',
          }}
        >
          {msg.content}
        </Typography>
      </Box>
    )
  }

  if (msg.type === 'error') {
    return (
      <Box sx={{ display: 'flex', gap: 1 }} className="animate-fade-in">
        <Box sx={{ width: 28, flexShrink: 0 }} />
        <Alert
          severity="error"
          variant="outlined"
          sx={{
            flex: 1,
            fontSize: 12,
            py: 0.5,
            '& .MuiAlert-message': { fontSize: 12 },
          }}
        >
          <Typography variant="body2" sx={{ fontSize: 12, fontWeight: 500 }}>
            {msg.content}
          </Typography>
          {msg.guidance && (
            <Typography variant="caption" sx={{ display: 'block', mt: 0.5, fontSize: 11 }}>
              {msg.guidance}
            </Typography>
          )}
        </Alert>
      </Box>
    )
  }

  // Default: text message (assistant text with markdown)
  return (
    <Box sx={{ display: 'flex', gap: 1 }} className="animate-fade-in">
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
      <Box
        sx={{
          maxWidth: '85%',
          fontSize: 13,
          borderLeft: '2px solid',
          borderColor: 'secondary.main',
          pl: 1.5,
          py: 0.5,
        }}
      >
        <Box className="markdown-content" sx={{ fontSize: 13, lineHeight: 1.5 }}>
          <ReactMarkdown>{msg.content}</ReactMarkdown>
        </Box>
      </Box>
    </Box>
  )
}

export function AgentDrawer({ open, session, width, onClose }: AgentDrawerProps) {
  const [messages, setMessages] = useState<AgentChatMessage[]>([])
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
    const msgType = msg.type as string

    // On connect, clear messages — the server will replay full history
    if (msgType === 'status') {
      if ((msg.status as string) === 'connected') {
        setMessages([])
      }
      return
    }

    if (msgType === 'text') {
      const chatMsg: AgentChatMessage = {
        id: `text-${msg.seq ?? crypto.randomUUID()}`,
        type: 'text',
        content: (msg.content as string) || '',
        timestamp: new Date().toISOString(),
      }
      setMessages(prev => {
        // If streaming and previous message has same seq, replace it
        if (msg.streaming === true) {
          const existing = prev.findIndex(m => m.id === chatMsg.id)
          if (existing >= 0) {
            const updated = [...prev]
            updated[existing] = chatMsg
            return updated
          }
        }
        return [...prev, chatMsg]
      })
      return
    }

    if (msgType === 'thinking') {
      setMessages(prev => [
        ...prev,
        {
          id: `thinking-${msg.seq ?? crypto.randomUUID()}`,
          type: 'thinking',
          content: (msg.content as string) || '',
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }

    if (msgType === 'tool_use') {
      const inputStr =
        typeof msg.input === 'string' ? msg.input : JSON.stringify(msg.input ?? {}, null, 2)
      setMessages(prev => [
        ...prev,
        {
          id: `tool_use-${msg.seq ?? crypto.randomUUID()}`,
          type: 'tool_use',
          content: `Using ${msg.name as string}`,
          toolName: msg.name as string,
          toolId: msg.id as string,
          toolInput: inputStr,
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }

    if (msgType === 'tool_result') {
      setMessages(prev => [
        ...prev,
        {
          id: `tool_result-${msg.seq ?? crypto.randomUUID()}`,
          type: 'tool_result',
          content: (msg.output as string) || '',
          toolId: msg.id as string,
          toolOutput: (msg.output as string) || '',
          isError: msg.is_error === true,
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }

    if (msgType === 'progress') {
      setMessages(prev => [
        ...prev,
        {
          id: `progress-${msg.seq ?? crypto.randomUUID()}`,
          type: 'progress',
          content: (msg.message as string) || '',
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }

    if (msgType === 'turn_complete') {
      // Optionally show cost; for now just mark turn as done via a progress entry
      const cost = msg.cost_usd as number | null
      const costStr = cost != null ? ` ($${cost.toFixed(4)})` : ''
      setMessages(prev => [
        ...prev,
        {
          id: `turn-${msg.seq ?? crypto.randomUUID()}`,
          type: 'progress',
          content: `Turn complete${costStr}`,
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }

    if (msgType === 'error') {
      setMessages(prev => [
        ...prev,
        {
          id: `error-${msg.seq ?? crypto.randomUUID()}`,
          type: 'error',
          content: (msg.message as string) || 'Unknown error',
          severity: (msg.severity as string) || 'transient',
          guidance: (msg.guidance as string) || '',
          timestamp: new Date().toISOString(),
        },
      ])
      return
    }
  }, [])

  const { send, connectionState } = useWebSocket({
    path: session ? `/ws/agent?session=${session.id}` : '',
    enabled: !!session && session.kind === 'agent',
    onMessage: handleAgentMessage,
  })

  const handleSend = useCallback(() => {
    if (!input.trim() || !session) return

    const userMsg: AgentChatMessage = {
      id: crypto.randomUUID(),
      type: 'user',
      content: input.trim(),
      timestamp: new Date().toISOString(),
    }
    setMessages(prev => [...prev, userMsg])

    send({
      type: 'message',
      text: input.trim(),
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
            messages.map(msg => <ChatMessage key={msg.id} msg={msg} />)
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
