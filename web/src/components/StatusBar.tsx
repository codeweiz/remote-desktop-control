import { Terminal, Bot, Globe, Command } from 'lucide-react'

interface StatusBarProps {
  sessionCount: number
  agentCount: number
  tunnelUrl: string | null
}

export function StatusBar({ sessionCount, agentCount, tunnelUrl }: StatusBarProps) {
  return (
    <div className="h-6 flex items-center justify-between px-4 bg-bg-secondary border-t border-border text-[11px] text-text-secondary shrink-0">
      {/* Left: Counts */}
      <div className="flex items-center gap-3">
        <span className="flex items-center gap-1">
          <Terminal size={11} />
          {sessionCount} session{sessionCount !== 1 ? 's' : ''}
        </span>
        <span className="flex items-center gap-1">
          <Bot size={11} />
          {agentCount} agent{agentCount !== 1 ? 's' : ''}
        </span>
        {tunnelUrl && (
          <span className="flex items-center gap-1">
            <Globe size={11} />
            <a
              href={tunnelUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-accent-blue transition-colors"
            >
              {tunnelUrl.replace(/^https?:\/\//, '')}
            </a>
          </span>
        )}
      </div>

      {/* Right: Keyboard shortcut hint */}
      <div className="flex items-center gap-1 opacity-60">
        <Command size={11} />
        <span>K</span>
      </div>
    </div>
  )
}
