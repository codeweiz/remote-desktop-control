import { useState, useCallback, useRef } from 'react'
import {
  Box,
  BottomNavigation,
  BottomNavigationAction,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Typography,
  IconButton,
  Paper,
  Chip,
  Divider,
  SwipeableDrawer,
  Button,
} from '@mui/material'
import {
  Terminal as TerminalIcon,
  SmartToy as BotIcon,
  ViewList as ListIcon,
  MoreHoriz as MoreIcon,
  Add as AddIcon,
  Delete as DeleteIcon,
  Circle as CircleIcon,
  QrCode2 as QrCodeIcon,
} from '@mui/icons-material'
import type { Session, SessionCreateRequest } from '../lib/types'
import type { SessionTree } from '../hooks/useSessions'
import { TerminalView } from './TerminalView'
import { AgentDrawer } from './AgentDrawer'
import { QRCodeModal } from './QRCodeModal'
import { StatusChip } from './StatusChip'
import MobileInputBar from './MobileInputBar'

type MobileTab = 'sessions' | 'terminal' | 'agent' | 'more'

interface MobileViewProps {
  sessions: Session[]
  tree: SessionTree
  onCreateSession: (req: SessionCreateRequest) => Promise<Session>
  onDeleteSession: (id: string) => void
  onSelectSession: (session: Session) => void
}

