import { useState, useRef, useEffect, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import { Send, Bot, User, Cpu } from 'lucide-react'
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
    <div className="w-[340px] bg-bg-secondary border-l border-border flex flex-col shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <div className="flex items-center gap-2">
          <Bot size={14} className="text-accent-purple" />
          <span className="text-xs font-medium text-text-primary">
            {session ? (session.name || `Agent ${session.id.slice(0, 6)}`) : 'Agent Chat'}
          </span>
          {session && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-bg-tertiary text-accent-purple">
              <Cpu size={9} className="inline mr-0.5" />
              agent
            </span>
          )}
        </div>
        <button
          onClick={onToggle}
          className="text-xs text-text-secondary hover:text-text-primary transition-colors"
        >
          Hide
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        {!session ? (
          <div className="flex-1 flex items-center justify-center h-full">
            <div className="text-center text-text-secondary">
              <Bot size={32} className="mx-auto mb-2 opacity-20" />
              <p className="text-xs">Select an agent session</p>
              <p className="text-[10px] mt-1">or create one from the sidebar</p>
            </div>
          </div>
        ) : messages.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <div className="text-center text-text-secondary">
              <p className="text-xs">No messages yet</p>
              <p className="text-[10px] mt-1">
                {connectionState === 'connected' ? 'Type a message below' : 'Connecting...'}
              </p>
            </div>
          </div>
        ) : (
          messages.map(msg => (
            <div key={msg.id} className={`flex gap-2 ${msg.role === 'user' ? 'justify-end' : ''}`}>
              {msg.role !== 'user' && (
                <div className="w-6 h-6 rounded bg-accent-purple/20 flex items-center justify-center shrink-0 mt-0.5">
                  <Bot size={12} className="text-accent-purple" />
                </div>
              )}
              <div
                className={`
                  max-w-[85%] rounded-lg px-3 py-2 text-xs
                  ${msg.role === 'user'
                    ? 'bg-accent-blue/20 text-text-primary'
                    : 'bg-bg-tertiary text-text-primary'
                  }
                `}
              >
                <div className="markdown-content">
                  <ReactMarkdown>{msg.content}</ReactMarkdown>
                </div>
                {msg.model && (
                  <div className="text-[10px] text-text-secondary mt-1">{msg.model}</div>
                )}
              </div>
              {msg.role === 'user' && (
                <div className="w-6 h-6 rounded bg-accent-blue/20 flex items-center justify-center shrink-0 mt-0.5">
                  <User size={12} className="text-accent-blue" />
                </div>
              )}
            </div>
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      {session && (
        <div className="p-3 border-t border-border">
          <div className="flex items-end gap-2 bg-bg-tertiary rounded-lg p-2">
            <textarea
              className="flex-1 bg-transparent text-xs text-text-primary resize-none outline-none placeholder-text-secondary"
              placeholder="Message agent..."
              rows={1}
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={connectionState !== 'connected'}
            />
            <button
              className="p-1 rounded hover:bg-bg-secondary text-text-secondary hover:text-accent-blue transition-colors disabled:opacity-30"
              onClick={handleSend}
              disabled={!input.trim() || connectionState !== 'connected'}
            >
              <Send size={14} />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
