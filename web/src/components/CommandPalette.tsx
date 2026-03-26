import { useState, useEffect, useRef, useMemo, useCallback } from 'react'
import { Terminal, Bot, Sun, Moon, Search, ArrowRight } from 'lucide-react'
import type { Theme } from '../lib/types'

interface CommandAction {
  id: string
  label: string
  description?: string
  icon: React.ReactNode
  shortcut?: string
  action: () => void
}

interface CommandPaletteProps {
  isOpen: boolean
  theme: Theme
  onClose: () => void
  onNewTerminal: () => void
  onNewAgent: () => void
  onToggleTheme: () => void
}

export function CommandPalette({
  isOpen,
  theme,
  onClose,
  onNewTerminal,
  onNewAgent,
  onToggleTheme,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  const actions: CommandAction[] = useMemo(() => [
    {
      id: 'new-terminal',
      label: 'New Terminal',
      description: 'Create a new terminal session',
      icon: <Terminal size={16} />,
      action: () => { onNewTerminal(); onClose() },
    },
    {
      id: 'new-agent',
      label: 'New Agent',
      description: 'Create a new agent session',
      icon: <Bot size={16} />,
      action: () => { onNewAgent(); onClose() },
    },
    {
      id: 'toggle-theme',
      label: theme === 'dark' ? 'Switch to Light Mode' : 'Switch to Dark Mode',
      description: 'Toggle between dark and light themes',
      icon: theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />,
      action: () => { onToggleTheme(); onClose() },
    },
  ], [theme, onNewTerminal, onNewAgent, onToggleTheme, onClose])

  // Fuzzy filter
  const filtered = useMemo(() => {
    if (!query.trim()) return actions
    const q = query.toLowerCase()
    return actions.filter(a =>
      a.label.toLowerCase().includes(q) ||
      (a.description?.toLowerCase().includes(q))
    )
  }, [actions, query])

  // Reset selection when filtered list changes
  useEffect(() => {
    setSelectedIndex(0)
  }, [filtered.length])

  // Focus input when opened
  useEffect(() => {
    if (isOpen) {
      setQuery('')
      setSelectedIndex(0)
      requestAnimationFrame(() => {
        inputRef.current?.focus()
      })
    }
  }, [isOpen])

  // Keyboard handler
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setSelectedIndex(prev => (prev + 1) % Math.max(filtered.length, 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        setSelectedIndex(prev => (prev - 1 + filtered.length) % Math.max(filtered.length, 1))
        break
      case 'Enter':
        e.preventDefault()
        if (filtered[selectedIndex]) {
          filtered[selectedIndex].action()
        }
        break
      case 'Escape':
        e.preventDefault()
        onClose()
        break
    }
  }, [filtered, selectedIndex, onClose])

  if (!isOpen) return null

  return (
    <div
      className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-start justify-center pt-[20vh]"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-[500px] max-w-[90vw] bg-[var(--bg-secondary)] border border-[var(--border-color)] rounded-lg shadow-2xl overflow-hidden animate-fade-in">
        {/* Search input */}
        <div className="flex items-center gap-2 border-b border-[var(--border-color)]">
          <div className="pl-4">
            <Search size={16} className="text-[var(--text-muted)] shrink-0" />
          </div>
          <input
            ref={inputRef}
            type="text"
            className="flex-1 w-full bg-transparent px-2 py-3 text-sm text-[var(--text-primary)] outline-none placeholder:text-[var(--text-muted)]"
            placeholder="Type a command..."
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
          />
          <kbd className="text-[10px] font-mono bg-[var(--bg-primary)] px-1.5 py-0.5 rounded text-[var(--text-muted)] mr-3">
            ESC
          </kbd>
        </div>

        {/* Results */}
        <div className="max-h-[300px] overflow-y-auto py-1">
          {filtered.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-[var(--text-muted)]">
              No matching commands
            </div>
          ) : (
            filtered.map((action, index) => (
              <div
                key={action.id}
                className={`
                  flex items-center gap-3 px-4 py-2 text-sm cursor-pointer transition-colors duration-150
                  ${index === selectedIndex ? 'bg-[var(--bg-elevated)]' : 'hover:bg-[var(--bg-elevated)]'}
                `}
                onClick={() => action.action()}
                onMouseEnter={() => setSelectedIndex(index)}
              >
                <span className="text-[var(--text-muted)]">{action.icon}</span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-[var(--text-primary)]">{action.label}</div>
                  {action.description && (
                    <div className="text-[11px] text-[var(--text-muted)] truncate">{action.description}</div>
                  )}
                </div>
                {action.shortcut && (
                  <span className="text-[10px] font-mono bg-[var(--bg-primary)] px-1.5 py-0.5 rounded text-[var(--text-muted)]">
                    {action.shortcut}
                  </span>
                )}
                {index === selectedIndex && (
                  <ArrowRight size={14} className="text-[var(--text-muted)] shrink-0" />
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
