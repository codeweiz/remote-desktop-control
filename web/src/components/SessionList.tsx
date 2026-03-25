import { Plus, Terminal, Bot, Trash2, ChevronRight } from 'lucide-react'
import type { Session } from '../lib/types'
import type { SessionTree } from '../hooks/useSessions'

interface SessionListProps {
  tree: SessionTree
  activeSessionId: string | null
  onSelectSession: (session: Session) => void
  onCreateTerminal: () => void
  onCreateAgent: () => void
  onDeleteSession: (id: string) => void
}

function StatusDot({ status, kind }: { status: string; kind: string }) {
  let colorClass = 'bg-gray-500'
  if (kind === 'agent') {
    colorClass = 'bg-accent-purple'
  } else if (status === 'running') {
    colorClass = 'bg-accent-green'
  } else if (status === 'exited') {
    colorClass = 'bg-gray-500'
  } else if (status === 'error') {
    colorClass = 'bg-accent-red'
  }

  return (
    <span className={`w-2 h-2 rounded-full ${colorClass} shrink-0`} />
  )
}

function SessionItem({
  session,
  isActive,
  onSelect,
  onDelete,
}: {
  session: Session
  isActive: boolean
  onSelect: () => void
  onDelete: () => void
}) {
  return (
    <div
      className={`
        group flex items-center gap-2 px-3 py-1.5 cursor-pointer text-xs
        hover:bg-bg-tertiary transition-colors
        ${isActive ? 'bg-bg-tertiary border-l-2 border-accent-blue' : 'border-l-2 border-transparent'}
      `}
      onClick={onSelect}
    >
      <StatusDot status={session.status} kind={session.kind} />
      <span className="truncate flex-1 text-text-primary">
        {session.name || `${session.kind}-${session.id.slice(0, 6)}`}
      </span>
      <button
        className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-bg-secondary text-text-secondary hover:text-accent-red transition-all"
        onClick={(e) => {
          e.stopPropagation()
          onDelete()
        }}
        title="Delete session"
      >
        <Trash2 size={12} />
      </button>
    </div>
  )
}

export function SessionList({
  tree,
  activeSessionId,
  onSelectSession,
  onCreateTerminal,
  onCreateAgent,
  onDeleteSession,
}: SessionListProps) {
  return (
    <div className="w-[220px] bg-bg-secondary border-r border-border flex flex-col shrink-0 overflow-hidden">
      {/* Terminals section */}
      <div className="flex-1 overflow-y-auto">
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-text-secondary flex items-center gap-1">
            <Terminal size={11} />
            Terminals
          </span>
          <button
            className="p-0.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-accent-green transition-colors"
            onClick={onCreateTerminal}
            title="New terminal"
          >
            <Plus size={14} />
          </button>
        </div>
        {tree.terminals.length === 0 ? (
          <div className="px-3 py-2 text-xs text-text-secondary italic">
            No terminals
          </div>
        ) : (
          tree.terminals.map(session => (
            <SessionItem
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onSelect={() => onSelectSession(session)}
              onDelete={() => onDeleteSession(session.id)}
            />
          ))
        )}

        {/* Agents section */}
        <div className="flex items-center justify-between px-3 py-2 mt-2">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-text-secondary flex items-center gap-1">
            <Bot size={11} />
            Agents
          </span>
          <button
            className="p-0.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-accent-purple transition-colors"
            onClick={onCreateAgent}
            title="New agent"
          >
            <Plus size={14} />
          </button>
        </div>
        {tree.agents.length === 0 ? (
          <div className="px-3 py-2 text-xs text-text-secondary italic">
            No agents
          </div>
        ) : (
          tree.agents.map(session => (
            <SessionItem
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onSelect={() => onSelectSession(session)}
              onDelete={() => onDeleteSession(session.id)}
            />
          ))
        )}
      </div>

      {/* Footer hint */}
      <div className="px-3 py-2 border-t border-border">
        <div className="flex items-center gap-1 text-[10px] text-text-secondary">
          <ChevronRight size={10} />
          <span>Click to open session</span>
        </div>
      </div>
    </div>
  )
}
