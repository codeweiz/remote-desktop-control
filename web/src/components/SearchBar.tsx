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
    <div className="absolute top-0 right-0 z-20 flex items-center gap-1 bg-bg-secondary border border-border rounded-bl-lg px-2 py-1 shadow-lg">
      <Search size={12} className="text-text-secondary shrink-0" />
      <input
        ref={inputRef}
        type="text"
        className="w-48 bg-bg-tertiary text-xs text-text-primary rounded px-2 py-1 outline-none border border-transparent focus:border-accent-blue transition-colors placeholder-text-secondary"
        placeholder="Search terminal..."
        value={query}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
      />
      <button
        className="p-0.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
        onClick={() => onFindPrevious(query)}
        title="Previous (Shift+Enter)"
      >
        <ChevronUp size={14} />
      </button>
      <button
        className="p-0.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
        onClick={() => onFindNext(query)}
        title="Next (Enter)"
      >
        <ChevronDown size={14} />
      </button>
      <button
        className="p-0.5 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
        onClick={onClose}
        title="Close (Esc)"
      >
        <X size={14} />
      </button>
    </div>
  )
}
