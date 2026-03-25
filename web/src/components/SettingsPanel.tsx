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
      className="fixed inset-0 z-50 command-palette-overlay flex items-center justify-center"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-[420px] max-w-[90vw] bg-bg-secondary border border-border rounded-xl shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 className="text-sm font-semibold text-text-primary">Settings</h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-6">
          {/* Appearance */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-text-secondary mb-3">
              Appearance
            </h3>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Monitor size={14} className="text-text-secondary" />
                <span className="text-sm text-text-primary">Theme</span>
              </div>
              <button
                onClick={onToggleTheme}
                className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-tertiary border border-border hover:border-text-secondary transition-colors"
              >
                {theme === 'dark' ? (
                  <>
                    <Moon size={14} className="text-accent-blue" />
                    <span className="text-xs text-text-primary">Dark</span>
                  </>
                ) : (
                  <>
                    <Sun size={14} className="text-accent-orange" />
                    <span className="text-xs text-text-primary">Light</span>
                  </>
                )}
              </button>
            </div>
          </div>

          {/* Terminal */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-text-secondary mb-3">
              Terminal
            </h3>
            <div className="flex items-center justify-between">
              <span className="text-sm text-text-primary">Font Size</span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => onSetFontSize(Math.max(8, fontSize - 1))}
                  className="w-7 h-7 rounded bg-bg-tertiary border border-border hover:border-text-secondary flex items-center justify-center text-xs text-text-primary transition-colors"
                >
                  -
                </button>
                <span className="text-sm text-text-primary w-8 text-center">{fontSize}</span>
                <button
                  onClick={() => onSetFontSize(Math.min(24, fontSize + 1))}
                  className="w-7 h-7 rounded bg-bg-tertiary border border-border hover:border-text-secondary flex items-center justify-center text-xs text-text-primary transition-colors"
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {/* About */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-text-secondary mb-3">
              About
            </h3>
            <div className="text-xs text-text-secondary space-y-1">
              <p>RTB 2.0 - Remote Terminal Bridge</p>
              <p>React 19 + xterm.js + Tailwind CSS</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
