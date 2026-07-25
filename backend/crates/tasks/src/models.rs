use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceDetail {
    #[serde(flatten)]
    pub workspace: Workspace,
    pub target_count: i64,
    pub todo_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateWorkspace {
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    #[default]
    Planned,
    Active,
    Done,
    Archived,
}

impl TargetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetStatus::Planned => "planned",
            TargetStatus::Active => "active",
            TargetStatus::Done => "done",
            TargetStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Target {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTarget {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTarget {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TargetStatus>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Todo,
    Doing,
    Done,
    Blocked,
}

impl TodoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TodoStatus::Todo => "todo",
            TodoStatus::Doing => "doing",
            TodoStatus::Done => "done",
            TodoStatus::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Todo {
    pub id: String,
    pub target_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTodo {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTodo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TodoStatus>,
    pub sort_order: Option<i64>,
}
