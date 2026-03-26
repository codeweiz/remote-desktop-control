import { useState } from 'react'
import {
  Box,
  Paper,
  IconButton,
  Chip,
  ToggleButtonGroup,
  ToggleButton,
  Tooltip,
  Typography,
} from '@mui/material'
import {
  GridView as GridViewIcon,
  ViewStream as FocusIcon,
  Search as SearchIcon,
  QrCode2 as QrCodeIcon,
  Hexagon as HexagonIcon,
  WifiOff as WifiOffIcon,
} from '@mui/icons-material'
import type { ConnectionState } from '../lib/types'
import { QRCodeModal } from './QRCodeModal'

interface TopBarProps {
  viewMode: 'grid' | 'focus'
  connectionState: ConnectionState
  latency: number | null
  onToggleView: () => void
  onOpenCommandPalette: () => void
}

export function TopBar({
  viewMode,
  connectionState,
  latency,
  onToggleView,
  onOpenCommandPalette,
}: TopBarProps) {
  const isConnected = connectionState === 'connected'
  const [qrOpen, setQrOpen] = useState(false)

  return (
    <Paper
      elevation={3}
      sx={{
        mx: 1,
        mt: 1,
        borderRadius: 3,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        px: 2,
        py: 0.75,
        minHeight: 48,
        flexShrink: 0,
      }}
    >
      {/* Left: Logo + Connection status */}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.75 }}>
          <HexagonIcon sx={{ fontSize: 18, color: 'success.main' }} />
          <Typography variant="subtitle2" sx={{ fontWeight: 600, fontSize: 14 }}>
            RTB
          </Typography>
          <Typography
            variant="caption"
            sx={{
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: 10,
              color: 'text.secondary',
            }}
          >
            2.0
          </Typography>
        </Box>

        <Chip
          size="small"
          icon={
            isConnected ? (
              <Box
                sx={{
                  width: 6,
                  height: 6,
                  borderRadius: '50%',
                  bgcolor: 'success.main',
                  boxShadow: '0 0 6px rgba(52,211,153,0.6)',
                  animation: 'pulse-glow 2s ease-in-out infinite',
                }}
              />
            ) : (
              <WifiOffIcon sx={{ fontSize: 12 }} />
            )
          }
          label={
            isConnected
              ? `Connected${latency !== null ? ` ${latency}ms` : ''}`
              : connectionState === 'connecting'
                ? 'Connecting...'
                : 'Disconnected'
          }
          sx={{
            bgcolor: isConnected
              ? 'rgba(52,211,153,0.1)'
              : 'rgba(248,113,113,0.1)',
            color: isConnected ? 'success.main' : 'error.main',
            fontSize: 11,
            fontWeight: 500,
            '& .MuiChip-icon': { ml: 0.5 },
          }}
        />
      </Box>

      {/* Center: View mode toggle */}
      <ToggleButtonGroup
        value={viewMode}
        exclusive
        onChange={onToggleView}
        size="small"
        sx={{
          '& .MuiToggleButton-root': {
            px: 1.5,
            py: 0.5,
            fontSize: 12,
            textTransform: 'none',
            border: '1px solid',
            borderColor: 'divider',
          },
        }}
      >
        <ToggleButton value="grid">
          <GridViewIcon sx={{ fontSize: 16, mr: 0.5 }} />
          Grid
        </ToggleButton>
        <ToggleButton value="focus">
          <FocusIcon sx={{ fontSize: 16, mr: 0.5 }} />
          Focus
        </ToggleButton>
      </ToggleButtonGroup>

      {/* Right: Actions */}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
        <Tooltip title="Search (Cmd+K)">
          <IconButton size="small" onClick={onOpenCommandPalette}>
            <SearchIcon sx={{ fontSize: 18 }} />
          </IconButton>
        </Tooltip>
        <Tooltip title="QR Code">
          <IconButton size="small" onClick={() => setQrOpen(true)}>
            <QrCodeIcon sx={{ fontSize: 18 }} />
          </IconButton>
        </Tooltip>
      </Box>

      <QRCodeModal isOpen={qrOpen} onClose={() => setQrOpen(false)} />
    </Paper>
  )
}
