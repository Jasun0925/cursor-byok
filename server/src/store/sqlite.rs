//! Initializes and configures SQLite storage.
use std::{str::FromStr, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Connection, SqlitePool,
};

use crate::Result;

use super::{migrations, writer::WriteCoordinator};

#[derive(Clone)]
pub struct Store {
    pub(crate) pool: SqlitePool,
    pub(crate) writes: WriteCoordinator,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));
        let database_path = options.get_filename().to_owned();
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .after_release(|connection, _| {
                Box::pin(async move {
                    if connection.is_in_transaction() {
                        // Transaction::drop queues a rollback on SQLite's worker.
                        // Flush it before deciding whether this connection is clean.
                        connection.ping().await?;
                    }
                    // SQLite can roll back automatically while SQLx retains its
                    // depth. A successful ping alone does not detect that state.
                    if connection.is_in_transaction() {
                        tracing::warn!("discarding SQLite connection with unfinished transaction");
                        return Ok(false);
                    }
                    Ok(true)
                })
            })
            .connect_with(options)
            .await?;
        migrations::run(&pool, &database_path).await?;
        Ok(Self {
            pool,
            writes: WriteCoordinator::default(),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
