import { useState } from 'react'
import {
  Box,
  IconButton,
  ToggleButtonGroup,
  ToggleButton,
  Tooltip,
  Typography,
  Chip,
} from '@mui/material'
import {
  ArrowBack as ArrowBackIcon,
  SmartToy as BotIcon,
  Terminal as TerminalIcon,
  Circle as CircleIcon,
} from '@mui/icons-material'
import type { Session } from '../lib/types'
import { TerminalView } from './TerminalView'
import { AgentDrawer } from './AgentDrawer'

interface FocusViewProps {
  sessions: Session[]
  activeSession: Session | null
  agentDrawerOpen: boolean
  onSelectSession: (session: Session) => void
  onToggleAgent: () => void
  onBack: () => void
}

export function FocusView({
  sessions,
  activeSession,
  agentDrawerOpen,
  onSelectSession,
  onToggleAgent,
  onBack,
}: FocusViewProps) {
  const [fontSize] = useState(() => {
    const stored = localStorage.getItem('rtb_font_size')
    return stored ? parseInt(stored, 10) : 14
  })

  const activeAgentSession = activeSession?.kind === 'agent' ? activeSession : null
  const drawerWidth = 360

  return (
    <Box sx={{ display: 'flex', height: '100%', position: 'relative' }}>
      {/* Main content area */}
      <Box
        sx={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          minWidth: 0,
          transition: 'margin-right 0.3s',
          mr: agentDrawerOpen ? `${drawerWidth}px` : 0,
        }}
      >
        {/* Header bar with back button and session tabs */}
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            gap: 1,
            px: 1,
            py: 0.5,
            minHeight: 40,
            borderBottom: '1px solid',
            borderColor: 'divider',
            bgcolor: 'rgba(15, 23, 42, 0.5)',
            flexShrink: 0,
          }}
        >
          <Tooltip title="Back to Grid (Esc)">
            <IconButton size="small" onClick={onBack}>
              <ArrowBackIcon sx={{ fontSize: 18 }} />
            </IconButton>
          </Tooltip>

          {/* Session tabs */}
          <Box sx={{ flex: 1, overflow: 'auto', display: 'flex', gap: 0.5, minWidth: 0 }}>
            <ToggleButtonGroup
              value={activeSession?.id || ''}
              exclusive
              onChange={(_, val) => {
                if (val) {
                  const s = sessions.find(s => s.id === val)
                  if (s) onSelectSession(s)
                }
              }}
              size="small"
              sx={{
                '& .MuiToggleButton-root': {
                  px: 1.5,
                  py: 0.25,
                  fontSize: 11,
                  textTransform: 'none',
                  gap: 0.5,
                  border: '1px solid',
                  borderColor: 'divider',
                },
              }}
            >
              {sessions.map(s => (
                <ToggleButton key={s.id} value={s.id}>
                  <CircleIcon
                    sx={{
                      fontSize: 7,
                      color: s.status === 'running'
                        ? 'success.main'
                        : s.status === 'error'
                          ? 'error.main'
                          : 'text.secondary',
                    }}
                  />
                  <Typography
                    variant="caption"
                    sx={{
                      maxWidth: 100,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                      fontSize: 11,
                    }}
                  >
                    {s.name || `${s.kind}-${s.id.slice(0, 6)}`}
                  </Typography>
                </ToggleButton>
              ))}
            </ToggleButtonGroup>
          </Box>

          {/* Agent toggle */}
          <Tooltip title={agentDrawerOpen ? 'Hide Agent Panel' : 'Show Agent Panel'}>
            <IconButton
              size="small"
              onClick={onToggleAgent}
              sx={{
                color: agentDrawerOpen ? 'secondary.main' : 'text.secondary',
              }}
            >
              <BotIcon sx={{ fontSize: 18 }} />
            </IconButton>
          </Tooltip>
        </Box>

        {/* Terminal area */}
        {activeSession?.kind === 'terminal' ? (
          <TerminalView sessionId={activeSession.id} fontSize={fontSize} />
        ) : activeSession?.kind === 'agent' ? (
          <Box
            sx={{
              flex: 1,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Box sx={{ textAlign: 'center' }}>
              <BotIcon sx={{ fontSize: 48, color: 'text.secondary', opacity: 0.2, mb: 2 }} />
              <Typography variant="body2" sx={{ color: 'text.secondary', fontWeight: 500 }}>
                Agent session active
              </Typography>
              <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                Use the agent panel on the right to chat
              </Typography>
            </Box>
          </Box>
        ) : (
          <Box
            sx={{
              flex: 1,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Box sx={{ textAlign: 'center' }}>
              <TerminalIcon sx={{ fontSize: 48, color: 'text.secondary', opacity: 0.2, mb: 2 }} />
              <Typography variant="body2" sx={{ color: 'text.secondary', fontWeight: 500 }}>
                No terminal selected
              </Typography>
              <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                Select a session from the tabs above or go back to the grid
              </Typography>
            </Box>
          </Box>
        )}
      </Box>

      {/* Agent Drawer */}
      <AgentDrawer
        open={agentDrawerOpen}
        session={activeAgentSession}
        width={drawerWidth}
        onClose={onToggleAgent}
      />
    </Box>
  )
}
