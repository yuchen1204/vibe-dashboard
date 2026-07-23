#![allow(dead_code)]

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub db_path: String,
    pub http_port: u16,
    pub log_level: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("env var error: {0}")]
    Env(#[from] env::VarError),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db_path = env::var("VIBE_DB_PATH").unwrap_or_else(|_| default_db_path());
        let http_port: u16 = env::var("VIBE_HTTP_PORT")
            .unwrap_or_else(|_| "8787".to_string())
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidPort(e.to_string()))?;
        let log_level = env::var("VIBE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        Ok(Self {
            db_path,
            http_port,
            log_level,
        })
    }
}

fn default_db_path() -> String {
    if let Ok(appdata) = env::var("APPDATA") {
        format!("{}\\vibe-dashboard\\data.db", appdata)
    } else {
        "data.db".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_env() {
        env::remove_var("VIBE_DB_PATH");
        env::remove_var("VIBE_HTTP_PORT");
        env::remove_var("VIBE_LOG_LEVEL");
        env::remove_var("APPDATA");
    }

    #[serial]
    #[test]
    fn from_env_uses_defaults_when_unset() {
        clear_env();
        let cfg = Config::from_env().expect("defaults");
        assert_eq!(cfg.http_port, 8787);
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.db_path.ends_with("data.db"));
    }

    #[serial]
    #[test]
    fn from_env_reads_overrides() {
        clear_env();
        env::set_var("VIBE_DB_PATH", "/tmp/test.db");
        env::set_var("VIBE_HTTP_PORT", "9999");
        env::set_var("VIBE_LOG_LEVEL", "debug");
        let cfg = Config::from_env().expect("overrides");
        assert_eq!(cfg.db_path, "/tmp/test.db");
        assert_eq!(cfg.http_port, 9999);
        assert_eq!(cfg.log_level, "debug");
        clear_env();
    }

    #[serial]
    #[test]
    fn from_env_rejects_invalid_port() {
        clear_env();
        env::set_var("VIBE_HTTP_PORT", "not-a-port");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPort(_)));
        clear_env();
    }

    #[serial]
    #[test]
    fn default_db_path_uses_appdata_on_windows() {
        clear_env();
        env::set_var("APPDATA", "C:\\Users\\test\\AppData\\Roaming");
        let cfg = Config::from_env().expect("appdata");
        assert_eq!(
            cfg.db_path,
            "C:\\Users\\test\\AppData\\Roaming\\vibe-dashboard\\data.db"
        );
        clear_env();
    }
}
