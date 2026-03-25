import { getToken } from './api'

/** Build a WebSocket URL from a path, using the current page protocol/host */
export function getWsUrl(path: string): string {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = getToken()
  const separator = path.includes('?') ? '&' : '?'
  const tokenParam = token ? `${separator}token=${encodeURIComponent(token)}` : ''
  return `${proto}//${window.location.host}${path}${tokenParam}`
}
