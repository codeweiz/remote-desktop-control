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
    case 'P0': return 'bg-accent-red/20 text-accent-red'
    case 'P1': return 'bg-accent-orange/20 text-accent-orange'
    case 'P2': return 'bg-accent-green/20 text-accent-green'
    case 'P3': return 'bg-accent-blue/20 text-accent-blue'
  }
}

function statusColor(status: TaskStatus): string {
  switch (status) {
    case 'Pending': return 'bg-bg-tertiary text-text-secondary'
    case 'InProgress': return 'bg-accent-blue/20 text-accent-blue'
    case 'NeedsReview': return 'bg-accent-orange/20 text-accent-orange'
    case 'Completed': return 'bg-accent-green/20 text-accent-green'
    case 'Cancelled': return 'bg-bg-tertiary text-text-secondary line-through'
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
    <div className="group flex items-start gap-2 px-3 py-1.5 hover:bg-bg-tertiary transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className={`text-[9px] font-bold px-1 py-0.5 rounded ${priorityColor(task.priority)}`}>
            {task.priority}
          </span>
          <span className="text-xs text-text-primary truncate">{task.title}</span>
        </div>
        <span className={`text-[9px] px-1 py-0.5 rounded mt-0.5 inline-block ${statusColor(task.status)}`}>
          {task.status}
        </span>
      </div>
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
        {task.status === 'NeedsReview' && (
          <button
            onClick={onApprove}
            className="p-0.5 rounded hover:bg-accent-green/20 text-accent-green transition-colors"
            title="Approve"
          >
            <CheckCircle size={12} />
          </button>
        )}
        {task.status !== 'Completed' && task.status !== 'Cancelled' && (
          <button
            onClick={onCancel}
            className="p-0.5 rounded hover:bg-accent-red/20 text-accent-red transition-colors"
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
    <div className="px-3 py-2 border-t border-border space-y-2">
      <input
        type="text"
        className="w-full bg-bg-tertiary text-xs text-text-primary rounded px-2 py-1.5 outline-none border border-border focus:border-accent-blue transition-colors placeholder-text-secondary"
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
            className={`text-[9px] font-bold px-1.5 py-0.5 rounded transition-colors ${
              priority === p ? priorityColor(p) : 'bg-bg-tertiary text-text-secondary'
            }`}
            onClick={() => setPriority(p)}
          >
            {p}
          </button>
        ))}
        <div className="flex-1" />
        <button
          className="text-[10px] px-2 py-0.5 rounded bg-accent-blue/20 text-accent-blue hover:bg-accent-blue/30 transition-colors"
          onClick={handleSubmit}
        >
          Add
        </button>
        <button
          className="text-[10px] px-2 py-0.5 rounded bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
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
    <div className="border-t border-border">
      {/* Header */}
      <button
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-bg-tertiary transition-colors"
        onClick={() => setExpanded(prev => !prev)}
      >
        <span className="text-[10px] font-semibold uppercase tracking-wider text-text-secondary flex items-center gap-1">
          <ListTodo size={11} />
          Tasks
          {activeTasks.length > 0 && (
            <span className="ml-1 text-[9px] px-1 py-0.5 rounded-full bg-accent-blue/20 text-accent-blue">
              {activeTasks.length}
            </span>
          )}
        </span>
        <div className="flex items-center gap-1">
          <button
            className="p-0.5 rounded hover:bg-bg-secondary text-text-secondary hover:text-text-primary transition-colors"
            onClick={(e) => {
              e.stopPropagation()
              refresh()
            }}
            title="Refresh tasks"
          >
            <RefreshCw size={10} className={loading ? 'animate-spin' : ''} />
          </button>
          <button
            className="p-0.5 rounded hover:bg-bg-secondary text-text-secondary hover:text-accent-green transition-colors"
            onClick={(e) => {
              e.stopPropagation()
              setShowAddDialog(true)
              setExpanded(true)
            }}
            title="Add task"
          >
            <Plus size={12} />
          </button>
          {expanded ? <ChevronUp size={12} className="text-text-secondary" /> : <ChevronDown size={12} className="text-text-secondary" />}
        </div>
      </button>

      {/* Task list */}
      {expanded && (
        <div className="max-h-[200px] overflow-y-auto">
          {displayTasks.length === 0 ? (
            <div className="px-3 py-3 text-xs text-text-secondary italic text-center">
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
