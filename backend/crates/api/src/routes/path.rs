use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct PathQuery {
    q: Option<String>,
}

pub async fn path_suggest(
    State(_state): State<AppState>,
    Query(params): Query<PathQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let prefix = params.q.unwrap_or_default();

    let candidates = if prefix.is_empty() {
        // 没有输入时返回常见根目录
        let drives = get_drives();
        let mut dirs: Vec<String> = drives.into_iter().map(|d| format!("{d}\\")).collect();
        dirs.sort();
        dirs
    } else {
        let (base_dir, partial) = split_path(&prefix);
        let mut results = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().starts_with(&partial.to_lowercase()) {
                        let full = format!("{}{}", base_dir, name);
                        // 追加反斜杠表示目录
                        results.push(format!("{}{}", full, if full.ends_with('\\') || full.ends_with('/') { "" } else { "\\" }));
                    }
                }
            }
        }
        results.sort();
        results
    };

    Ok(Json(json!({ "paths": candidates })))
}

fn split_path(input: &str) -> (String, String) {
    let input = input.replace('/', "\\");
    if let Some(pos) = input.rfind('\\') {
        let base = input[..=pos].to_string();
        let partial = input[pos + 1..].to_string();
        (base, partial)
    } else {
        // 输入不含反斜杠，视作当前盘符下搜索
        ("".to_string(), input.to_string())
    }
}

fn get_drives() -> Vec<String> {
    let mut drives = Vec::new();
    for letter in 'A'..='Z' {
        let drive = format!("{letter}:");
        if Path::new(&format!("{drive}\\")).exists() {
            drives.push(drive);
        }
    }
    drives
}