use chrono::Utc;
use shared::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{CreateWorkspace, UpdateWorkspace, Workspace, WorkspaceDetail};

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
