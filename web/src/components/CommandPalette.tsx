import { useState, useEffect, useRef, useMemo, useCallback } from 'react'
import {
  Dialog,
  Box,
  Typography,
  InputBase,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Backdrop,
} from '@mui/material'
import {
  Terminal as TerminalIcon,
  Search as SearchIcon,
  ArrowForward as ArrowRightIcon,
  Keyboard as KeyboardIcon,
} from '@mui/icons-material'

interface CommandAction {
  id: string
  label: string
  description?: string
  icon: React.ReactNode
  shortcut?: string
  action: () => void
}

interface CommandPaletteProps {
  open: boolean
  onClose: () => void
  onNewTerminal: () => void
}

export function CommandPalette({
  open,
  onClose,
  onNewTerminal,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  const actions: CommandAction[] = useMemo(() => [
    {
      id: 'new-terminal',
      label: 'New Terminal',
      description: 'Create a new terminal session',
      icon: <TerminalIcon sx={{ fontSize: 18 }} />,
      action: () => { onNewTerminal(); onClose() },
    },
  ], [onNewTerminal, onClose])

  // Fuzzy filter
  const filtered = useMemo(() => {
    if (!query.trim()) return actions
    const q = query.toLowerCase()
    return actions.filter(a =>
      a.label.toLowerCase().includes(q) ||
      (a.description?.toLowerCase().includes(q))
    )
  }, [actions, query])

  useEffect(() => {
    setSelectedIndex(0)
  }, [filtered.length])

  useEffect(() => {
    if (open) {
      setQuery('')
      setSelectedIndex(0)
      requestAnimationFrame(() => {
        inputRef.current?.focus()
      })
    }
  }, [open])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setSelectedIndex(prev => (prev + 1) % Math.max(filtered.length, 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        setSelectedIndex(prev => (prev - 1 + filtered.length) % Math.max(filtered.length, 1))
        break
      case 'Enter':
        e.preventDefault()
        if (filtered[selectedIndex]) {
          filtered[selectedIndex].action()
        }
        break
      case 'Escape':
        e.preventDefault()
        onClose()
        break
    }
  }, [filtered, selectedIndex, onClose])

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="sm"
      fullWidth
      slots={{ backdrop: Backdrop }}
      slotProps={{
        backdrop: {
          sx: { backdropFilter: 'blur(8px)', bgcolor: 'rgba(0,0,0,0.5)' },
        },
      }}
      PaperProps={{
        sx: {
          position: 'absolute',
          top: '20vh',
          m: 0,
          borderRadius: 2,
          maxWidth: 500,
          overflow: 'hidden',
        },
      }}
    >
      {/* Search input */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 1.5,
          borderBottom: '1px solid',
          borderColor: 'divider',
          px: 2,
        }}
      >
        <SearchIcon sx={{ fontSize: 18, color: 'text.secondary', flexShrink: 0 }} />
        <InputBase
          inputRef={inputRef}
          fullWidth
          placeholder="Type a command..."
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          sx={{
            py: 1.5,
            fontSize: 14,
          }}
        />
        <Box
          component="kbd"
          sx={{
            fontSize: 10,
            fontFamily: "'JetBrains Mono', monospace",
            bgcolor: 'rgba(255,255,255,0.05)',
            px: 1,
            py: 0.25,
            borderRadius: 0.5,
            color: 'text.secondary',
            flexShrink: 0,
          }}
        >
          ESC
        </Box>
      </Box>

      {/* Results */}
      <List sx={{ maxHeight: 300, overflow: 'auto', py: 0.5 }}>
        {filtered.length === 0 ? (
          <Box sx={{ px: 3, py: 4, textAlign: 'center' }}>
            <Typography variant="body2" sx={{ color: 'text.secondary' }}>
              No matching commands
            </Typography>
          </Box>
        ) : (
          filtered.map((action, index) => (
            <ListItemButton
              key={action.id}
              selected={index === selectedIndex}
              onClick={() => action.action()}
              onMouseEnter={() => setSelectedIndex(index)}
              sx={{
                py: 1,
                px: 2,
                '&.Mui-selected': {
                  bgcolor: 'rgba(255,255,255,0.05)',
                },
              }}
            >
              <ListItemIcon sx={{ minWidth: 36, color: 'text.secondary' }}>
                {action.icon}
              </ListItemIcon>
              <ListItemText
                primary={action.label}
                secondary={action.description}
                primaryTypographyProps={{ fontSize: 13, fontWeight: 500 }}
                secondaryTypographyProps={{ fontSize: 11 }}
              />
              {action.shortcut && (
                <Box
                  component="kbd"
                  sx={{
                    fontSize: 10,
                    fontFamily: "'JetBrains Mono', monospace",
                    bgcolor: 'rgba(255,255,255,0.05)',
                    px: 1,
                    py: 0.25,
                    borderRadius: 0.5,
                    color: 'text.secondary',
                    ml: 1,
                  }}
                >
                  {action.shortcut}
                </Box>
              )}
              {index === selectedIndex && (
                <ArrowRightIcon sx={{ fontSize: 16, color: 'text.secondary', ml: 1 }} />
              )}
            </ListItemButton>
          ))
        )}
      </List>
    </Dialog>
  )
}
