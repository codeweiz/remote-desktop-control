import { useState, useEffect, useCallback } from 'react'
import type { Session } from './lib/types'
import { renameSession } from './lib/api'
import { useTheme } from './hooks/useTheme'
import { useSessions } from './hooks/useSessions'
import { useWebSocket } from './hooks/useWebSocket'
import { TopBar } from './components/TopBar'
import { StatusBar } from './components/StatusBar'
import { SessionList } from './components/SessionList'
import { TerminalView } from './components/TerminalView'
import { AgentChat } from './components/AgentChat'
import { CommandPalette } from './components/CommandPalette'
import { SettingsPanel } from './components/SettingsPanel'
import { NotificationToast } from './components/NotificationToast'
import { MobileNav } from './components/MobileNav'
import type { MobileTab } from './components/MobileNav'

export default function App() {
  const { theme, toggleTheme } = useTheme()
  const {
    tree,
    statusConnection,
    addSession,
    removeSession,
  } = useSessions()

  // UI state
  const [activeSession, setActiveSession] = useState<Session | null>(null)
  const [openTabs, setOpenTabs] = useState<Session[]>([])
  const [agentPanelVisible, setAgentPanelVisible] = useState(true)
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [fontSize, setFontSize] = useState(() => {
    const stored = localStorage.getItem('rtb_font_size')
    return stored ? parseInt(stored, 10) : 14
  })

  // Mobile state
  const [sidebarVisible, setSidebarVisible] = useState(true)
  const [mobileTab, setMobileTab] = useState<MobileTab>('terminal')
  const [isMobile, setIsMobile] = useState(false)

  // Detect mobile
  useEffect(() => {
    const checkMobile = () => {
      const mobile = window.innerWidth < 768
      setIsMobile(mobile)
      if (mobile) {
        setSidebarVisible(false)
        setAgentPanelVisible(false)
      } else {
        setSidebarVisible(true)
      }
    }
    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  // Active agent session for the chat panel
  const activeAgentSession = activeSession?.kind === 'agent' ? activeSession : null

  // Status WS for latency measurement
  const { latency } = useWebSocket({
    path: '/ws/status',
    enabled: true,
  })

  // Persist font size
  useEffect(() => {
    localStorage.setItem('rtb_font_size', String(fontSize))
  }, [fontSize])

  // Cmd+K / Ctrl+K to open command palette
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setCommandPaletteOpen(prev => !prev)
      }
      if (e.key === 'Escape') {
        setCommandPaletteOpen(false)
        setSettingsOpen(false)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  // Select a session (open it as a tab too)
  const handleSelectSession = useCallback((session: Session) => {
    setActiveSession(session)
    setOpenTabs(prev => {
      if (prev.some(t => t.id === session.id)) return prev
      return [...prev, session]
    })
    // Show agent panel if it's an agent session
    if (session.kind === 'agent') {
      setAgentPanelVisible(true)
    }
    // On mobile, switch to appropriate tab and close sidebar
    if (isMobile) {
      setSidebarVisible(false)
      setMobileTab(session.kind === 'agent' ? 'chat' : 'terminal')
    }
  }, [isMobile])

  // Close a tab
  const handleCloseTab = useCallback((sessionId: string) => {
    setOpenTabs(prev => {
      const next = prev.filter(t => t.id !== sessionId)
      // If we closed the active tab, switch to the last remaining one
      if (activeSession?.id === sessionId) {
        setActiveSession(next.length > 0 ? next[next.length - 1] : null)
      }
      return next
    })
  }, [activeSession])

  // Create new terminal
  const handleCreateTerminal = useCallback(async () => {
    try {
      const session = await addSession({ kind: 'terminal' })
      handleSelectSession(session)
    } catch {
      // Error already tracked in useSessions
    }
  }, [addSession, handleSelectSession])

  // Create new agent
  const handleCreateAgent = useCallback(async () => {
    try {
      const session = await addSession({ kind: 'agent' })
      handleSelectSession(session)
    } catch {
      // Error already tracked in useSessions
    }
  }, [addSession, handleSelectSession])

  // Delete session
  const handleDeleteSession = useCallback(async (id: string) => {
    handleCloseTab(id)
    try {
      await removeSession(id)
    } catch {
      // Error already tracked in useSessions
    }
  }, [removeSession, handleCloseTab])

  // Rename session
  const handleRenameSession = useCallback(async (id: string, name: string) => {
    try {
      await renameSession(id, name)
    } catch {
      // Ignore rename errors silently
    }
  }, [])

  // Navigate to session from notification toast
  const handleNavigateToSession = useCallback((sessionId: string) => {
    const allSessions = [...tree.terminals, ...tree.agents]
    const session = allSessions.find(s => s.id === sessionId)
    if (session) {
      handleSelectSession(session)
    }
  }, [tree, handleSelectSession])

  // Handle mobile tab changes
  const handleMobileTabChange = useCallback((tab: MobileTab) => {
    setMobileTab(tab)
    if (tab === 'sessions') {
      setSidebarVisible(true)
    } else {
      setSidebarVisible(false)
    }
    if (tab === 'settings') {
      setSettingsOpen(true)
    }
    if (tab === 'chat') {
      setAgentPanelVisible(true)
    }
  }, [])

  // Determine visibility based on mobile tab
  const showSidebar = isMobile ? sidebarVisible : true
  const showTerminal = isMobile ? mobileTab === 'terminal' : true
  const showChat = isMobile ? mobileTab === 'chat' : agentPanelVisible

  return (
    <div className="h-full w-full flex flex-col bg-[var(--bg-primary)]">
      {/* Top bar */}
      <TopBar
        connectionState={statusConnection}
        latency={latency}
        theme={theme}
        onToggleTheme={toggleTheme}
        onOpenSettings={() => setSettingsOpen(true)}
        onToggleSidebar={() => setSidebarVisible(prev => !prev)}
      />

      {/* Main content: three-column layout */}
      <div className="flex-1 flex min-h-0">
        {/* Sidebar */}
        <SessionList
          tree={tree}
          activeSessionId={activeSession?.id ?? null}
          onSelectSession={handleSelectSession}
          onCreateTerminal={handleCreateTerminal}
          onCreateAgent={handleCreateAgent}
          onDeleteSession={handleDeleteSession}
          onRenameSession={handleRenameSession}
          sidebarVisible={showSidebar}
        />

        {/* Mobile overlay backdrop when sidebar is open */}
        {isMobile && sidebarVisible && (
          <div
            className="fixed inset-0 z-30 bg-black/40"
            onClick={() => setSidebarVisible(false)}
          />
        )}

        {/* Terminal (center) */}
        {showTerminal && (
          <TerminalView
            activeSession={activeSession}
            openTabs={openTabs.map(s => ({ session: s }))}
            fontSize={fontSize}
            onSelectTab={handleSelectSession}
            onCloseTab={handleCloseTab}
          />
        )}

        {/* Agent chat (right) - hidden on mobile unless explicitly shown */}
        {showChat && (
          <AgentChat
            session={activeAgentSession}
            isVisible={true}
            onToggle={() => {
              setAgentPanelVisible(prev => !prev)
              if (isMobile) setMobileTab('terminal')
            }}
          />
        )}
      </div>

      {/* Status bar - hidden on mobile */}
      {!isMobile && (
        <StatusBar
          sessionCount={tree.terminals.length}
          agentCount={tree.agents.length}
          tunnelUrl={null}
        />
      )}

      {/* Mobile bottom navigation */}
      {isMobile && (
        <MobileNav
          activeTab={mobileTab}
          onTabChange={handleMobileTabChange}
        />
      )}

      {/* Notification toasts */}
      <NotificationToast onNavigateToSession={handleNavigateToSession} />

      {/* Overlays */}
      <CommandPalette
        isOpen={commandPaletteOpen}
        theme={theme}
        onClose={() => setCommandPaletteOpen(false)}
        onNewTerminal={handleCreateTerminal}
        onNewAgent={handleCreateAgent}
        onToggleTheme={toggleTheme}
      />

      <SettingsPanel
        isOpen={settingsOpen}
        theme={theme}
        fontSize={fontSize}
        onClose={() => setSettingsOpen(false)}
        onToggleTheme={toggleTheme}
        onSetFontSize={setFontSize}
      />
    </div>
  )
}
