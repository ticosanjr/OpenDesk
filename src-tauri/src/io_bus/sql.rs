use crate::error::{AppError, AppResult};
use sqlx::{AnyPool, any::AnyPoolOptions};

pub async fn connect(database_url: &str) -> AppResult<AnyPool> {
    sqlx::any::install_default_drivers();
    AnyPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|error| AppError::Daemon(format!("SQL connection: {error}")))
}
