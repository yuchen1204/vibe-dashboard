use shared::{AppError, AppResult};
use sqlx::SqlitePool;

/// 获取一个设置值
pub async fn get_setting(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = ?"
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// 设置一个值（INSERT OR REPLACE）
pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)"
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| AppError::Internal(format!("failed to set setting: {e}")))?;
    Ok(())
}

/// 删除一个设置
pub async fn delete_setting(pool: &SqlitePool, key: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

/// LLM 配置键
pub const LLM_API_BASE_KEY: &str = "llm.api_base";
pub const LLM_API_KEY_KEY: &str = "llm.api_key";
pub const LLM_MODEL_KEY: &str = "llm.model";

/// 从 DB 读取 LLM 配置，返回 `(api_base, api_key, model)`
pub async fn get_llm_config(pool: &SqlitePool) -> AppResult<(Option<String>, Option<String>, Option<String>)> {
    let api_base = get_setting(pool, LLM_API_BASE_KEY).await?;
    let api_key = get_setting(pool, LLM_API_KEY_KEY).await?;
    let model = get_setting(pool, LLM_MODEL_KEY).await?;
    Ok((api_base, api_key, model))
}

/// 保存 LLM 配置
pub async fn set_llm_config(
    pool: &SqlitePool,
    api_base: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
) -> AppResult<()> {
    if let Some(v) = api_base {
        set_setting(pool, LLM_API_BASE_KEY, v).await?;
    }
    if let Some(v) = api_key {
        set_setting(pool, LLM_API_KEY_KEY, v).await?;
    }
    if let Some(v) = model {
        set_setting(pool, LLM_MODEL_KEY, v).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setting_roundtrip() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
        )
        .execute(&pool)
        .await
        .unwrap();

        set_setting(&pool, "test_key", "test_value").await.unwrap();
        let val = get_setting(&pool, "test_key").await.unwrap();
        assert_eq!(val, Some("test_value".to_string()));

        // overwrite
        set_setting(&pool, "test_key", "new_value").await.unwrap();
        let val = get_setting(&pool, "test_key").await.unwrap();
        assert_eq!(val, Some("new_value".to_string()));

        // get non-existent
        let val = get_setting(&pool, "no_such_key").await.unwrap();
        assert_eq!(val, None);
    }
}