import React, { useRef, useState } from 'react'
import { Box, IconButton, TextField } from '@mui/material'
import KeyboardReturnIcon from '@mui/icons-material/KeyboardReturn'

interface MobileInputBarProps {
  onSend: (data: string) => void
}

const SPECIAL_KEYS = [
  { label: 'Esc', data: '\x1b' },
  { label: 'Tab', data: '\t' },
  { label: 'Ctrl', data: null }, // modifier toggle
  { label: '\u2191', data: '\x1b[A' }, // Up
  { label: '\u2193', data: '\x1b[B' }, // Down
  { label: '\u2190', data: '\x1b[D' }, // Left
  { label: '\u2192', data: '\x1b[C' }, // Right
]

export default function MobileInputBar({ onSend }: MobileInputBarProps) {
  const [ctrlActive, setCtrlActive] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleKey = (data: string | null) => {
    if (data === null) {
      // Toggle Ctrl modifier
      setCtrlActive(prev => !prev)
      return
    }
    if (ctrlActive && data.length === 1) {
      // Send control character
      const code = data.toUpperCase().charCodeAt(0) - 64
      if (code > 0 && code < 32) {
        onSend(String.fromCharCode(code))
      }
      setCtrlActive(false)
    } else {
      onSend(data)
    }
  }

  const handleTextSubmit = () => {
    const value = inputRef.current?.value ?? ''
    if (value) {
      onSend(value)
      if (inputRef.current) inputRef.current.value = ''
    }
    onSend('\r') // Enter
  }

  return (
    <Box sx={{
      display: 'flex',
      alignItems: 'center',
      gap: 0.5,
      p: 0.5,
      bgcolor: 'background.paper',
      borderTop: '1px solid',
      borderColor: 'divider',
    }}>
      {SPECIAL_KEYS.map(({ label, data }) => (
        <IconButton
          key={label}
          size="small"
          onClick={() => handleKey(data)}
          sx={{
            fontSize: 11,
            minWidth: 32,
            height: 28,
            borderRadius: 1,
            bgcolor: (label === 'Ctrl' && ctrlActive) ? 'primary.main' : 'action.hover',
            color: (label === 'Ctrl' && ctrlActive) ? 'primary.contrastText' : 'text.primary',
          }}
        >
          {label}
        </IconButton>
      ))}
      <TextField
        inputRef={inputRef}
        size="small"
        variant="outlined"
        placeholder="type..."
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            handleTextSubmit()
          }
        }}
        sx={{ flex: 1, '& input': { py: 0.25, fontSize: 12 } }}
      />
      <IconButton size="small" onClick={handleTextSubmit}>
        <KeyboardReturnIcon sx={{ fontSize: 16 }} />
      </IconButton>
    </Box>
  )
}
