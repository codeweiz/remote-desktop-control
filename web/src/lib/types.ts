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
  provider?: string
}

export interface SessionCreateRequest {
  name?: string
  kind?: SessionKind
  shell?: string
  provider?: string
  model?: string
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

export interface TerminalResize {
  type: 'resize'
  cols: number
  rows: number
}

export interface KeepaliveMessage {
  type: 'keepalive'
  client_time: number
}

export interface KeepaliveAck {
  type: 'keepalive_ack'
  server_time: number
}

export interface TerminalExit {
  type: 'exit'
  code: number
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

/** Task Pool types */
export type TaskPriority = 'P0' | 'P1' | 'P2' | 'P3'

export type TaskStatus = 'Pending' | 'InProgress' | 'NeedsReview' | 'Completed' | 'Cancelled'

export interface Task {
  id: string
  title: string
  description?: string
  priority: TaskPriority
  status: TaskStatus
  session_id?: string | null
  created_at: string
  updated_at?: string
}

export interface TaskCreateRequest {
  title: string
  description?: string
  priority?: TaskPriority
}

/** Plugin types */
export interface PluginInfo {
  id: string
  name: string
  type: string
  status: string
}

export interface TunnelStatus {
  active: boolean
  provider?: string
  url?: string
  message: string
}

/** Notification types */
export interface NotificationEvent {
  type: 'notification'
  id: string
  trigger: 'agent' | 'task' | 'session' | 'system'
  summary: string
  session_id?: string
  session_name?: string
  timestamp: string
}
