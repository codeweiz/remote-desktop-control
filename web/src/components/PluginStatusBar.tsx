import { Box, Chip, Tooltip, Typography } from '@mui/material'
import {
  Extension as PluginIcon,
  Language as TunnelIcon,
  Chat as FeishuIcon,
} from '@mui/icons-material'
import type { PluginInfo, TunnelStatus } from '../lib/types'

interface PluginStatusBarProps {
  plugins: PluginInfo[]
  tunnel: TunnelStatus | null
}

function isReady(status: string): boolean {
  return status === 'ready' || status === 'running'
}

export function PluginStatusBar({ plugins, tunnel }: PluginStatusBarProps) {
  // Always show — even when no plugins, show a dim indicator
  if (plugins.length === 0 && !tunnel?.active) {
    return (
      <Tooltip title="No plugins active. Configure Feishu or Tunnel in ~/.rtb/config.toml">
        <Chip
          size="small"
          icon={<PluginIcon sx={{ fontSize: 12 }} />}
          label="No Plugins"
          sx={{
            fontSize: 10,
            height: 22,
            bgcolor: 'rgba(255,255,255,0.04)',
            color: 'text.disabled',
            '& .MuiChip-icon': { ml: 0.5 },
            '& .MuiChip-label': { px: 0.5 },
          }}
        />
      </Tooltip>
    )
  }

  const feishuPlugin = plugins.find(
    (p) => p.id.includes('feishu') || p.name.toLowerCase().includes('feishu'),
  )

  const otherPlugins = plugins.filter(
    (p) =>
      !p.id.includes('feishu') &&
      !p.name.toLowerCase().includes('feishu') &&
      !p.id.includes('tunnel') &&
      !p.name.toLowerCase().includes('tunnel'),
  )

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
      {/* Feishu plugin indicator */}
      {feishuPlugin && (
        <Tooltip title={`Feishu: ${feishuPlugin.status}`}>
          <Chip
            size="small"
            icon={<FeishuIcon sx={{ fontSize: 12 }} />}
            label="Feishu"
            sx={{
              fontSize: 10,
              fontWeight: 500,
              height: 22,
              bgcolor: isReady(feishuPlugin.status)
                ? 'rgba(52,211,153,0.1)'
                : 'rgba(248,113,113,0.1)',
              color: isReady(feishuPlugin.status)
                ? 'success.main'
                : 'error.main',
              '& .MuiChip-icon': { ml: 0.5 },
              '& .MuiChip-label': { px: 0.5 },
            }}
          />
        </Tooltip>
      )}

      {/* Tunnel indicator */}
      {tunnel && tunnel.active && (
        <Tooltip
          title={
            tunnel.url
              ? `Tunnel: ${tunnel.url}`
              : `Tunnel: ${tunnel.message}`
          }
        >
          <Chip
            size="small"
            icon={<TunnelIcon sx={{ fontSize: 12 }} />}
            label={tunnel.url ? tunnel.url.replace(/^https?:\/\//, '') : 'Tunnel'}
            sx={{
              fontSize: 10,
              fontWeight: 500,
              height: 22,
              maxWidth: 160,
              bgcolor: 'rgba(96,165,250,0.1)',
              color: '#60a5fa',
              '& .MuiChip-icon': { ml: 0.5 },
              '& .MuiChip-label': {
                px: 0.5,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              },
            }}
          />
        </Tooltip>
      )}

      {/* Other plugins as small dots */}
      {otherPlugins.map((plugin) => (
        <Tooltip key={plugin.id} title={`${plugin.name}: ${plugin.status}`}>
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 0.25,
              px: 0.5,
              py: 0.25,
              borderRadius: 1,
              bgcolor: 'rgba(255,255,255,0.04)',
            }}
          >
            <Box
              sx={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                bgcolor: isReady(plugin.status) ? '#34d399' : '#94a3b8',
                boxShadow: isReady(plugin.status) ? '0 0 4px rgba(52,211,153,0.5)' : 'none',
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
              {plugin.name}
            </Typography>
          </Box>
        </Tooltip>
      ))}
    </Box>
  )
}
