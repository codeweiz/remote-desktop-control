import { useState, useEffect, useCallback } from 'react'
import type { Task, TaskCreateRequest } from '../lib/types'
import { getTasks, createTask, updateTask } from '../lib/api'

export function useTaskPool() {
  const [tasks, setTasks] = useState<Task[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchTasks = useCallback(async () => {
    try {
      setLoading(true)
      const data = await getTasks()
      setTasks(data)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch tasks')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchTasks()
  }, [fetchTasks])

  const addTask = useCallback(async (req: TaskCreateRequest) => {
    try {
      const task = await createTask(req)
      setTasks(prev => [...prev, task])
      return task
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create task')
      throw err
    }
  }, [])

  const approveTask = useCallback(async (id: string) => {
    try {
      const updated = await updateTask(id, { status: 'Completed' })
      setTasks(prev => prev.map(t => t.id === id ? updated : t))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve task')
    }
  }, [])

  const cancelTask = useCallback(async (id: string) => {
    try {
      const updated = await updateTask(id, { status: 'Cancelled' })
      setTasks(prev => prev.map(t => t.id === id ? updated : t))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to cancel task')
    }
  }, [])

  return {
    tasks,
    loading,
    error,
    addTask,
    approveTask,
    cancelTask,
    refresh: fetchTasks,
  }
}