export function MobileView({
  sessions,
  tree,
  onCreateSession,
  onDeleteSession,
  onSelectSession,
}: MobileViewProps) {
  const [activeTab, setActiveTab] = useState<MobileTab>('sessions')
  const [activeSession, setActiveSession] = useState<Session | null>(null)
  const [qrOpen, setQrOpen] = useState(false)
  const [fontSize] = useState(() => {
    const stored = localStorage.getItem('rtb_font_size')
    return stored ? parseInt(stored, 10) : 14
  })

  // Ref to hold the terminal's sendData function for MobileInputBar
  const sendDataRef = useRef<((data: string) => void) | null>(null)

  const handleSendReady = useCallback((send: (data: string) => void) => {
    sendDataRef.current = send
  }, [])

  const handleMobileInput = useCallback((data: string) => {
    sendDataRef.current?.(data)
  }, [])

  const handleSelectSession = useCallback((session: Session) => {
    setActiveSession(session)
    setActiveTab(session.kind === 'agent' ? 'agent' : 'terminal')
  }, [])

  const handleCreateTerminal = useCallback(async () => {
    try {
      const session = await onCreateSession({ kind: 'terminal' })
      handleSelectSession(session)
    } catch {
      // handled upstream
    }
  }, [onCreateSession, handleSelectSession])

  const handleCreateAgent = useCallback(async () => {
    try {
      const session = await onCreateSession({ kind: 'agent' })
      handleSelectSession(session)
    } catch {
      // handled upstream
    }
  }, [onCreateSession, handleSelectSession])

  return (
    <Box
      sx={{
        height: '100%',
        width: '100%',
        display: 'flex',
        flexDirection: 'column',
        background: 'linear-gradient(145deg, #020617 0%, #0c1222 40%, #0a1628 100%)',
      }}
    >
      {/* Content area */}
      <Box sx={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
        {/* Sessions tab */}
        {activeTab === 'sessions' && (
          <Box sx={{ height: '100%', overflow: 'auto' }}>
            {/* Header */}
            <Box sx={{ px: 2, pt: 2, pb: 1 }}>
              <Typography variant="h6" sx={{ fontWeight: 700, fontSize: 20 }}>
                Sessions
              </Typography>
            </Box>

            {/* Terminals */}
            <Box sx={{ px: 1 }}>
              <Typography
                variant="overline"
                sx={{
                  px: 1,
                  fontSize: 10,
                  color: 'text.secondary',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 0.5,
                }}
              >
                <TerminalIcon sx={{ fontSize: 12 }} />
                Terminals ({tree.terminals.length})
              </Typography>
              <List dense disablePadding>
                {tree.terminals.map(session => (
                  <ListItemButton
                    key={session.id}
                    onClick={() => handleSelectSession(session)}
                    selected={activeSession?.id === session.id}
                    sx={{ borderRadius: 1, mb: 0.25 }}
                  >
                    <ListItemIcon sx={{ minWidth: 28 }}>
                      <CircleIcon
                        sx={{
                          fontSize: 8,
                          color: session.status === 'running' ? 'success.main' : 'text.secondary',
                        }}
                      />
                    </ListItemIcon>
                    <ListItemText
                      primary={session.name || `terminal-${session.id.slice(0, 6)}`}
                      primaryTypographyProps={{ fontSize: 13, fontWeight: 500 }}
                    />
                    <IconButton
                      size="small"
                      edge="end"
                      onClick={(e) => {
                        e.stopPropagation()
                        onDeleteSession(session.id)
                      }}
                    >
                      <DeleteIcon sx={{ fontSize: 14, color: 'error.main' }} />
                    </IconButton>
                  </ListItemButton>
                ))}
              </List>

              {/* Agents */}
              <Typography
                variant="overline"
                sx={{
                  px: 1,
                  mt: 1.5,
                  fontSize: 10,
                  color: 'text.secondary',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 0.5,
                }}
              >
                <BotIcon sx={{ fontSize: 12 }} />
                Agents ({tree.agents.length})
              </Typography>
              <List dense disablePadding>
                {tree.agents.map(session => (
                  <ListItemButton
                    key={session.id}
                    onClick={() => handleSelectSession(session)}
                    selected={activeSession?.id === session.id}
                    sx={{ borderRadius: 1, mb: 0.25 }}
                  >
                    <ListItemIcon sx={{ minWidth: 28 }}>
                      <CircleIcon
                        sx={{
                          fontSize: 8,
                          color: session.status === 'running' ? '#8b5cf6' : 'text.secondary',
                        }}
                      />
                    </ListItemIcon>
                    <ListItemText
                      primary={session.name || `agent-${session.id.slice(0, 6)}`}
                      primaryTypographyProps={{ fontSize: 13, fontWeight: 500 }}
                    />
                    <IconButton
                      size="small"
                      edge="end"
                      onClick={(e) => {
                        e.stopPropagation()
                        onDeleteSession(session.id)
                      }}
                    >
                      <DeleteIcon sx={{ fontSize: 14, color: 'error.main' }} />
                    </IconButton>
                  </ListItemButton>
                ))}
              </List>
            </Box>

            {/* Create buttons */}
            <Box sx={{ px: 2, py: 2, display: 'flex', gap: 1 }}>
              <Button
                variant="outlined"
                startIcon={<AddIcon />}
                onClick={handleCreateTerminal}
                fullWidth
                sx={{
                  fontSize: 12,
                  borderStyle: 'dashed',
                  color: 'success.main',
                  borderColor: 'success.main',
                }}
              >
                Terminal
              </Button>
              <Button
                variant="outlined"
                startIcon={<AddIcon />}
                onClick={handleCreateAgent}
                fullWidth
                sx={{
                  fontSize: 12,
                  borderStyle: 'dashed',
                  color: 'secondary.main',
                  borderColor: 'secondary.main',
                }}
              >
                Agent
              </Button>
            </Box>
          </Box>
        )}

        {/* Terminal tab */}
        {activeTab === 'terminal' && (
          <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
            {activeSession?.kind === 'terminal' ? (
              <>
                <Box sx={{ flex: 1, minHeight: 0 }}>
                  <TerminalView
                    sessionId={activeSession.id}
                    fontSize={fontSize}
                    onSendReady={handleSendReady}
                  />
                </Box>
                <MobileInputBar onSend={handleMobileInput} />
              </>
            ) : (
              <Box sx={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Box sx={{ textAlign: 'center' }}>
                  <TerminalIcon sx={{ fontSize: 48, color: 'text.secondary', opacity: 0.2, mb: 2 }} />
                  <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                    No terminal selected
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                    Select a terminal session first
                  </Typography>
                </Box>
              </Box>
            )}
          </Box>
        )}

        {/* Agent tab */}
        {activeTab === 'agent' && (
          <Box sx={{ height: '100%' }}>
            {activeSession?.kind === 'agent' ? (
              <AgentDrawer
                open={true}
                session={activeSession}
                width={window.innerWidth}
                onClose={() => setActiveTab('sessions')}
              />
            ) : (
              <Box sx={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Box sx={{ textAlign: 'center' }}>
                  <BotIcon sx={{ fontSize: 48, color: 'text.secondary', opacity: 0.2, mb: 2 }} />
                  <Typography variant="body2" sx={{ color: 'text.secondary' }}>
                    No agent selected
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block', mt: 0.5 }}>
                    Select an agent session first
                  </Typography>
                </Box>
              </Box>
            )}
          </Box>
        )}

        {/* More tab */}
        {activeTab === 'more' && (
          <Box sx={{ height: '100%', overflow: 'auto', p: 2 }}>
            <Typography variant="h6" sx={{ fontWeight: 700, fontSize: 20, mb: 2 }}>
              Settings
            </Typography>

            <Paper sx={{ p: 2, mb: 2 }}>
              <Typography variant="body2" sx={{ fontWeight: 600, mb: 1 }}>
                About
              </Typography>
              <Typography variant="caption" sx={{ color: 'text.secondary', display: 'block' }}>
                RTB 2.0 - Remote Terminal Bridge
              </Typography>
              <Typography
                variant="caption"
                sx={{
                  fontFamily: "'JetBrains Mono', monospace",
                  color: 'text.secondary',
                  display: 'block',
                  mt: 0.5,
                }}
              >
                React 19 + xterm.js + MUI
              </Typography>
            </Paper>

            <Paper sx={{ p: 2, mb: 2 }}>
              <Typography variant="body2" sx={{ fontWeight: 600, mb: 1 }}>
                Status
              </Typography>
              <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                <Chip
                  size="small"
                  icon={<TerminalIcon sx={{ fontSize: 12 }} />}
                  label={`${tree.terminals.length} terminals`}
                  sx={{ fontSize: 11 }}
                />
                <Chip
                  size="small"
                  icon={<BotIcon sx={{ fontSize: 12 }} />}
                  label={`${tree.agents.length} agents`}
                  sx={{ fontSize: 11 }}
                />
              </Box>
            </Paper>

            <Button
              variant="outlined"
              startIcon={<QrCodeIcon />}
              onClick={() => setQrOpen(true)}
              fullWidth
              sx={{ mb: 1, fontSize: 12 }}
            >
              Show QR Code
            </Button>

            <QRCodeModal isOpen={qrOpen} onClose={() => setQrOpen(false)} />
          </Box>
        )}
      </Box>

      {/* Bottom navigation */}
      <Paper
        elevation={8}
        sx={{
          flexShrink: 0,
          borderRadius: 0,
          borderTop: '1px solid',
          borderColor: 'divider',
        }}
        className="safe-area-bottom"
      >
        <BottomNavigation
          value={activeTab}
          onChange={(_, newValue) => setActiveTab(newValue)}
          showLabels
          sx={{
            bgcolor: 'transparent',
            height: 56,
            '& .MuiBottomNavigationAction-root': {
              minWidth: 0,
              py: 0.5,
              '&.Mui-selected': {
                color: 'primary.main',
              },
            },
            '& .MuiBottomNavigationAction-label': {
              fontSize: 10,
              '&.Mui-selected': {
                fontSize: 10,
              },
            },
          }}
        >
          <BottomNavigationAction
            value="sessions"
            label="Sessions"
            icon={<ListIcon sx={{ fontSize: 22 }} />}
          />
          <BottomNavigationAction
            value="terminal"
            label="Terminal"
            icon={<TerminalIcon sx={{ fontSize: 22 }} />}
          />
          <BottomNavigationAction
            value="agent"
            label="Agent"
            icon={<BotIcon sx={{ fontSize: 22 }} />}
          />
          <BottomNavigationAction
            value="more"
            label="More"
            icon={<MoreIcon sx={{ fontSize: 22 }} />}
          />
        </BottomNavigation>
      </Paper>
    </Box>
  )
}
