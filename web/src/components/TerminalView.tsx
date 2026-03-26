import { useTerminal } from '../hooks/useTerminal'
import { X, Circle, Terminal } from 'lucide-react'
import type { Session } from '../lib/types'
import { SearchBar } from './SearchBar'

interface TerminalTab {
  session: Session
}

interface TerminalViewProps {
  activeSession: Session | null
  openTabs: TerminalTab[]
  fontSize: number
  onSelectTab: (session: Session) => void
  onCloseTab: (sessionId: string) => void
}

function TabItem({
  tab,
  isActive,
  onSelect,
  onClose,
}: {
  tab: TerminalTab
  isActive: boolean
  onSelect: () => void
  onClose: () => void
}) {
  return (
    <div
      className={`
        group flex items-center gap-1.5 h-7 px-3 cursor-pointer text-[11px] rounded-t-md transition-colors duration-150
        ${isActive
          ? 'bg-[var(--bg-primary)] border-t-2 border-[var(--accent-blue)] text-[var(--text-primary)]'
          : 'border-t-2 border-transparent text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-elevated)]'
        }
      `}
      onClick={onSelect}
    >
      <Circle
        size={7}
        className={tab.session.status === 'running' ? 'fill-[var(--accent-green)] text-[var(--accent-green)]' : 'fill-[var(--text-muted)] text-[var(--text-muted)]'}
      />
      <span className="truncate max-w-[120px]">
        {tab.session.name || `${tab.session.kind}-${tab.session.id.slice(0, 6)}`}
      </span>
      <button
        className="w-4 h-4 flex items-center justify-center rounded opacity-0 group-hover:opacity-100 hover:bg-[var(--bg-elevated)] transition-all duration-150 cursor-pointer"
        onClick={(e) => {
          e.stopPropagation()
          onClose()
        }}
      >
        <X size={10} />
      </button>
    </div>
  )
}

export function TerminalView({
  activeSession,
  openTabs,
  fontSize,
  onSelectTab,
  onCloseTab,
}: TerminalViewProps) {
  const { containerRef, connectionState, searchVisible, setSearchVisible, findNext, findPrevious } = useTerminal({
    sessionId: activeSession?.kind === 'terminal' ? activeSession.id : null,
    fontSize,
  })

  return (
    <div className="flex-1 flex flex-col min-w-0 bg-[var(--bg-primary)]">
      {/* Tab bar */}
      {openTabs.length > 0 && (
        <div className="h-9 bg-[var(--bg-secondary)] border-b border-[var(--border-color)] flex items-center px-1 gap-0.5 overflow-x-auto shrink-0">
          {openTabs.map(tab => (
            <TabItem
              key={tab.session.id}
              tab={tab}
              isActive={activeSession?.id === tab.session.id}
              onSelect={() => onSelectTab(tab.session)}
              onClose={() => onCloseTab(tab.session.id)}
            />
          ))}
          {/* Connection indicator */}
          {activeSession && (
            <div className="ml-auto px-3 text-[10px] font-mono text-[var(--text-muted)] flex items-center gap-1.5 shrink-0">
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  connectionState === 'connected' ? 'bg-[var(--accent-green)]' :
                  connectionState === 'connecting' ? 'bg-[var(--accent-amber)] animate-pulse-dot' :
                  'bg-[var(--text-muted)]'
                }`}
              />
              <span>{connectionState}</span>
            </div>
          )}
        </div>
      )}

      {/* Terminal area */}
      {activeSession?.kind === 'terminal' ? (
        <div className="flex-1 relative bg-[#0d1117]">
          <SearchBar
            isVisible={searchVisible}
            onClose={() => setSearchVisible(false)}
            onFindNext={findNext}
            onFindPrevious={findPrevious}
          />
          <div ref={containerRef} className="absolute inset-0 xterm-container" />
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-[var(--text-muted)]">
          <div className="text-center animate-fade-in">
            <Terminal size={40} className="mx-auto mb-3 opacity-20" />
            <p className="text-sm font-medium text-[var(--text-secondary)]">No terminal selected</p>
            <p className="text-xs mt-1 text-[var(--text-muted)]">Select a session from the sidebar or create a new one</p>
          </div>
        </div>
      )}
    </div>
  )
}
