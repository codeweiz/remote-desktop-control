import { Plus, Terminal, Bot, Trash2, Edit3, Copy, GitBranch } from 'lucide-react'
import type { Session } from '../lib/types'
import type { SessionTree } from '../hooks/useSessions'
import { ContextMenu, useContextMenu } from './ContextMenu'
import type { ContextMenuItem } from './ContextMenu'
import { TaskPool } from './TaskPool'

interface SessionListProps {
  tree: SessionTree
  activeSessionId: string | null
  onSelectSession: (session: Session) => void
  onCreateTerminal: () => void
  onCreateAgent: () => void
  onDeleteSession: (id: string) => void
  onRenameSession?: (id: string, name: string) => void
  sidebarVisible?: boolean
}

function StatusDot({ status, kind }: { status: string; kind: string }) {
  const isAgent = kind === 'agent'

  if (isAgent) {
    return (
      <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent-purple)] shrink-0 animate-pulse-dot" />
    )
  }

  let colorClass = 'bg-[var(--text-muted)]'
  let animate = ''
  if (status === 'running') {
    colorClass = 'bg-[var(--accent-green)]'
    animate = 'animate-pulse-dot'
  } else if (status === 'error') {
    colorClass = 'bg-[var(--accent-red)]'
  } else if (status === 'idle') {
    colorClass = 'bg-[var(--accent-amber)]'
  }

  return (
    <span className={`w-1.5 h-1.5 rounded-full ${colorClass} ${animate} shrink-0`} />
  )
}

function SessionItem({
  session,
  isActive,
  onSelect,
  onDelete,
  onContextMenu,
}: {
  session: Session
  isActive: boolean
  onSelect: () => void
  onDelete: () => void
  onContextMenu: (e: React.MouseEvent) => void
}) {
  const isAgent = session.kind === 'agent'
  const borderColor = isAgent ? 'border-[var(--accent-purple)]' : 'border-[var(--accent-green)]'

  return (
    <div
      className={`
        group flex items-center gap-2 px-3 py-1.5 cursor-pointer text-sm
        hover:bg-[var(--bg-hover)] rounded-md mx-1 transition-colors duration-150
        ${isActive ? `bg-[var(--bg-elevated)] border-l-2 ${borderColor}` : 'border-l-2 border-transparent'}
      `}
      onClick={onSelect}
      onContextMenu={onContextMenu}
    >
      <StatusDot status={session.status} kind={session.kind} />
      <span className="truncate flex-1 text-[var(--text-primary)] text-sm">
        {session.name || `${session.kind}-${session.id.slice(0, 6)}`}
      </span>
      <button
        className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[var(--bg-secondary)] text-[var(--text-muted)] hover:text-[var(--accent-red)] transition-all duration-150 cursor-pointer"
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
  onRenameSession,
  sidebarVisible = true,
}: SessionListProps) {
  const { menuState, showMenu, closeMenu } = useContextMenu()

  const buildContextItems = (session: Session): ContextMenuItem[] => [
    {
      id: 'rename',
      label: 'Rename',
      icon: <Edit3 size={12} />,
      action: () => {
        const newName = prompt('Enter new name:', session.name || '')
        if (newName !== null && newName !== session.name && onRenameSession) {
          onRenameSession(session.id, newName)
        }
      },
    },
    {
      id: 'copy-id',
      label: 'Copy ID',
      icon: <Copy size={12} />,
      action: () => {
        navigator.clipboard.writeText(session.id).catch(() => {
          // Clipboard API not available
        })
      },
    },
    {
      id: 'fork',
      label: 'Fork Session',
      icon: <GitBranch size={12} />,
      disabled: true,
      action: () => {},
    },
    {
      id: 'delete',
      label: 'Delete',
      icon: <Trash2 size={12} />,
      danger: true,
      action: () => onDeleteSession(session.id),
    },
  ]

  if (!sidebarVisible) return null

  return (
    <div className="w-[240px] bg-[var(--bg-secondary)] border-r border-[var(--border-color)] flex flex-col shrink-0 overflow-hidden max-md:absolute max-md:inset-y-0 max-md:left-0 max-md:z-40 max-md:w-[260px] max-md:shadow-2xl">
      {/* Terminals section */}
      <div className="flex-1 overflow-y-auto">
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] flex items-center gap-1.5">
            <Terminal size={11} />
            Terminals
          </span>
          <button
            className="p-0.5 rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--accent-green)] transition-colors duration-150 cursor-pointer"
            onClick={onCreateTerminal}
            title="New terminal"
          >
            <Plus size={14} />
          </button>
        </div>
        {tree.terminals.length === 0 ? (
          <div className="px-3 py-2 text-xs text-[var(--text-muted)] italic">
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
              onContextMenu={(e) => showMenu(e, buildContextItems(session))}
            />
          ))
        )}

        {/* Agents section */}
        <div className="flex items-center justify-between px-3 py-2 mt-3">
          <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] flex items-center gap-1.5">
            <Bot size={11} />
            Agents
          </span>
          <button
            className="p-0.5 rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--accent-purple)] transition-colors duration-150 cursor-pointer"
            onClick={onCreateAgent}
            title="New agent"
          >
            <Plus size={14} />
          </button>
        </div>
        {tree.agents.length === 0 ? (
          <div className="px-3 py-2 text-xs text-[var(--text-muted)] italic">
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
              onContextMenu={(e) => showMenu(e, buildContextItems(session))}
            />
          ))
        )}
      </div>

      {/* Task Pool */}
      <TaskPool />

      {/* Create buttons at bottom */}
      <div className="px-2 pb-2 pt-1 border-t border-[var(--border-color)] space-y-1.5">
        <button
          onClick={onCreateTerminal}
          className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] border border-dashed border-[var(--border-color)] rounded-md py-1.5 w-full hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer flex items-center justify-center gap-1.5"
        >
          <Plus size={12} />
          New Terminal
        </button>
        <button
          onClick={onCreateAgent}
          className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] border border-dashed border-[var(--border-color)] rounded-md py-1.5 w-full hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer flex items-center justify-center gap-1.5"
        >
          <Plus size={12} />
          New Agent
        </button>
      </div>

      {/* Context menu overlay */}
      <ContextMenu state={menuState} onClose={closeMenu} />
    </div>
  )
}
