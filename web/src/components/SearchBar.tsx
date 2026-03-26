import { useState, useRef, useEffect, useCallback } from 'react'
import { X, ChevronUp, ChevronDown, Search } from 'lucide-react'

interface SearchBarProps {
  isVisible: boolean
  onClose: () => void
  onFindNext: (term: string) => void
  onFindPrevious: (term: string) => void
}

export function SearchBar({ isVisible, onClose, onFindNext, onFindPrevious }: SearchBarProps) {
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isVisible) {
      requestAnimationFrame(() => {
        inputRef.current?.focus()
        inputRef.current?.select()
      })
    }
  }, [isVisible])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (e.shiftKey) {
        onFindPrevious(query)
      } else {
        onFindNext(query)
      }
    }
  }, [query, onClose, onFindNext, onFindPrevious])

  const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value
    setQuery(val)
    if (val) {
      onFindNext(val)
    }
  }, [onFindNext])

  if (!isVisible) return null

  return (
    <div className="absolute top-0 right-0 z-20 flex items-center gap-1 bg-[var(--bg-secondary)] border border-[var(--border-color)] rounded-bl-md px-2 py-1.5 shadow-lg animate-slide-in">
      <Search size={12} className="text-[var(--text-muted)] shrink-0" />
      <input
        ref={inputRef}
        type="text"
        className="w-48 bg-[var(--bg-elevated)] text-xs text-[var(--text-primary)] rounded-md px-2 py-1 outline-none border border-transparent focus:border-[var(--accent-blue)] transition-colors duration-150 placeholder:text-[var(--text-muted)]"
        placeholder="Search terminal..."
        value={query}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
      />
      <button
        className="p-0.5 rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
        onClick={() => onFindPrevious(query)}
        title="Previous (Shift+Enter)"
      >
        <ChevronUp size={14} />
      </button>
      <button
        className="p-0.5 rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
        onClick={() => onFindNext(query)}
        title="Next (Enter)"
      >
        <ChevronDown size={14} />
      </button>
      <button
        className="p-0.5 rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
        onClick={onClose}
        title="Close (Esc)"
      >
        <X size={14} />
      </button>
    </div>
  )
}
