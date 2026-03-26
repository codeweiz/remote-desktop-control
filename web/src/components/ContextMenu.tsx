import { useState, useEffect, useRef, useCallback } from 'react'

export interface ContextMenuItem {
  id: string
  label: string
  icon?: React.ReactNode
  danger?: boolean
  disabled?: boolean
  action: () => void
}

interface ContextMenuState {
  x: number
  y: number
  items: ContextMenuItem[]
}

interface ContextMenuProps {
  state: ContextMenuState | null
  onClose: () => void
}

export function ContextMenu({ state, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!state) return

    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      }
    }

    // Delay adding the click listener so the contextmenu event itself doesn't close it
    requestAnimationFrame(() => {
      document.addEventListener('mousedown', handleClickOutside)
      document.addEventListener('keydown', handleEscape)
    })

    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [state, onClose])

  if (!state) return null

  // Adjust position to stay in viewport
  const adjustedX = Math.min(state.x, window.innerWidth - 180)
  const adjustedY = Math.min(state.y, window.innerHeight - state.items.length * 32 - 8)

  return (
    <div
      ref={menuRef}
      className="fixed z-[100] bg-bg-secondary border border-border rounded-lg shadow-2xl py-1 min-w-[160px]"
      style={{ left: adjustedX, top: adjustedY }}
    >
      {state.items.map(item => (
        <button
          key={item.id}
          className={`
            w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors text-left
            ${item.disabled
              ? 'text-text-secondary/50 cursor-not-allowed'
              : item.danger
                ? 'text-accent-red hover:bg-accent-red/10'
                : 'text-text-primary hover:bg-bg-tertiary'
            }
          `}
          onClick={() => {
            if (!item.disabled) {
              item.action()
              onClose()
            }
          }}
          disabled={item.disabled}
        >
          {item.icon && <span className="shrink-0">{item.icon}</span>}
          <span>{item.label}</span>
        </button>
      ))}
    </div>
  )
}

/** Hook to manage context menu state */
export function useContextMenu() {
  const [menuState, setMenuState] = useState<ContextMenuState | null>(null)

  const showMenu = useCallback((e: React.MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault()
    e.stopPropagation()
    setMenuState({ x: e.clientX, y: e.clientY, items })
  }, [])

  const closeMenu = useCallback(() => {
    setMenuState(null)
  }, [])

  return { menuState, showMenu, closeMenu }
}
