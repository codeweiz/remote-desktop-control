import { useState, useCallback, useEffect } from 'react'
import { Box, useMediaQuery } from '@mui/material'
import { TopBar } from './components/TopBar'
import { GridView } from './components/GridView'
import { FocusView } from './components/FocusView'
import { MobileView } from './components/MobileView'
import { CommandPalette } from './components/CommandPalette'
import { NotificationToast } from './components/NotificationToast'
import { useSessions } from './hooks/useSessions'
import { useWebSocket } from './hooks/useWebSocket'
import type { Session } from './lib/types'

type ViewMode = 'grid' | 'focus'

export default function App() {
  const isMobile = useMediaQuery('(max-width:768px)')
  const {
    tree,
    statusConnection,
    addSession,
    removeSession,
  } = useSessions()

  const [viewMode, setViewMode] = useState<ViewMode>('grid')
  const [activeSession, setActiveSession] = useState<Session | null>(null)
  const [cmdPaletteOpen, setCmdPaletteOpen] = useState(false)

  // Status WS for latency measurement
  const { latency } = useWebSocket({
    path: '/ws/status',
    enabled: true,
  })

  // Enter focus mode when clicking a session card
  const handleFocusSession = useCallback((session: Session) => {
    setActiveSession(session)
    setViewMode('focus')
  }, [])

  // Back to grid
  const handleBackToGrid = useCallback(() => {
    setViewMode('grid')
  }, [])

  // Create new terminal
  const handleCreateTerminal = useCallback(async () => {
    try {
      const session = await addSession({ kind: 'terminal' })
      handleFocusSession(session)
    } catch {
      // Error already tracked in useSessions
    }
  }, [addSession, handleFocusSession])

  // Delete session
  const handleDeleteSession = useCallback(async (id: string) => {
    try {
      await removeSession(id)
      if (activeSession?.id === id) {
        setActiveSession(null)
        setViewMode('grid')
      }
    } catch {
      // Error already tracked in useSessions
    }
  }, [removeSession, activeSession])

  // Navigate to session from notification toast
  const handleNavigateToSession = useCallback((sessionId: string) => {
    const allSessions = [...tree.terminals, ...tree.agents]
    const session = allSessions.find(s => s.id === sessionId)
    if (session) {
      handleFocusSession(session)
    }
  }, [tree, handleFocusSession])

  // Cmd+K
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setCmdPaletteOpen(v => !v)
      }
      if (e.key === 'Escape' && viewMode === 'focus') handleBackToGrid()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [viewMode, handleBackToGrid])

  if (isMobile) {
    return (
      <>
        <MobileView
          sessions={[...tree.terminals, ...tree.agents]}
          tree={tree}
          onCreateSession={addSession}
          onDeleteSession={handleDeleteSession}
          onSelectSession={handleFocusSession}
        />
        <NotificationToast onNavigateToSession={handleNavigateToSession} />
      </>
    )
  }

  return (
    <Box sx={{
      height: '100%',
      width: '100%',
      background: 'linear-gradient(145deg, #020617 0%, #0c1222 40%, #0a1628 100%)',
      display: 'flex',
      flexDirection: 'column',
    }}>
      <TopBar
        viewMode={viewMode}
        connectionState={statusConnection}
        latency={latency}
        onToggleView={() => setViewMode(v => v === 'grid' ? 'focus' : 'grid')}
        onOpenCommandPalette={() => setCmdPaletteOpen(true)}
      />
      <Box sx={{ flex: 1, minHeight: 0 }}>
        {viewMode === 'grid' ? (
          <GridView
            sessions={[...tree.terminals, ...tree.agents]}
            tree={tree}
            onFocusSession={handleFocusSession}
            onCreateSession={addSession}
            onDeleteSession={handleDeleteSession}
          />
        ) : (
          <FocusView
            sessions={tree.terminals}
            activeSession={activeSession}
            onSelectSession={setActiveSession}
            onBack={handleBackToGrid}
          />
        )}
      </Box>
      <CommandPalette
        open={cmdPaletteOpen}
        onClose={() => setCmdPaletteOpen(false)}
        onNewTerminal={handleCreateTerminal}
      />
      <NotificationToast onNavigateToSession={handleNavigateToSession} />
    </Box>
  )
}
