import { useTerminal } from '../hooks/useTerminal'
import { X, Circle } from 'lucide-react'
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
        group flex items-center gap-1.5 px-3 py-1.5 cursor-pointer text-xs border-b-2 transition-colors
        ${isActive
          ? 'border-accent-blue bg-bg-primary text-text-primary'
          : 'border-transparent text-text-secondary hover:text-text-primary hover:bg-bg-tertiary'
        }
      `}
      onClick={onSelect}
    >
      <Circle
        size={8}
        className={tab.session.status === 'running' ? 'fill-accent-green text-accent-green' : 'fill-gray-500 text-gray-500'}
      />
      <span className="truncate max-w-[120px]">
        {tab.session.name || `${tab.session.kind}-${tab.session.id.slice(0, 6)}`}
      </span>
      <button
        className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-bg-secondary transition-all"
        onClick={(e) => {
          e.stopPropagation()
          onClose()
        }}
      >
        <X size={12} />
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
    <div className="flex-1 flex flex-col min-w-0 bg-bg-primary">
      {/* Tab bar */}
      {openTabs.length > 0 && (
        <div className="flex items-center bg-bg-secondary border-b border-border overflow-x-auto shrink-0">
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
            <div className="ml-auto px-3 text-[10px] text-text-secondary flex items-center gap-1">
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  connectionState === 'connected' ? 'bg-accent-green' :
                  connectionState === 'connecting' ? 'bg-accent-orange animate-pulse' :
                  'bg-gray-500'
                }`}
              />
              {connectionState}
            </div>
          )}
        </div>
      )}

      {/* Terminal area */}
      {activeSession?.kind === 'terminal' ? (
        <div className="flex-1 relative">
          <SearchBar
            isVisible={searchVisible}
            onClose={() => setSearchVisible(false)}
            onFindNext={findNext}
            onFindPrevious={findPrevious}
          />
          <div ref={containerRef} className="absolute inset-0 xterm-container" />
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-text-secondary">
          <div className="text-center">
            <div className="text-4xl mb-3 opacity-20">{'>'}_</div>
            <p className="text-sm">No terminal selected</p>
            <p className="text-xs mt-1">Select a session from the sidebar or create a new one</p>
          </div>
        </div>
      )}
    </div>
  )
}
