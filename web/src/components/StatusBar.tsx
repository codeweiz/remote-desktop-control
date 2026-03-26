import { Terminal, Bot, Globe, Command } from 'lucide-react'

interface StatusBarProps {
  sessionCount: number
  agentCount: number
  tunnelUrl: string | null
}

export function StatusBar({ sessionCount, agentCount, tunnelUrl }: StatusBarProps) {
  return (
    <div className="h-7 flex items-center justify-between px-4 bg-[var(--bg-secondary)] border-t border-[var(--border-color)] text-[11px] text-[var(--text-muted)] font-mono shrink-0 select-none">
      {/* Left: Counts */}
      <div className="flex items-center gap-4">
        <span className="flex items-center gap-1.5 hover:text-[var(--text-secondary)] cursor-pointer transition-colors duration-150">
          <Terminal size={11} />
          {sessionCount} session{sessionCount !== 1 ? 's' : ''}
        </span>
        <span className="flex items-center gap-1.5 hover:text-[var(--text-secondary)] cursor-pointer transition-colors duration-150">
          <Bot size={11} />
          {agentCount} agent{agentCount !== 1 ? 's' : ''}
        </span>
        {tunnelUrl && (
          <span className="flex items-center gap-1.5 hover:text-[var(--text-secondary)] cursor-pointer transition-colors duration-150">
            <Globe size={11} />
            <a
              href={tunnelUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-[var(--accent-blue)] transition-colors duration-150"
            >
              {tunnelUrl.replace(/^https?:\/\//, '')}
            </a>
          </span>
        )}
      </div>

      {/* Right: Keyboard shortcut hint */}
      <div className="flex items-center gap-1 hover:text-[var(--text-secondary)] cursor-pointer transition-colors duration-150">
        <Command size={11} />
        <span>K</span>
      </div>
    </div>
  )
}
