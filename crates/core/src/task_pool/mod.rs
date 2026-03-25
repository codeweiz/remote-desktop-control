//! Task Pool — queued task management and scheduling.
//!
//! Provides an in-memory task pool backed by `tasks.jsonl` for persistence,
//! with priority-based FIFO scheduling, dependency tracking, and automatic
//! task execution when the system is idle.

pub mod pool;
pub mod scheduler;
pub mod types;
