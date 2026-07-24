use shared::AppResult;
use sqlx::SqlitePool;

use crate::models::Workspace;

pub async fn list_workspaces(_pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    Ok(Vec::new())
}
