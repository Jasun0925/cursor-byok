//! Row accounting and cleanup for disposable observability data.

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

use super::Store;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct StatisticsStorage {
    pub call_count: i64,
    pub trace_count: i64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StatisticsStorageScope {
    #[default]
    Details,
    All,
}

impl Store {
    pub async fn statistics_storage(&self) -> Result<StatisticsStorage> {
        let (call_count, trace_count) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM llm_calls), (SELECT COUNT(*) FROM cursor_run_traces)",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(StatisticsStorage {
            call_count,
            trace_count,
        })
    }

    pub async fn clear_statistics_storage(&self) -> Result<StatisticsStorage> {
        self.clear_statistics(StatisticsStorageScope::Details).await
    }

    pub async fn clear_all_statistics_storage(&self) -> Result<StatisticsStorage> {
        self.clear_statistics(StatisticsStorageScope::All).await
    }

    async fn clear_statistics(&self, scope: StatisticsStorageScope) -> Result<StatisticsStorage> {
        // A large cleanup must finish reclaiming the files even if the settings
        // page disconnects after the deletion transaction has committed.
        let store = self.clone();
        tokio::spawn(async move { store.clear_statistics_inner(scope).await })
            .await
            .map_err(|error| Error::Store(format!("statistics cleanup task failed: {error}")))?
    }

    async fn clear_statistics_inner(
        &self,
        scope: StatisticsStorageScope,
    ) -> Result<StatisticsStorage> {
        // Hold the existing write coordinator through deletion and compaction so
        // new trace chunks and conversation writes resume only after maintenance.
        let _write = self.writes.lock().await;
        let mut transaction = self.pool.begin().await?;
        match scope {
            StatisticsStorageScope::Details => {
                Self::clear_detail_storage_tx(&mut transaction).await?;
            }
            StatisticsStorageScope::All => {
                Self::clear_trace_artifacts_tx(&mut transaction).await?;
                sqlx::query("DELETE FROM llm_calls")
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query("DELETE FROM cursor_run_traces")
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        transaction.commit().await?;
        self.reclaim_storage_space().await.map_err(|error| {
            Error::Store(format!(
                "statistics records were cleared, but disk space reclamation did not finish: {error}"
            ))
        })?;
        self.statistics_storage().await
    }

    async fn reclaim_storage_space(&self) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        // DELETE leaves reusable pages in the database. VACUUM must run outside
        // the deletion transaction; the final checkpoint also releases the WAL.
        sqlx::query("VACUUM main").execute(&mut *connection).await?;
        let (busy, _, _) =
            sqlx::query_as::<_, (i64, i64, i64)>("PRAGMA main.wal_checkpoint(TRUNCATE)")
                .fetch_one(&mut *connection)
                .await?;
        if busy != 0 {
            return Err(Error::Store(
                "database readers prevented WAL truncation; close other database readers and retry cleanup"
                    .into(),
            ));
        }
        Ok(())
    }

    async fn clear_detail_storage_tx(
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<()> {
        sqlx::query("DELETE FROM llm_call_requests")
            .execute(&mut **transaction)
            .await?;
        sqlx::query("DELETE FROM llm_call_response_chunks")
            .execute(&mut **transaction)
            .await?;
        Self::clear_trace_artifacts_tx(transaction).await
    }

    async fn clear_trace_artifacts_tx(
        transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<()> {
        sqlx::query(
            "CREATE TEMP TABLE IF NOT EXISTS clear_statistics_blob_ids(
                blob_id BLOB PRIMARY KEY
             )",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query("DELETE FROM clear_statistics_blob_ids")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT OR IGNORE INTO clear_statistics_blob_ids(blob_id)
             SELECT blob_id FROM cursor_run_trace_artifacts",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query("DELETE FROM cursor_run_trace_artifacts")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "DELETE FROM blobs
             WHERE blob_id IN (SELECT blob_id FROM clear_statistics_blob_ids)
               AND NOT EXISTS (
                   SELECT 1 FROM cursor_run_trace_artifacts a WHERE a.blob_id = blobs.blob_id
               )
               AND NOT EXISTS (
                   SELECT 1 FROM blob_edges e
                   WHERE e.parent_blob_id = blobs.blob_id OR e.child_blob_id = blobs.blob_id
               )",
        )
        .execute(&mut **transaction)
        .await?;
        sqlx::query("DROP TABLE clear_statistics_blob_ids")
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }
}
