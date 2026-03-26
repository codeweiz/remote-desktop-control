import { useState, useRef, useEffect, useCallback } from 'react'
import { Box, Paper, IconButton, InputBase } from '@mui/material'
import {
  Close as CloseIcon,
  KeyboardArrowUp as ChevronUpIcon,
  KeyboardArrowDown as ChevronDownIcon,
  Search as SearchIcon,
} from '@mui/icons-material'

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
    <Paper
      elevation={4}
      sx={{
        position: 'absolute',
        top: 0,
        right: 0,
        zIndex: 20,
        display: 'flex',
        alignItems: 'center',
        gap: 0.5,
        px: 1,
        py: 0.75,
        borderBottomLeftRadius: 8,
        borderTopLeftRadius: 0,
        borderTopRightRadius: 0,
        borderBottomRightRadius: 0,
      }}
      className="animate-slide-in"
    >
      <SearchIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
      <InputBase
        inputRef={inputRef}
        size="small"
        placeholder="Search terminal..."
        value={query}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        sx={{
          fontSize: 12,
          width: 180,
          '& input': { py: 0.25, px: 0.5 },
        }}
      />
      <IconButton size="small" onClick={() => onFindPrevious(query)} title="Previous (Shift+Enter)" sx={{ p: 0.25 }}>
        <ChevronUpIcon sx={{ fontSize: 16 }} />
      </IconButton>
      <IconButton size="small" onClick={() => onFindNext(query)} title="Next (Enter)" sx={{ p: 0.25 }}>
        <ChevronDownIcon sx={{ fontSize: 16 }} />
      </IconButton>
      <IconButton size="small" onClick={onClose} title="Close (Esc)" sx={{ p: 0.25 }}>
        <CloseIcon sx={{ fontSize: 16 }} />
      </IconButton>
    </Paper>
  )
}
