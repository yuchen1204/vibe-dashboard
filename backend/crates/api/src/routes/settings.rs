use axum::{extract::State, http::StatusCode, Json};

use crate::error::AppResult;
use crate::state::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LlmConfigInput {
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LlmConfigResponse {
    pub api_base: String,
    pub model: String,
    pub configured: bool,
}

/// GET /api/settings/llm - 获取当前 LLM 配置（不含 api_key）
pub async fn get_llm_config(State(state): State<AppState>) -> AppResult<Json<LlmConfigResponse>> {
    let (api_base, _api_key, model) = tasks::settings::get_llm_config(&state.db).await?;
    Ok(Json(LlmConfigResponse {
        api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        model: model.unwrap_or_else(|| "gpt-4o".to_string()),
        configured: state.llm_config.is_configured(),
    }))
}

/// PUT /api/settings/llm - 保存 LLM 配置
pub async fn set_llm_config(
    State(mut state): State<AppState>,
    Json(input): Json<LlmConfigInput>,
) -> AppResult<StatusCode> {
    tasks::settings::set_llm_config(
        &state.db,
        input.api_base.as_deref(),
        input.api_key.as_deref(),
        input.model.as_deref(),
    )
    .await?;

    // 更新内存中的 llm_config
    if let Some(api_base) = input.api_base {
        state.llm_config.api_base = api_base;
    }
    if let Some(api_key) = input.api_key {
        state.llm_config.api_key = api_key;
    }
    if let Some(model) = input.model {
        state.llm_config.model = model;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/settings/llm - 清除 LLM 配置
pub async fn clear_llm_config(State(mut state): State<AppState>) -> AppResult<StatusCode> {
    let _ = tasks::settings::delete_setting(&state.db, tasks::settings::LLM_API_BASE_KEY).await;
    let _ = tasks::settings::delete_setting(&state.db, tasks::settings::LLM_API_KEY_KEY).await;
    let _ = tasks::settings::delete_setting(&state.db, tasks::settings::LLM_MODEL_KEY).await;

    // 重置为默认值
    state.llm_config = orchestrator::llm::LlmConfig::default();
    state.llm_config.api_key = String::new(); // 保证未配置

    Ok(StatusCode::NO_CONTENT)
}