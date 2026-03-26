import type { Session, SessionKind, SessionStatus, SessionCreateRequest, ServerStatus, Task, TaskCreateRequest } from './types'

/** Extract and store auth token from URL or localStorage */
function initToken(): string | null {
  const params = new URLSearchParams(window.location.search)
  const urlToken = params.get('token')
  if (urlToken) {
    localStorage.setItem('rtb_token', urlToken)
    // Remove token from URL to avoid leaking it
    params.delete('token')
    const newUrl = params.toString()
      ? `${window.location.pathname}?${params.toString()}`
      : window.location.pathname
    window.history.replaceState({}, '', newUrl)
    return urlToken
  }
  return localStorage.getItem('rtb_token')
}

let token = initToken()

export function getToken(): string | null {
  return token
}

export function setToken(t: string | null) {
  token = t
  if (t) {
    localStorage.setItem('rtb_token', t)
  } else {
    localStorage.removeItem('rtb_token')
  }
}

/** Fetch wrapper with auth header */
async function apiFetch<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string> || {}),
  }
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }
  const res = await fetch(path, { ...options, headers })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(`API error ${res.status}: ${body}`)
  }
  if (res.status === 204) {
    return undefined as T
  }
  return res.json()
}

/** List all sessions */
export async function getSessions(): Promise<Session[]> {
  return apiFetch<Session[]>('/api/v1/sessions')
}

/** Create a new session */
export async function createSession(req: SessionCreateRequest = {}): Promise<Session> {
  const body: Record<string, unknown> = {
    name: req.name || `${req.kind || 'terminal'}-${Date.now()}`,
    type: req.kind || 'terminal',
  }
  if (req.kind === 'agent') {
    body.provider = req.provider || 'claude-code'
    body.model = req.model || ''
  } else {
    body.shell = req.shell
  }
  const result = await apiFetch<{ id: string; status?: string; error?: string }>('/api/v1/sessions', {
    method: 'POST',
    body: JSON.stringify(body),
  })
  // Server only returns { id, status?, error? }, so construct a minimal Session object
  return {
    id: result.id,
    name: body.name as string,
    kind: (req.kind || 'terminal') as SessionKind,
    status: (result.status === 'crashed' ? 'error' : 'running') as SessionStatus,
    parent_id: null,
    created_at: new Date().toISOString(),
    exit_code: null,
    shell: req.shell || null,
    cols: 80,
    rows: 24,
    provider: req.kind === 'agent' ? (req.provider || 'claude-code') : undefined,
  }
}

/** Get a single session */
export async function getSession(id: string): Promise<Session> {
  return apiFetch<Session>(`/api/v1/sessions/${id}`)
}

/** Delete a session */
export async function deleteSession(id: string): Promise<void> {
  return apiFetch<void>(`/api/v1/sessions/${id}`, {
    method: 'DELETE',
  })
}

/** Get server status */
export async function getStatus(): Promise<ServerStatus> {
  return apiFetch<ServerStatus>('/api/v1/status')
}

/** List all tasks */
export async function getTasks(): Promise<Task[]> {
  return apiFetch<Task[]>('/api/v1/tasks')
}

/** Create a new task */
export async function createTask(req: TaskCreateRequest): Promise<Task> {
  return apiFetch<Task>('/api/v1/tasks', {
    method: 'POST',
    body: JSON.stringify(req),
  })
}

/** Update a task (approve, cancel, etc.) */
export async function updateTask(id: string, update: Partial<Task>): Promise<Task> {
  return apiFetch<Task>(`/api/v1/tasks/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(update),
  })
}

/** Delete a task */
export async function deleteTask(id: string): Promise<void> {
  return apiFetch<void>(`/api/v1/tasks/${id}`, {
    method: 'DELETE',
  })
}

/** Rename a session */
export async function renameSession(id: string, name: string): Promise<Session> {
  return apiFetch<Session>(`/api/v1/sessions/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ name }),
  })
}
