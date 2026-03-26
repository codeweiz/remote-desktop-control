import { useState, useCallback } from 'react'
import {
  Box,
  Paper,
  Typography,
  IconButton,
  Grid,
  Chip,
} from '@mui/material'
import {
  Terminal as TerminalIcon,
  SmartToy as BotIcon,
  Add as AddIcon,
  Delete as DeleteIcon,
  PlayArrow as PlayIcon,
} from '@mui/icons-material'
import type { Session, SessionCreateRequest } from '../lib/types'
import type { SessionTree } from '../hooks/useSessions'
import { TaskPool } from './TaskPool'
import { StatusChip } from './StatusChip'

interface GridViewProps {
  sessions: Session[]
  tree: SessionTree
  onFocusSession: (session: Session) => void
  onCreateSession: (req: SessionCreateRequest) => Promise<Session>
  onDeleteSession: (id: string) => void
}

function SessionCard({
  session,
  onFocus,
  onDelete,
}: {
  session: Session
  onFocus: () => void
  onDelete: () => void
}) {
  const isAgent = session.kind === 'agent'
  const accentColor = isAgent ? '#8b5cf6' : '#34d399'
  const statusColor =
    session.status === 'running' ? '#34d399' :
    session.status === 'error' ? '#f87171' : '#94a3b8'

  return (
    <Paper
      elevation={2}
      onClick={onFocus}
      sx={{
        p: 2,
        cursor: 'pointer',
        transition: 'all 0.2s',
        borderLeft: `3px solid ${accentColor}`,
        '&:hover': {
          transform: 'translateY(-2px)',
          boxShadow: `0 4px 20px ${accentColor}20`,
          borderColor: accentColor,
        },
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      <Box sx={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between' }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, minWidth: 0 }}>
          {isAgent ? (
            <BotIcon sx={{ fontSize: 18, color: '#8b5cf6' }} />
          ) : (
            <TerminalIcon sx={{ fontSize: 18, color: '#34d399' }} />
          )}
          <Typography
            variant="body2"
            sx={{
              fontWeight: 500,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {session.name || `${session.kind}-${session.id.slice(0, 6)}`}
          </Typography>
        </Box>
        <IconButton
          size="small"
          onClick={(e) => {
            e.stopPropagation()
            onDelete()
          }}
          sx={{
            opacity: 0,
            transition: 'opacity 0.15s',
            '.MuiPaper-root:hover &': { opacity: 1 },
            color: 'error.main',
            p: 0.5,
          }}
        >
          <DeleteIcon sx={{ fontSize: 14 }} />
        </IconButton>
      </Box>

      <Box sx={{ mt: 1.5, display: 'flex', alignItems: 'center', gap: 1 }}>
        <StatusChip label={session.status} color={statusColor} />
        <Chip
          size="small"
          label={isAgent ? 'agent' : 'terminal'}
          sx={{
            fontSize: 10,
            fontFamily: "'JetBrains Mono', monospace",
            bgcolor: 'rgba(255,255,255,0.05)',
            color: 'text.secondary',
          }}
        />
      </Box>

      {/* Monospace preview area */}
      <Box
        sx={{
          mt: 1.5,
          p: 1,
          borderRadius: 1,
          bgcolor: 'rgba(0,0,0,0.3)',
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: 10,
          color: 'text.secondary',
          lineHeight: 1.4,
          minHeight: 40,
          overflow: 'hidden',
        }}
      >
        {isAgent ? (
          <Typography variant="caption" sx={{ color: 'text.secondary', fontStyle: 'italic', fontSize: 10 }}>
            Click to open agent chat...
          </Typography>
        ) : (
          <Typography variant="caption" sx={{ color: 'text.secondary', fontStyle: 'italic', fontSize: 10 }}>
            Click to open terminal...
          </Typography>
        )}
      </Box>

      <Typography
        variant="caption"
        sx={{
          display: 'block',
          mt: 1,
          fontSize: 10,
          fontFamily: "'JetBrains Mono', monospace",
          color: 'text.secondary',
          opacity: 0.6,
        }}
      >
        {session.id.slice(0, 8)}
      </Typography>
    </Paper>
  )
}

function NewSessionCard({ kind, onCreate }: { kind: 'terminal' | 'agent'; onCreate: () => void }) {
  const isAgent = kind === 'agent'
  return (
    <Paper
      elevation={0}
      onClick={onCreate}
      sx={{
        p: 2,
        cursor: 'pointer',
        border: '2px dashed',
        borderColor: 'divider',
        bgcolor: 'transparent',
        transition: 'all 0.2s',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: 140,
        '&:hover': {
          borderColor: isAgent ? '#8b5cf6' : '#34d399',
          bgcolor: isAgent ? 'rgba(139,92,246,0.05)' : 'rgba(52,211,153,0.05)',
        },
      }}
    >
      <AddIcon sx={{ fontSize: 28, color: 'text.secondary', mb: 1 }} />
      <Typography variant="body2" sx={{ color: 'text.secondary', fontWeight: 500 }}>
        New {isAgent ? 'Agent' : 'Terminal'}
      </Typography>
    </Paper>
  )
}

export function GridView({
  sessions,
  tree,
  onFocusSession,
  onCreateSession,
  onDeleteSession,
}: GridViewProps) {
  const handleCreateTerminal = useCallback(async () => {
    try {
      await onCreateSession({ kind: 'terminal' })
    } catch {
      // handled upstream
    }
  }, [onCreateSession])

  const handleCreateAgent = useCallback(async () => {
    try {
      await onCreateSession({ kind: 'agent' })
    } catch {
      // handled upstream
    }
  }, [onCreateSession])

  return (
    <Box sx={{ height: '100%', overflow: 'auto', p: 1.5 }}>
      <Grid container spacing={1.5}>
        {/* Terminal sessions */}
        {tree.terminals.map(session => (
          <Grid size={{ xs: 12, sm: 6, md: 4 }} key={session.id}>
            <SessionCard
              session={session}
              onFocus={() => onFocusSession(session)}
              onDelete={() => onDeleteSession(session.id)}
            />
          </Grid>
        ))}

        {/* Agent sessions */}
        {tree.agents.map(session => (
          <Grid size={{ xs: 12, sm: 6, md: 4 }} key={session.id}>
            <SessionCard
              session={session}
              onFocus={() => onFocusSession(session)}
              onDelete={() => onDeleteSession(session.id)}
            />
          </Grid>
        ))}

        {/* New session cards */}
        <Grid size={{ xs: 12, sm: 6, md: 4 }}>
          <NewSessionCard kind="terminal" onCreate={handleCreateTerminal} />
        </Grid>
        <Grid size={{ xs: 12, sm: 6, md: 4 }}>
          <NewSessionCard kind="agent" onCreate={handleCreateAgent} />
        </Grid>

        {/* Task Pool card */}
        <Grid size={{ xs: 12, sm: 6, md: 4 }}>
          <Paper elevation={2} sx={{ p: 0, overflow: 'hidden' }}>
            <TaskPool />
          </Paper>
        </Grid>

        {/* System Status card */}
        <Grid size={{ xs: 12, sm: 6, md: 4 }}>
          <Paper elevation={2} sx={{ p: 2 }}>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1.5 }}>
              <PlayIcon sx={{ fontSize: 16, color: 'info.main' }} />
              <Typography variant="body2" sx={{ fontWeight: 600 }}>
                System Status
              </Typography>
            </Box>
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
              <Chip
                size="small"
                icon={<TerminalIcon sx={{ fontSize: 12 }} />}
                label={`${tree.terminals.length} terminal${tree.terminals.length !== 1 ? 's' : ''}`}
                sx={{ fontSize: 11 }}
              />
              <Chip
                size="small"
                icon={<BotIcon sx={{ fontSize: 12 }} />}
                label={`${tree.agents.length} agent${tree.agents.length !== 1 ? 's' : ''}`}
                sx={{ fontSize: 11 }}
              />
            </Box>
          </Paper>
        </Grid>
      </Grid>
    </Box>
  )
}
