/** Session types matching the backend API */
export type SessionKind = 'terminal' | 'agent'

export type SessionStatus = 'running' | 'exited' | 'error'

export interface Session {
  id: string
  name: string
  kind: SessionKind
  status: SessionStatus
  parent_id: string | null
  created_at: string
  exit_code: number | null
  shell: string | null
  cols: number
  rows: number
}

export interface SessionCreateRequest {
  name?: string
  kind?: SessionKind
  shell?: string
  cols?: number
  rows?: number
  parent_id?: string | null
}

export interface ServerStatus {
  sessions: number
  agents: number
  uptime: number
  tunnel_url: string | null
}

/** WebSocket message types */
export interface WsMessage {
  type: string
  [key: string]: unknown
}

export interface TerminalOutput {
  type: 'output'
  data: string // base64 encoded
}

export interface TerminalInput {
  type: 'input'
  data: string // base64 encoded
}

export interface TerminalResize {
  type: 'resize'
  cols: number
  rows: number
}

export interface SessionEvent {
  type: 'session_created' | 'session_deleted' | 'session_updated'
  session: Session
}

export interface AgentMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: string
  model?: string
}

export type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error'

export type Theme = 'dark' | 'light'
