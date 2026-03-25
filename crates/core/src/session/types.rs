use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type SessionId = String; // nanoid 12-char

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: SessionId,
    pub name: String,
    pub session_type: SessionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    pub cwd: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub last_seq: u64,
    pub status: SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<SessionId>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Terminal,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Idle,
    Exited,
    Suspended,
    Crashed,
}

/// Event stored in events.jsonl
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub seq: u64,
    #[serde(rename = "t")]
    pub event_type: String,
    pub ts: i64,
    #[serde(flatten)]
    pub data: serde_json::Value,
}
