import { Sun, Moon, Settings, Wifi, WifiOff } from 'lucide-react'
import type { ConnectionState, Theme } from '../lib/types'

interface TopBarProps {
  connectionState: ConnectionState
  latency: number | null
  theme: Theme
  onToggleTheme: () => void
  onOpenSettings: () => void
}

export function TopBar({
  connectionState,
  latency,
  theme,
  onToggleTheme,
  onOpenSettings,
}: TopBarProps) {
  const isConnected = connectionState === 'connected'

  return (
    <div className="h-10 flex items-center justify-between px-4 bg-bg-secondary border-b border-border shrink-0">
      {/* Left: Logo */}
      <div className="flex items-center gap-3">
        <span className="font-bold text-sm tracking-wider text-accent-green">RTB</span>
        <span className="text-xs text-text-secondary">2.0</span>
      </div>

      {/* Center: Connection status */}
      <div className="flex items-center gap-2 text-xs">
        {isConnected ? (
          <>
            <Wifi size={14} className="text-accent-green" />
            <span className="text-accent-green">Connected</span>
          </>
        ) : (
          <>
            <WifiOff size={14} className="text-accent-red" />
            <span className="text-accent-red">
              {connectionState === 'connecting' ? 'Connecting...' : 'Disconnected'}
            </span>
          </>
        )}
        {latency !== null && isConnected && (
          <span className="text-text-secondary ml-1">{latency}ms</span>
        )}
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-1">
        <button
          onClick={onToggleTheme}
          className="p-1.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
          title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        >
          {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        </button>
        <button
          onClick={onOpenSettings}
          className="p-1.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
          title="Settings"
        >
          <Settings size={16} />
        </button>
      </div>
    </div>
  )
}
