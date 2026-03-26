import { useState, useCallback } from 'react'
import {
  ChevronDown,
  ChevronUp,
  Plus,
  CheckCircle,
  XCircle,
  ListTodo,
  RefreshCw,
} from 'lucide-react'
import type { Task, TaskPriority, TaskStatus } from '../lib/types'
import { useTaskPool } from '../hooks/useTaskPool'

function priorityColor(priority: TaskPriority): string {
  switch (priority) {
    case 'P0': return 'bg-red-500/20 text-red-400'
    case 'P1': return 'bg-amber-500/20 text-amber-400'
    case 'P2': return 'bg-green-500/20 text-green-400'
    case 'P3': return 'bg-blue-500/20 text-blue-400'
  }
}

function statusColor(status: TaskStatus): string {
  switch (status) {
    case 'Pending': return 'bg-[var(--bg-elevated)] text-[var(--text-muted)]'
    case 'InProgress': return 'bg-blue-500/20 text-blue-400'
    case 'NeedsReview': return 'bg-amber-500/20 text-amber-400'
    case 'Completed': return 'bg-green-500/20 text-green-400'
    case 'Cancelled': return 'bg-[var(--bg-elevated)] text-[var(--text-muted)] line-through'
  }
}

function TaskItem({
  task,
  onApprove,
  onCancel,
}: {
  task: Task
  onApprove: () => void
  onCancel: () => void
}) {
  return (
    <div className="group flex items-start gap-2 px-3 py-1.5 hover:bg-[var(--bg-hover)] transition-colors duration-150">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className={`w-5 h-5 rounded text-[10px] font-bold flex items-center justify-center shrink-0 ${priorityColor(task.priority)}`}>
            {task.priority}
          </span>
          <span className="text-xs text-[var(--text-primary)] truncate">{task.title}</span>
        </div>
        <span className={`text-[10px] font-mono px-1 py-0.5 rounded mt-0.5 inline-block ${statusColor(task.status)}`}>
          {task.status}
        </span>
      </div>
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity duration-150 shrink-0">
        {task.status === 'NeedsReview' && (
          <button
            onClick={onApprove}
            className="p-0.5 rounded hover:bg-green-500/20 text-green-400 transition-colors duration-150 cursor-pointer"
            title="Approve"
          >
            <CheckCircle size={12} />
          </button>
        )}
        {task.status !== 'Completed' && task.status !== 'Cancelled' && (
          <button
            onClick={onCancel}
            className="p-0.5 rounded hover:bg-red-500/20 text-red-400 transition-colors duration-150 cursor-pointer"
            title="Cancel"
          >
            <XCircle size={12} />
          </button>
        )}
      </div>
    </div>
  )
}

function AddTaskDialog({
  onSubmit,
  onClose,
}: {
  onSubmit: (title: string, priority: TaskPriority) => void
  onClose: () => void
}) {
  const [title, setTitle] = useState('')
  const [priority, setPriority] = useState<TaskPriority>('P2')

  const handleSubmit = () => {
    if (!title.trim()) return
    onSubmit(title.trim(), priority)
    onClose()
  }

  return (
    <div className="px-3 py-2 border-t border-[var(--border-color)] space-y-2">
      <input
        type="text"
        className="w-full bg-[var(--bg-elevated)] text-xs text-[var(--text-primary)] rounded-md px-2 py-1.5 outline-none border border-[var(--border-color)] focus:border-[var(--accent-blue)] transition-colors duration-150 placeholder:text-[var(--text-muted)]"
        placeholder="Task title..."
        value={title}
        onChange={e => setTitle(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') handleSubmit()
          if (e.key === 'Escape') onClose()
        }}
        autoFocus
      />
      <div className="flex items-center gap-1">
        {(['P0', 'P1', 'P2', 'P3'] as TaskPriority[]).map(p => (
          <button
            key={p}
            className={`text-[10px] font-bold px-1.5 py-0.5 rounded transition-colors duration-150 cursor-pointer ${
              priority === p ? priorityColor(p) : 'bg-[var(--bg-elevated)] text-[var(--text-muted)]'
            }`}
            onClick={() => setPriority(p)}
          >
            {p}
          </button>
        ))}
        <div className="flex-1" />
        <button
          className="text-[10px] px-2 py-0.5 rounded bg-blue-500/20 text-blue-400 hover:bg-blue-500/30 transition-colors duration-150 cursor-pointer"
          onClick={handleSubmit}
        >
          Add
        </button>
        <button
          className="text-[10px] px-2 py-0.5 rounded bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
          onClick={onClose}
        >
          Cancel
        </button>
      </div>
    </div>
  )
}

export function TaskPool() {
  const { tasks, loading, addTask, approveTask, cancelTask, refresh } = useTaskPool()
  const [expanded, setExpanded] = useState(false)
  const [showAddDialog, setShowAddDialog] = useState(false)

  const handleAddTask = useCallback(async (title: string, priority: TaskPriority) => {
    try {
      await addTask({ title, priority })
    } catch {
      // Error tracked in hook
    }
  }, [addTask])

  const activeTasks = tasks.filter(t => t.status !== 'Completed' && t.status !== 'Cancelled')
  const displayTasks = expanded ? tasks : activeTasks.slice(0, 5)

  return (
    <div className="border-t border-[var(--border-color)]">
      {/* Header */}
      <button
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-[var(--bg-hover)] transition-colors duration-150 cursor-pointer"
        onClick={() => setExpanded(prev => !prev)}
      >
        <span className="text-[10px] font-medium uppercase tracking-wider text-[var(--text-muted)] flex items-center gap-1.5">
          <ListTodo size={11} />
          Tasks
          {activeTasks.length > 0 && (
            <span className="ml-1 text-[9px] px-1.5 py-0.5 rounded-full bg-blue-500/20 text-blue-400 font-bold">
              {activeTasks.length}
            </span>
          )}
        </span>
        <div className="flex items-center gap-1">
          <button
            className="p-0.5 rounded hover:bg-[var(--bg-secondary)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors duration-150 cursor-pointer"
            onClick={(e) => {
              e.stopPropagation()
              refresh()
            }}
            title="Refresh tasks"
          >
            <RefreshCw size={10} className={loading ? 'animate-spin' : ''} />
          </button>
          <button
            className="p-0.5 rounded hover:bg-[var(--bg-secondary)] text-[var(--text-muted)] hover:text-[var(--accent-green)] transition-colors duration-150 cursor-pointer"
            onClick={(e) => {
              e.stopPropagation()
              setShowAddDialog(true)
              setExpanded(true)
            }}
            title="Add task"
          >
            <Plus size={12} />
          </button>
          {expanded ? <ChevronUp size={12} className="text-[var(--text-muted)]" /> : <ChevronDown size={12} className="text-[var(--text-muted)]" />}
        </div>
      </button>

      {/* Task list */}
      {expanded && (
        <div className="max-h-[200px] overflow-y-auto">
          {displayTasks.length === 0 ? (
            <div className="px-3 py-3 text-xs text-[var(--text-muted)] italic text-center">
              {loading ? 'Loading tasks...' : 'No tasks'}
            </div>
          ) : (
            displayTasks.map(task => (
              <TaskItem
                key={task.id}
                task={task}
                onApprove={() => approveTask(task.id)}
                onCancel={() => cancelTask(task.id)}
              />
            ))
          )}
        </div>
      )}

      {/* Add task dialog */}
      {showAddDialog && (
        <AddTaskDialog
          onSubmit={handleAddTask}
          onClose={() => setShowAddDialog(false)}
        />
      )}
    </div>
  )
}
