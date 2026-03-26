import { useState, useEffect, useCallback } from 'react'
import type { PluginInfo, TunnelStatus } from '../lib/types'
import { getPlugins, getTunnelStatus } from '../lib/api'

export interface PluginState {
  plugins: PluginInfo[]
  tunnel: TunnelStatus | null
  loading: boolean
  error: string | null
  refresh: () => Promise<void>
}

const POLL_INTERVAL = 15_000 // 15 seconds

export function usePlugins(): PluginState {
  const [plugins, setPlugins] = useState<PluginInfo[]>([])
  const [tunnel, setTunnel] = useState<TunnelStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    try {
      const [pluginList, tunnelStatus] = await Promise.all([
        getPlugins().catch(() => [] as PluginInfo[]),
        getTunnelStatus().catch(() => null),
      ])
      setPlugins(pluginList)
      setTunnel(tunnelStatus)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch plugin status')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    refresh()
    const interval = setInterval(refresh, POLL_INTERVAL)
    return () => clearInterval(interval)
  }, [refresh])

  return { plugins, tunnel, loading, error, refresh }
}
