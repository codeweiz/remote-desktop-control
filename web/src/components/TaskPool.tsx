import { useState, useCallback } from 'react'
import {
  Box,
  Typography,
  IconButton,
  Chip,
  Collapse,
  TextField,
  Button,
  Divider,
} from '@mui/material'
import {
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
  Add as AddIcon,
  CheckCircle as CheckCircleIcon,
  Cancel as CancelIcon,
  Delete as DeleteIcon,
  Checklist as TaskListIcon,
  Refresh as RefreshIcon,
} from '@mui/icons-material'
import type { Task, TaskPriority, TaskStatus } from '../lib/types'
import { useTaskPool } from '../hooks/useTaskPool'

function priorityColor(priority: TaskPriority): string {
  switch (priority) {
    case 'P0': return '#f87171'
    case 'P1': return '#fbbf24'
    case 'P2': return '#34d399'
    case 'P3': return '#3b82f6'
  }
}

function statusColor(status: TaskStatus): string {
  switch (status) {
    case 'Pending': return '#94a3b8'
    case 'InProgress': return '#3b82f6'
    case 'NeedsReview': return '#fbbf24'
    case 'Completed': return '#34d399'
    case 'Cancelled': return '#64748b'
  }
}

function TaskItem({
  task,
  onApprove,
  onCancel,
  onDelete,
}: {
  task: Task
  onApprove: () => void
  onCancel: () => void
  onDelete: () => void
}) {
  const pColor = priorityColor(task.priority)
  const sColor = statusColor(task.status)

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'flex-start',
        gap: 1,
        px: 1.5,
        py: 0.75,
        '&:hover': { bgcolor: 'rgba(255,255,255,0.03)' },
        '&:hover .task-actions': { opacity: 1 },
        transition: 'background-color 0.15s',
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.75 }}>
          <Chip
            size="small"
            label={task.priority}
            sx={{
              height: 18,
              fontSize: 9,
              fontWeight: 700,
              bgcolor: `${pColor}20`,
              color: pColor,
              '& .MuiChip-label': { px: 0.75 },
            }}
          />
          <Typography
            variant="caption"
            sx={{
              fontSize: 11,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {task.title}
          </Typography>
        </Box>
        <Chip
          size="small"
          label={task.status}
          sx={{
            mt: 0.25,
            height: 16,
            fontSize: 9,
            fontFamily: "'JetBrains Mono', monospace",
            bgcolor: `${sColor}20`,
            color: sColor,
            '& .MuiChip-label': { px: 0.5 },
            textDecoration: task.status === 'Cancelled' ? 'line-through' : 'none',
          }}
        />
      </Box>
      <Box
        className="task-actions"
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: 0.25,
          opacity: 0,
          transition: 'opacity 0.15s',
          flexShrink: 0,
        }}
      >
        {task.status === 'NeedsReview' && (
          <IconButton size="small" onClick={onApprove} title="Approve" sx={{ p: 0.25 }}>
            <CheckCircleIcon sx={{ fontSize: 14, color: 'success.main' }} />
          </IconButton>
        )}
        {task.status !== 'Completed' && task.status !== 'Cancelled' && (
          <IconButton size="small" onClick={onCancel} title="Cancel" sx={{ p: 0.25 }}>
            <CancelIcon sx={{ fontSize: 14, color: 'error.main' }} />
          </IconButton>
        )}
        <IconButton size="small" onClick={onDelete} title="Delete" sx={{ p: 0.25 }}>
          <DeleteIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
        </IconButton>
      </Box>
    </Box>
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
    <Box sx={{ px: 1.5, py: 1, borderTop: '1px solid', borderColor: 'divider' }}>
      <TextField
        size="small"
        fullWidth
        placeholder="Task title..."
        value={title}
        onChange={e => setTitle(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') handleSubmit()
          if (e.key === 'Escape') onClose()
        }}
        autoFocus
        sx={{
          mb: 1,
          '& .MuiOutlinedInput-root': { fontSize: 12 },
        }}
      />
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
        {(['P0', 'P1', 'P2', 'P3'] as TaskPriority[]).map(p => (
          <Chip
            key={p}
            size="small"
            label={p}
            onClick={() => setPriority(p)}
            sx={{
              height: 20,
              fontSize: 9,
              fontWeight: 700,
              cursor: 'pointer',
              bgcolor: priority === p ? `${priorityColor(p)}30` : 'rgba(255,255,255,0.05)',
              color: priority === p ? priorityColor(p) : 'text.secondary',
              '& .MuiChip-label': { px: 0.75 },
            }}
          />
        ))}
        <Box sx={{ flex: 1 }} />
        <Button size="small" onClick={handleSubmit} sx={{ fontSize: 10, minWidth: 0, px: 1 }}>
          Add
        </Button>
        <Button
          size="small"
          onClick={onClose}
          sx={{ fontSize: 10, minWidth: 0, px: 1, color: 'text.secondary' }}
        >
          Cancel
        </Button>
      </Box>
    </Box>
  )
}

