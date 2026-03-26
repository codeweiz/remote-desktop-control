import { Terminal, MessageSquare, List, Settings } from 'lucide-react'

export type MobileTab = 'sessions' | 'terminal' | 'chat' | 'settings'

interface MobileNavProps {
  activeTab: MobileTab
  onTabChange: (tab: MobileTab) => void
}

const tabs: { id: MobileTab; label: string; icon: React.ReactNode }[] = [
  { id: 'sessions', label: 'Sessions', icon: <List size={18} /> },
  { id: 'terminal', label: 'Terminal', icon: <Terminal size={18} /> },
  { id: 'chat', label: 'Chat', icon: <MessageSquare size={18} /> },
  { id: 'settings', label: 'Settings', icon: <Settings size={18} /> },
]

export function MobileNav({ activeTab, onTabChange }: MobileNavProps) {
  return (
    <div className="md:hidden fixed bottom-0 left-0 right-0 z-50 bg-bg-secondary border-t border-border flex items-center justify-around h-14 safe-area-bottom">
      {tabs.map(tab => (
        <button
          key={tab.id}
          className={`
            flex flex-col items-center justify-center gap-0.5 flex-1 h-full transition-colors
            ${activeTab === tab.id
              ? 'text-accent-blue'
              : 'text-text-secondary'
            }
          `}
          onClick={() => onTabChange(tab.id)}
        >
          {tab.icon}
          <span className="text-[9px]">{tab.label}</span>
        </button>
      ))}
    </div>
  )
}
