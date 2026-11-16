//! Shared execution context for recipe engine, HITL channels and sinks.
//!
//! It intentionally contains only core-owned services. The shell builds it from
//! its AppState and, in A3, injects the step dispatcher as well.

use crate::{paths::AppPaths, secrets::SecretStore};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct RecipeContext {
    pub pool: SqlitePool,
    pub paths: AppPaths,
    pub secrets: Arc<SecretStore>,
}

impl RecipeContext {
    pub fn new(pool: SqlitePool, paths: AppPaths, secrets: Arc<SecretStore>) -> Self {
        Self {
            pool,
            paths,
            secrets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_from_core_services() {
        let ctx = crate::test_support::context().await;
        let secrets = Arc::new(SecretStore::new(&ctx.paths));
        let context = RecipeContext::new(ctx.db.clone(), ctx.paths.clone(), secrets);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT 1")
                .fetch_one(&context.pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(context.paths.data_dir, ctx.paths.data_dir);
    }
}