export function TaskPool() {
  const { tasks, loading, addTask, approveTask, cancelTask, removeTask, refresh } = useTaskPool()
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
    <Box>
      {/* Header */}
      <Box
        onClick={() => setExpanded(prev => !prev)}
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          px: 1.5,
          py: 1,
          cursor: 'pointer',
          '&:hover': { bgcolor: 'rgba(255,255,255,0.03)' },
          transition: 'background-color 0.15s',
        }}
      >
        <Typography
          variant="overline"
          sx={{
            fontSize: 10,
            letterSpacing: 1,
            color: 'text.secondary',
            display: 'flex',
            alignItems: 'center',
            gap: 0.75,
          }}
        >
          <TaskListIcon sx={{ fontSize: 13 }} />
          Tasks
          {activeTasks.length > 0 && (
            <Chip
              size="small"
              label={activeTasks.length}
              sx={{
                height: 16,
                fontSize: 9,
                fontWeight: 700,
                bgcolor: 'rgba(59,130,246,0.2)',
                color: '#3b82f6',
                '& .MuiChip-label': { px: 0.75 },
              }}
            />
          )}
        </Typography>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.25 }}>
          <IconButton
            size="small"
            onClick={(e) => {
              e.stopPropagation()
              refresh()
            }}
            title="Refresh tasks"
            sx={{ p: 0.25 }}
          >
            <RefreshIcon
              sx={{
                fontSize: 12,
                animation: loading ? 'spin 1s linear infinite' : 'none',
                '@keyframes spin': {
                  from: { transform: 'rotate(0deg)' },
                  to: { transform: 'rotate(360deg)' },
                },
              }}
            />
          </IconButton>
          <IconButton
            size="small"
            onClick={(e) => {
              e.stopPropagation()
              setShowAddDialog(true)
              setExpanded(true)
            }}
            title="Add task"
            sx={{ p: 0.25 }}
          >
            <AddIcon sx={{ fontSize: 14 }} />
          </IconButton>
          {expanded ? (
            <ExpandLessIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
          ) : (
            <ExpandMoreIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
          )}
        </Box>
      </Box>

      {/* Task list */}
      <Collapse in={expanded}>
        <Box sx={{ maxHeight: 200, overflow: 'auto' }}>
          {displayTasks.length === 0 ? (
            <Typography
              variant="caption"
              sx={{
                display: 'block',
                textAlign: 'center',
                py: 2,
                fontStyle: 'italic',
                color: 'text.secondary',
                fontSize: 11,
              }}
            >
              {loading ? 'Loading tasks...' : 'No tasks'}
            </Typography>
          ) : (
            displayTasks.map(task => (
              <TaskItem
                key={task.id}
                task={task}
                onApprove={() => approveTask(task.id)}
                onCancel={() => cancelTask(task.id)}
                onDelete={() => removeTask(task.id)}
              />
            ))
          )}
        </Box>
      </Collapse>

      {/* Add task dialog */}
      {showAddDialog && (
        <AddTaskDialog
          onSubmit={handleAddTask}
          onClose={() => setShowAddDialog(false)}
        />
      )}
    </Box>
  )
}
