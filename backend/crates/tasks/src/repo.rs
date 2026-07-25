use chrono::Utc;
use shared::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{
    CreateTarget, CreateWorkspace, Target, TargetStatus, UpdateTarget, UpdateWorkspace, Workspace,
    WorkspaceDetail,
};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub async fn list_workspaces(pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    let rows = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces
           ORDER BY updated_at DESC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_workspace(pool: &SqlitePool, id: &str) -> AppResult<WorkspaceDetail> {
    let workspace = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces
           WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("workspace {id} not found")))?;

    let (target_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM targets WHERE workspace_id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?;
    let (todo_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM todos t
           JOIN targets tg ON t.target_id = tg.id
           WHERE tg.workspace_id = ?"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(WorkspaceDetail {
        workspace,
        target_count,
        todo_count,
    })
}

pub async fn create_workspace(pool: &SqlitePool, input: CreateWorkspace) -> AppResult<Workspace> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if input.path.trim().is_empty() {
        return Err(AppError::BadRequest("path must not be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    sqlx::query!(
        r#"INSERT INTO workspaces (id, name, path, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?)"#,
        id,
        input.name,
        input.path,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Workspace {
        id,
        name: input.name,
        path: input.path,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_workspace(
    pool: &SqlitePool,
    id: &str,
    input: UpdateWorkspace,
) -> AppResult<Workspace> {
    let existing = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("workspace {id} not found")))?;

    let name = input.name.unwrap_or(existing.name);
    let path = input.path.unwrap_or(existing.path);

    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if path.trim().is_empty() {
        return Err(AppError::BadRequest("path must not be empty".into()));
    }

    let now = now_rfc3339();
    sqlx::query!(
        r#"UPDATE workspaces SET name = ?, path = ?, updated_at = ? WHERE id = ?"#,
        name,
        path,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Workspace {
        id: existing.id,
        name,
        path,
        created_at: existing.created_at,
        updated_at: now,
    })
}

pub async fn delete_workspace(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM workspaces WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("workspace {id} not found")));
    }
    Ok(())
}

pub async fn list_targets(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Target>> {
    let rows = sqlx::query_as!(
        Target,
        r#"SELECT id, workspace_id, title, description, status, sort_order, created_at, updated_at
           FROM targets
           WHERE workspace_id = ?
           ORDER BY sort_order ASC, created_at ASC"#,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_target(pool: &SqlitePool, id: &str) -> AppResult<Target> {
    sqlx::query_as!(
        Target,
        r#"SELECT id, workspace_id, title, description, status, sort_order, created_at, updated_at
           FROM targets WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("target {id} not found")))
}

pub async fn create_target(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateTarget,
) -> AppResult<Target> {
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM workspaces WHERE id = ?")
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!(
            "workspace {workspace_id} not found"
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = TargetStatus::default().as_str();
    sqlx::query!(
        r#"INSERT INTO targets (id, workspace_id, title, description, status, sort_order, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 0, ?, ?)"#,
        id,
        workspace_id,
        input.title,
        input.description,
        status,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Target {
        id,
        workspace_id: workspace_id.to_string(),
        title: input.title,
        description: input.description,
        status: status.to_string(),
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_target(pool: &SqlitePool, id: &str, input: UpdateTarget) -> AppResult<Target> {
    let existing = get_target(pool, id).await?;

    let title = input.title.unwrap_or(existing.title);
    let description = input.description.unwrap_or(existing.description);
    let status = input
        .status
        .map(|s| s.as_str().to_string())
        .unwrap_or(existing.status);
    let sort_order = input.sort_order.unwrap_or(existing.sort_order);

    if title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }

    let now = now_rfc3339();
    sqlx::query!(
        r#"UPDATE targets SET title = ?, description = ?, status = ?, sort_order = ?, updated_at = ?
           WHERE id = ?"#,
        title,
        description,
        status,
        sort_order,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Target {
        id: existing.id,
        workspace_id: existing.workspace_id,
        title,
        description,
        status,
        sort_order,
        created_at: existing.created_at,
        updated_at: now,
    })
}

pub async fn delete_target(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM targets WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("target {id} not found")));
    }
    Ok(())
}
