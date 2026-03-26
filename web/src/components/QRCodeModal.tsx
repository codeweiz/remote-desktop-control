import { useState, useEffect } from 'react'
import {
  Dialog,
  Box,
  Typography,
  IconButton,
  Paper,
  Backdrop,
} from '@mui/material'
import {
  Close as CloseIcon,
  ContentCopy as CopyIcon,
  Check as CheckIcon,
} from '@mui/icons-material'
import QRCode from 'qrcode'
import { getToken } from '../lib/api'

interface QRCodeModalProps {
  isOpen: boolean
  onClose: () => void
}

export function QRCodeModal({ isOpen, onClose }: QRCodeModalProps) {
  const [svgData, setSvgData] = useState<string>('')
  const [copied, setCopied] = useState(false)

  const deepLink = buildDeepLink()

  useEffect(() => {
    if (!isOpen) return

    QRCode.toString(deepLink, {
      type: 'svg',
      color: {
        dark: '#f1f5f9',
        light: '#00000000',
      },
      margin: 1,
      width: 220,
    }).then(svg => {
      setSvgData(svg)
    }).catch(() => {
      setSvgData('')
    })
  }, [isOpen, deepLink])

  const handleCopy = () => {
    navigator.clipboard.writeText(deepLink).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }).catch(() => {
      // Clipboard not available
    })
  }

  return (
    <Dialog
      open={isOpen}
      onClose={onClose}
      maxWidth="xs"
      fullWidth
      slots={{ backdrop: Backdrop }}
      slotProps={{
        backdrop: {
          sx: { backdropFilter: 'blur(8px)', bgcolor: 'rgba(0,0,0,0.5)' },
        },
      }}
      PaperProps={{
        sx: {
          borderRadius: 2,
          maxWidth: 340,
          overflow: 'hidden',
        },
      }}
    >
      {/* Header */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 2.5,
          py: 1.5,
          borderBottom: '1px solid',
          borderColor: 'divider',
        }}
      >
        <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
          Connect via QR Code
        </Typography>
        <IconButton size="small" onClick={onClose}>
          <CloseIcon sx={{ fontSize: 18 }} />
        </IconButton>
      </Box>

      {/* QR Code */}
      <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', p: 3 }}>
        {svgData ? (
          <Box
            sx={{
              bgcolor: 'rgba(255,255,255,0.05)',
              borderRadius: 2,
              p: 2,
            }}
            dangerouslySetInnerHTML={{ __html: svgData }}
          />
        ) : (
          <Box
            sx={{
              width: 220,
              height: 220,
              bgcolor: 'rgba(255,255,255,0.05)',
              borderRadius: 2,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Typography variant="caption" sx={{ color: 'text.secondary' }}>
              Generating...
            </Typography>
          </Box>
        )}

        <Typography
          variant="caption"
          sx={{
            mt: 2,
            textAlign: 'center',
            maxWidth: 240,
            color: 'text.secondary',
            fontSize: 11,
          }}
        >
          Scan this QR code with the RTB mobile app to connect to this server
        </Typography>

        {/* Deep link URL */}
        <Box
          sx={{
            mt: 2,
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: 0.5,
            bgcolor: 'rgba(255,255,255,0.05)',
            borderRadius: 1,
            px: 1.5,
            py: 1,
            border: '1px solid',
            borderColor: 'divider',
          }}
        >
          <Typography
            variant="caption"
            sx={{
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: 10,
              color: 'text.secondary',
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {deepLink}
          </Typography>
          <IconButton size="small" onClick={handleCopy} title="Copy link" sx={{ p: 0.5, flexShrink: 0 }}>
            {copied ? (
              <CheckIcon sx={{ fontSize: 14, color: 'success.main' }} />
            ) : (
              <CopyIcon sx={{ fontSize: 14 }} />
            )}
          </IconButton>
        </Box>
      </Box>
    </Dialog>
  )
}

function buildDeepLink(): string {
  const host = window.location.host
  const token = getToken()
  const params = new URLSearchParams()
  params.set('host', host)
  if (token) {
    params.set('token', token)
  }
  return `rtb://connect?${params.toString()}`
}
