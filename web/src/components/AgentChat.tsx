import { useState, useRef, useEffect, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import { Send, Bot, User, Cpu, X } from 'lucide-react'
import type { Session, AgentMessage, WsMessage } from '../lib/types'
import { useWebSocket } from '../hooks/useWebSocket'

interface AgentChatProps {
  session: Session | null
  isVisible: boolean
  onToggle: () => void
}

export function AgentChat({ session, isVisible, onToggle }: AgentChatProps) {
  const [messages, setMessages] = useState<AgentMessage[]>([])
  const [input, setInput] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [])

  useEffect(() => {
    scrollToBottom()
  }, [messages, scrollToBottom])

  // Reset messages when session changes
  useEffect(() => {
    setMessages([])
  }, [session?.id])

  const handleAgentMessage = useCallback((msg: WsMessage) => {
    if (msg.type === 'agent_message') {
      const agentMsg: AgentMessage = {
        id: (msg.id as string) || crypto.randomUUID(),
        role: (msg.role as AgentMessage['role']) || 'assistant',
        content: (msg.content as string) || '',
        timestamp: (msg.timestamp as string) || new Date().toISOString(),
        model: msg.model as string | undefined,
      }
      setMessages(prev => [...prev, agentMsg])
    }
  }, [])

  const { send, connectionState } = useWebSocket({
    path: session ? `/ws/agent/${session.id}` : '',
    enabled: !!session && session.kind === 'agent',
    onMessage: handleAgentMessage,
  })

  const handleSend = useCallback(() => {
    if (!input.trim() || !session) return

    const userMsg: AgentMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: input.trim(),
      timestamp: new Date().toISOString(),
    }
    setMessages(prev => [...prev, userMsg])

    send({
      type: 'agent_input',
      content: input.trim(),
    })

    setInput('')
  }, [input, session, send])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }, [handleSend])

  if (!isVisible) return null

  return (
    <div className="w-[340px] bg-[var(--bg-secondary)] border-l border-[var(--border-color)] flex flex-col shrink-0">
      {/* Header */}
      <div className="h-10 px-3 flex items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-secondary)] shrink-0">
        <div className="flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent-purple)] animate-pulse-dot" />
          <span className="text-sm font-medium text-[var(--text-primary)]">
            {session ? (session.name || `Agent ${session.id.slice(0, 6)}`) : 'Agent Chat'}
          </span>
          {session && (
            <span className="text-[10px] bg-[var(--bg-elevated)] px-1.5 py-0.5 rounded font-mono text-[var(--text-muted)]">
              <Cpu size={9} className="inline mr-0.5" />
              agent
            </span>
          )}
        </div>
        <button
          onClick={onToggle}
          className="w-6 h-6 flex items-center justify-center rounded-md hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
          title="Hide panel"
        >
          <X size={14} />
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        {!session ? (
          <div className="flex-1 flex items-center justify-center h-full">
            <div className="text-center text-[var(--text-muted)] animate-fade-in">
              <Bot size={36} className="mx-auto mb-3 text-[var(--accent-purple)] opacity-30" />
              <p className="text-sm font-medium text-[var(--text-secondary)]">Select an agent session</p>
              <p className="text-xs mt-1 text-[var(--text-muted)]">or create one from the sidebar</p>
            </div>
          </div>
        ) : messages.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <div className="text-center text-[var(--text-muted)] animate-fade-in">
              <p className="text-sm">No messages yet</p>
              <p className="text-xs mt-1">
                {connectionState === 'connected' ? 'Type a message below' : 'Connecting...'}
              </p>
            </div>
          </div>
        ) : (
          messages.map(msg => (
            <div key={msg.id} className={`flex gap-2 animate-fade-in ${msg.role === 'user' ? 'justify-end' : ''}`}>
              {msg.role !== 'user' && (
                <div className="w-6 h-6 rounded-md bg-[var(--accent-purple)]/15 flex items-center justify-center shrink-0 mt-0.5">
                  <Bot size={12} className="text-[var(--accent-purple)]" />
                </div>
              )}
              <div
                className={`
                  max-w-[85%] text-sm
                  ${msg.role === 'user'
                    ? 'bg-[var(--bg-elevated)] rounded-lg px-3 py-2 text-[var(--text-primary)]'
                    : 'bg-transparent border-l-2 border-[var(--accent-purple)] pl-3 py-1 text-[var(--text-primary)]'
                  }
                `}
              >
                <div className="markdown-content">
                  <ReactMarkdown>{msg.content}</ReactMarkdown>
                </div>
                {msg.model && (
                  <div className="text-[10px] font-mono text-[var(--text-muted)] mt-1">{msg.model}</div>
                )}
              </div>
              {msg.role === 'user' && (
                <div className="w-6 h-6 rounded-md bg-[var(--accent-blue)]/15 flex items-center justify-center shrink-0 mt-0.5">
                  <User size={12} className="text-[var(--accent-blue)]" />
                </div>
              )}
            </div>
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      {session && (
        <div className="border-t border-[var(--border-color)] p-2">
          <div className="flex items-end gap-2">
            <textarea
              className="flex-1 bg-[var(--bg-secondary)] border border-[var(--border-color)] rounded-lg px-3 py-2 text-sm text-[var(--text-primary)] resize-none outline-none placeholder:text-[var(--text-muted)] focus:border-[var(--accent-purple)] focus:ring-1 focus:ring-[var(--accent-purple)]/30 transition-all duration-150"
              placeholder="Message agent..."
              rows={1}
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={connectionState !== 'connected'}
            />
            <button
              className="p-2 rounded-lg hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--accent-purple)] transition-colors duration-150 disabled:opacity-30 cursor-pointer disabled:cursor-not-allowed"
              onClick={handleSend}
              disabled={!input.trim() || connectionState !== 'connected'}
            >
              <Send size={16} />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
