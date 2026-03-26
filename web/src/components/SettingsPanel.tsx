import { X, Sun, Moon, Monitor } from 'lucide-react'
import type { Theme } from '../lib/types'

interface SettingsPanelProps {
  isOpen: boolean
  theme: Theme
  fontSize: number
  onClose: () => void
  onToggleTheme: () => void
  onSetFontSize: (size: number) => void
}

export function SettingsPanel({
  isOpen,
  theme,
  fontSize,
  onClose,
  onToggleTheme,
  onSetFontSize,
}: SettingsPanelProps) {
  if (!isOpen) return null

  return (
    <div
      className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-[420px] max-w-[90vw] bg-[var(--bg-secondary)] border border-[var(--border-color)] rounded-lg shadow-2xl overflow-hidden animate-fade-in">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-color)]">
          <h2 className="text-sm font-semibold text-[var(--text-primary)]">Settings</h2>
          <button
            onClick={onClose}
            className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-6">
          {/* Appearance */}
          <div>
            <h3 className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] mb-3">
              Appearance
            </h3>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Monitor size={14} className="text-[var(--text-muted)]" />
                <span className="text-sm text-[var(--text-primary)]">Theme</span>
              </div>
              <button
                onClick={onToggleTheme}
                className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-[var(--bg-elevated)] border border-[var(--border-color)] hover:border-[var(--text-muted)] transition-colors duration-150 cursor-pointer"
              >
                {theme === 'dark' ? (
                  <>
                    <Moon size={14} className="text-[var(--accent-blue)]" />
                    <span className="text-xs text-[var(--text-primary)]">Dark</span>
                  </>
                ) : (
                  <>
                    <Sun size={14} className="text-[var(--accent-amber)]" />
                    <span className="text-xs text-[var(--text-primary)]">Light</span>
                  </>
                )}
              </button>
            </div>
          </div>

          {/* Terminal */}
          <div>
            <h3 className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] mb-3">
              Terminal
            </h3>
            <div className="flex items-center justify-between">
              <span className="text-sm text-[var(--text-primary)]">Font Size</span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => onSetFontSize(Math.max(8, fontSize - 1))}
                  className="w-7 h-7 rounded-md bg-[var(--bg-elevated)] border border-[var(--border-color)] hover:border-[var(--text-muted)] flex items-center justify-center text-xs text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
                >
                  -
                </button>
                <span className="text-sm font-mono text-[var(--text-primary)] w-8 text-center">{fontSize}</span>
                <button
                  onClick={() => onSetFontSize(Math.min(24, fontSize + 1))}
                  className="w-7 h-7 rounded-md bg-[var(--bg-elevated)] border border-[var(--border-color)] hover:border-[var(--text-muted)] flex items-center justify-center text-xs text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {/* About */}
          <div>
            <h3 className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] mb-3">
              About
            </h3>
            <div className="text-xs text-[var(--text-muted)] space-y-1">
              <p>RTB 2.0 - Remote Terminal Bridge</p>
              <p className="font-mono text-[11px]">React 19 + xterm.js + Tailwind CSS</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
