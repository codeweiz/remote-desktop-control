import { useState } from 'react'
import { Sun, Moon, Settings, Wifi, WifiOff, QrCode, Menu, Hexagon } from 'lucide-react'
import type { ConnectionState, Theme } from '../lib/types'
import { QRCodeModal } from './QRCodeModal'

interface TopBarProps {
  connectionState: ConnectionState
  latency: number | null
  theme: Theme
  onToggleTheme: () => void
  onOpenSettings: () => void
  onToggleSidebar?: () => void
}

export function TopBar({
  connectionState,
  latency,
  theme,
  onToggleTheme,
  onOpenSettings,
  onToggleSidebar,
}: TopBarProps) {
  const isConnected = connectionState === 'connected'
  const [qrOpen, setQrOpen] = useState(false)

  return (
    <div className="h-10 flex items-center justify-between px-3 bg-[var(--bg-secondary)] border-b border-[var(--border-color)] shrink-0 select-none">
      {/* Left: Logo + Mobile hamburger */}
      <div className="flex items-center gap-2.5">
        {onToggleSidebar && (
          <button
            onClick={onToggleSidebar}
            className="md:hidden w-8 h-8 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
          >
            <Menu size={16} />
          </button>
        )}
        <div className="flex items-center gap-1.5">
          <Hexagon size={16} className="text-[var(--accent-green)]" />
          <span className="font-semibold text-sm text-[var(--text-primary)]">RTB</span>
          <span className="text-[10px] font-mono text-[var(--text-muted)]">2.0</span>
        </div>

        {/* Connection status */}
        <div className="flex items-center gap-1.5 ml-2 px-2 py-1 rounded-md bg-[var(--bg-primary)]">
          {isConnected ? (
            <>
              <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent-green)] animate-pulse-dot" />
              <span className="text-[11px] text-[var(--accent-green)] font-medium">Connected</span>
            </>
          ) : (
            <>
              <WifiOff size={12} className="text-[var(--accent-red)]" />
              <span className="text-[11px] text-[var(--accent-red)] font-medium">
                {connectionState === 'connecting' ? 'Connecting...' : 'Disconnected'}
              </span>
            </>
          )}
          {latency !== null && isConnected && (
            <span className="text-[11px] font-mono text-[var(--text-muted)] ml-0.5">{latency}ms</span>
          )}
        </div>
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-0.5">
        <button
          onClick={onToggleTheme}
          className="w-8 h-8 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer focus-visible:ring-2 focus-visible:ring-[var(--accent-blue)]/50"
          title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        >
          {theme === 'dark' ? <Sun size={15} /> : <Moon size={15} />}
        </button>
        <button
          onClick={() => setQrOpen(true)}
          className="w-8 h-8 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer focus-visible:ring-2 focus-visible:ring-[var(--accent-blue)]/50"
          title="QR Code"
        >
          <QrCode size={15} />
        </button>
        <button
          onClick={onOpenSettings}
          className="w-8 h-8 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer focus-visible:ring-2 focus-visible:ring-[var(--accent-blue)]/50"
          title="Settings"
        >
          <Settings size={15} />
        </button>
      </div>

      {/* QR Code Modal */}
      <QRCodeModal isOpen={qrOpen} onClose={() => setQrOpen(false)} />
    </div>
  )
}
