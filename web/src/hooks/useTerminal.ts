import { useEffect, useRef, useCallback, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebglAddon } from '@xterm/addon-webgl'
import { CanvasAddon } from '@xterm/addon-canvas'
import { SearchAddon } from '@xterm/addon-search'
import '@xterm/xterm/css/xterm.css'
import type { ConnectionState } from '../lib/types'
import { getWsUrl } from '../lib/websocket'

interface UseTerminalOptions {
  sessionId: string | null
  fontSize?: number
  isMobile?: boolean
}

interface UseTerminalReturn {
  containerRef: React.RefObject<HTMLDivElement | null>
  connectionState: ConnectionState
  fitTerminal: () => void
  sendData: (data: string) => void
  searchVisible: boolean
  setSearchVisible: (visible: boolean) => void
  findNext: (term: string) => void
  findPrevious: (term: string) => void
}

export function useTerminal({ sessionId, fontSize = 14, isMobile }: UseTerminalOptions): UseTerminalReturn {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const searchAddonRef = useRef<SearchAddon | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected')
  const [searchVisible, setSearchVisible] = useState(false)

  const fitTerminal = useCallback(() => {
    if (fitAddonRef.current) {
      try {
        fitAddonRef.current.fit()
      } catch {
        // Ignore fit errors (element might not be visible)
      }
    }
  }, [])

  const findNext = useCallback((term: string) => {
    if (searchAddonRef.current && term) {
      searchAddonRef.current.findNext(term)
    }
  }, [])

  const findPrevious = useCallback((term: string) => {
    if (searchAddonRef.current && term) {
      searchAddonRef.current.findPrevious(term)
    }
  }, [])

  /** Send raw data to the PTY via WebSocket (used by MobileInputBar). */
  const sendData = useCallback((data: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(new TextEncoder().encode(data))
    }
  }, [])

  // Ctrl+Shift+F keyboard shortcut for search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'f') {
        e.preventDefault()
        setSearchVisible(prev => !prev)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  useEffect(() => {
    if (!containerRef.current || !sessionId) return

    const container = containerRef.current

    // Create terminal instance
    const terminal = new Terminal({
      fontSize,
      fontFamily: "'JetBrains Mono', ui-monospace, monospace",
      theme: {
        background: '#0d1117',
        foreground: '#c9d1d9',
        cursor: '#c9d1d9',
        cursorAccent: '#0d1117',
        selectionBackground: '#264f78',
        selectionForeground: '#ffffff',
        black: '#484f58',
        red: '#ff7b72',
        green: '#7ee787',
        yellow: '#ffa657',
        blue: '#79c0ff',
        magenta: '#d2a8ff',
        cyan: '#56d4dd',
        white: '#b1bac4',
        brightBlack: '#6e7681',
        brightRed: '#ffa198',
        brightGreen: '#56d364',
        brightYellow: '#e3b341',
        brightBlue: '#a5d6ff',
        brightMagenta: '#d2a8ff',
        brightCyan: '#76e3ea',
        brightWhite: '#f0f6fc',
      },
      cursorBlink: true,
      cursorStyle: 'bar',
      scrollback: 0,
      allowProposedApi: true,
      disableStdin: isMobile ?? false,
    })

    // Addons
    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)

    const searchAddon = new SearchAddon()
    terminal.loadAddon(searchAddon)

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon
    searchAddonRef.current = searchAddon

    // Open terminal in DOM
    terminal.open(container)

    // Renderer fallback chain: WebGL -> Canvas -> DOM
    try {
      const webglAddon = new WebglAddon()
      webglAddon.onContextLoss(() => {
        webglAddon.dispose()
        try { terminal.loadAddon(new CanvasAddon()) } catch { /* DOM fallback */ }
      })
      terminal.loadAddon(webglAddon)
    } catch {
      try { terminal.loadAddon(new CanvasAddon()) } catch { /* DOM fallback */ }
    }

    // Initial fit
    requestAnimationFrame(() => {
      fitAddon.fit()
    })

    // Resize observer for responsive terminal
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit()
        } catch {
          // Ignore
        }
      })
    })
    resizeObserver.observe(container)

    // Connect WebSocket for terminal I/O
    const url = getWsUrl('/ws/terminal?session=' + sessionId)
    setConnectionState('connecting')
    const ws = new WebSocket(url)
    ws.binaryType = 'arraybuffer' // MUST be before onmessage
    wsRef.current = ws

    ws.onopen = () => {
      setConnectionState('connected')
      // Wait for DOM to settle before fitting and sending resize
      setTimeout(() => {
        fitAddon.fit()
        terminal.focus()
        const dims = fitAddon.proposeDimensions()
        if (dims) {
          ws.send(JSON.stringify({
            type: 'resize',
            cols: dims.cols,
            rows: dims.rows,
          }))
        }
      }, 100)
    }

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        // Binary frame -> PTY output, write directly
        terminal.write(new Uint8Array(event.data))
      } else if (event.data instanceof Blob) {
        event.data.arrayBuffer().then(buf => terminal.write(new Uint8Array(buf)))
      } else if (typeof event.data === 'string') {
        // Text frame -> control message (JSON)
        try {
          const msg = JSON.parse(event.data)
          if (msg.type === 'exit') {
            terminal.writeln(`\r\n[Process exited with code ${msg.code}]`)
          } else if (msg.type === 'keepalive_ack') {
            // Connection health — could update latency display
          }
        } catch { /* ignore */ }
      }
    }

    ws.onerror = () => {
      setConnectionState('error')
    }

    ws.onclose = () => {
      setConnectionState('disconnected')
    }

    // Keepalive — every 10 seconds
    const keepaliveInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'keepalive', client_time: Date.now() }))
      }
    }, 10000)

    // Terminal input -> WebSocket (send as binary for raw PTY input)
    const inputDisposable = terminal.onData((data: string) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(new TextEncoder().encode(data))
      }
    })

    // Terminal resize -> WebSocket
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({
          type: 'resize',
          cols,
          rows,
        }))
      }
    })

    return () => {
      clearInterval(keepaliveInterval)
      resizeObserver.disconnect()
      inputDisposable.dispose()
      resizeDisposable.dispose()
      ws.close()
      wsRef.current = null
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
      searchAddonRef.current = null
      setConnectionState('disconnected')
      setSearchVisible(false)
    }
  }, [sessionId, fontSize, isMobile])

  return { containerRef, connectionState, fitTerminal, sendData, searchVisible, setSearchVisible, findNext, findPrevious }
}
