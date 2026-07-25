use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub hub: Arc<Hub>,
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new(db: SqlitePool, hub: Arc<Hub>, config: Config) -> Self {
        Self {
            db,
            hub,
            config: Arc::new(config),
            started_at: Utc::now(),
        }
    }
}
