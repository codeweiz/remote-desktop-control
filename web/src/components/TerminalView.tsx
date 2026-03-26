import { useEffect } from 'react'
import { Box, Paper, IconButton, Typography } from '@mui/material'
import {
  Close as CloseIcon,
  KeyboardArrowUp as ChevronUpIcon,
  KeyboardArrowDown as ChevronDownIcon,
  Search as SearchIcon,
} from '@mui/icons-material'
import { useTerminal } from '../hooks/useTerminal'
import { SearchBar } from './SearchBar'

interface TerminalViewProps {
  sessionId: string | null
  fontSize?: number
  /** Called when the terminal's sendData function becomes available. */
  onSendReady?: (send: (data: string) => void) => void
}

export function TerminalView({ sessionId, fontSize = 14, onSendReady }: TerminalViewProps) {
  const {
    containerRef,
    connectionState,
    sendData,
    searchVisible,
    setSearchVisible,
    findNext,
    findPrevious,
  } = useTerminal({ sessionId, fontSize })

  // Notify parent when sendData is available
  useEffect(() => {
    onSendReady?.(sendData)
  }, [sendData, onSendReady])

  return (
    <Box
      sx={{
        flex: 1,
        position: 'relative',
        bgcolor: '#0d1117',
        minHeight: 0,
      }}
    >
      {/* Connection indicator */}
      <Box
        sx={{
          position: 'absolute',
          top: 4,
          left: 8,
          zIndex: 10,
          display: 'flex',
          alignItems: 'center',
          gap: 0.5,
          opacity: 0.6,
        }}
      >
        <Box
          sx={{
            width: 6,
            height: 6,
            borderRadius: '50%',
            bgcolor:
              connectionState === 'connected'
                ? 'success.main'
                : connectionState === 'connecting'
                  ? 'warning.main'
                  : 'text.secondary',
            animation:
              connectionState === 'connecting'
                ? 'pulse-glow 2s ease-in-out infinite'
                : 'none',
          }}
        />
        <Typography
          variant="caption"
          sx={{
            fontSize: 9,
            fontFamily: "'JetBrains Mono', monospace",
            color: 'text.secondary',
          }}
        >
          {connectionState}
        </Typography>
      </Box>

      {/* Search bar */}
      <SearchBar
        isVisible={searchVisible}
        onClose={() => setSearchVisible(false)}
        onFindNext={findNext}
        onFindPrevious={findPrevious}
      />

      {/* Terminal container */}
      <Box
        ref={containerRef}
        className="terminal-container"
        sx={{
          position: 'absolute',
          inset: 0,
        }}
      />
    </Box>
  )
}
