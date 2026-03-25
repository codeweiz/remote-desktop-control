import type { Session, SessionCreateRequest, ServerStatus } from './types'

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
  return apiFetch<Session[]>('/api/sessions')
}

/** Create a new session */
export async function createSession(req: SessionCreateRequest = {}): Promise<Session> {
  return apiFetch<Session>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify(req),
  })
}

/** Get a single session */
export async function getSession(id: string): Promise<Session> {
  return apiFetch<Session>(`/api/sessions/${id}`)
}

/** Delete a session */
export async function deleteSession(id: string): Promise<void> {
  return apiFetch<void>(`/api/sessions/${id}`, {
    method: 'DELETE',
  })
}

/** Get server status */
export async function getStatus(): Promise<ServerStatus> {
  return apiFetch<ServerStatus>('/api/status')
}
