use sqlx::{Sqlite, SqlitePool, Transaction};

/// Serialized write side.
///
/// The pool is opened with a single connection (`Storage::open`), so every
/// transaction is serialized by SQLite itself. Readers use the separate
/// `Storage::read` pool and never queue behind a writer.
#[derive(Clone)]
pub struct Writer {
    pool: SqlitePool,
}

impl Writer {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn transaction(&self) -> sqlx::Result<Transaction<'_, Sqlite>> {
        self.pool.begin().await
    }

    /// Runs several statements in one `BEGIN IMMEDIATE` transaction. The
    /// connection is already exclusive, so this only makes the intent explicit
    /// and takes the write lock before the first statement.
    pub async fn immediate<T, F, Fut>(&self, run: F) -> sqlx::Result<T>
    where
        F: FnOnce(&mut sqlx::SqliteConnection) -> Fut,
        Fut: std::future::Future<Output = sqlx::Result<T>>,
    {
        let mut connection = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await?;
        match run(&mut connection).await {
            Ok(value) => {
                sqlx::query("COMMIT").execute(&mut *connection).await?;
                Ok(value)
            }
            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                Err(error)
            }
        }
    }
}
