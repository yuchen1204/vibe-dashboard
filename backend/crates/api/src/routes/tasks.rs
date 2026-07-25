use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppResult;
use crate::state::AppState;
use tasks::{
    CreateTarget, CreateTodo, CreateWorkspace, UpdateTarget, UpdateTodo, UpdateWorkspace,
    WorkspaceDetail,
};

// ---------- Workspaces ----------

pub async fn list_workspaces(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<tasks::Workspace>>> {
    Ok(Json(tasks::repo::list_workspaces(&state.db).await?))
}

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(input): Json<CreateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    Ok(Json(tasks::repo::create_workspace(&state.db, input).await?))
}

pub async fn get_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<WorkspaceDetail>> {
    Ok(Json(tasks::repo::get_workspace(&state.db, &id).await?))
}

pub async fn update_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    Ok(Json(
        tasks::repo::update_workspace(&state.db, &id, input).await?,
    ))
}

pub async fn delete_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_workspace(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Targets ----------

pub async fn list_targets(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<tasks::Target>>> {
    Ok(Json(tasks::repo::list_targets(&state.db, &wid).await?))
}

pub async fn create_target(
    State(state): State<AppState>,
    Path(wid): Path<String>,
    Json(input): Json<CreateTarget>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(
        tasks::repo::create_target(&state.db, &wid, input).await?,
    ))
}

pub async fn get_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(tasks::repo::get_target(&state.db, &id).await?))
}

pub async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTarget>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(
        tasks::repo::update_target(&state.db, &id, input).await?,
    ))
}

pub async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_target(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Todos ----------

pub async fn list_todos_by_workspace(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<tasks::Todo>>> {
    Ok(Json(
        tasks::repo::list_todos_by_workspace(&state.db, &wid).await?,
    ))
}

pub async fn list_todos_by_target(
    State(state): State<AppState>,
    Path(tid): Path<String>,
) -> AppResult<Json<Vec<tasks::Todo>>> {
    Ok(Json(
        tasks::repo::list_todos_by_target(&state.db, &tid).await?,
    ))
}

pub async fn create_todo(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<CreateTodo>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(
        tasks::repo::create_todo(&state.db, &tid, input).await?,
    ))
}

pub async fn get_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(tasks::repo::get_todo(&state.db, &id).await?))
}

pub async fn update_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTodo>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(tasks::repo::update_todo(&state.db, &id, input).await?))
}

pub async fn delete_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_todo(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
