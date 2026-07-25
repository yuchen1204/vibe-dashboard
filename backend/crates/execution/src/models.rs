use serde::{Deserialize, Serialize};

// ---------- Worktree ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    #[default]
    Active,
    Merged,
    Abandoned,
}

impl WorktreeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            WorktreeStatus::Active => "active",
            WorktreeStatus::Merged => "merged",
            WorktreeStatus::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Worktree {
    pub id: String,
    pub workspace_id: String,
    pub target_id: Option<String>,
    pub branch: String,
    pub path: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorktree {
    pub branch: String,
    pub target_id: Option<String>,
}

// ---------- ExecutionJob ----------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Success => "success",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ExecutionJob {
    pub id: String,
    pub todo_id: String,
    pub worktree_id: Option<String>,
    pub status: String,
    pub agent_type: String,
    pub prompt: String,
    pub output: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteTodo {
    #[serde(default)]
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateJob {
    pub todo_id: String,
    pub prompt: String,
    pub agent_type: Option<String>,
}

// ---------- WS 消息 ----------

#[derive(Debug, Clone, Serialize)]
pub struct JobOutputPayload {
    pub job_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobStatusPayload {
    pub job_id: String,
    pub todo_id: String,
    pub status: String,
}
