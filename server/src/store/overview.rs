//! Provides aggregate data for the control API.
//! Efficient database aggregates for the desktop overview.

use std::collections::BTreeMap;

use chrono::Utc;
use sqlx::Row;

use crate::{
    model::{ModelUsage, Overview, OverviewMetrics, TokenUsageBucket, TokenUsageGranularity},
    Result,
};

use super::Store;

const OVERVIEW_DAYS: u64 = 365;
const MAX_RANGE_BUCKETS: i64 = 60;
const MAX_EXPLICIT_BUCKETS: i64 = 1440;
const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

#[cfg(test)]
mod tests;

impl Store {
    pub async fn overview(
        &self,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        model_hashes: Option<&str>,
        bucket_ms: Option<i64>,
    ) -> Result<Overview> {
        // Keep the breakdown and series on one read snapshot so a call that finishes during this
        // request cannot make the two views disagree.
        let mut tx = self.pool.begin().await?;
        // One grouped query supplies both the total and its breakdown, even while calls finish.
        let rows = sqlx::query(&format!(
            "WITH filtered AS (
                SELECT *, ROW_NUMBER() OVER (
                    PARTITION BY model_hash, model_id, provider_type, provider_url
                    ORDER BY created_at_ms DESC, call_id DESC
                ) AS latest_rank
                FROM llm_calls
                WHERE (? IS NULL OR created_at_ms >= ?)
                  AND (? IS NULL OR created_at_ms < ?)
                  AND (? IS NULL OR model_hash IN (SELECT value FROM json_each(?)))
            )
            SELECT model_hash, model_id,
                MAX(CASE WHEN latest_rank = 1 THEN display_name END) AS display_name,
                provider_type, provider_url,
                SUM(status != 'running') AS llm_calls,
                SUM(status = 'completed') AS successful_calls,
                SUM(status NOT IN ('running', 'completed')) AS failed_calls,
                COALESCE(SUM({fresh_input}), 0) AS input_tokens,
                COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens,
                COALESCE(SUM(cache_write_tokens), 0) AS cache_write_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens
             FROM filtered
             GROUP BY model_hash, model_id, provider_type, provider_url
             ORDER BY model_id, provider_url, provider_type, model_hash",
            fresh_input = fresh_input_sql(),
        ))
        .bind(start_ms)
        .bind(start_ms)
        .bind(end_ms)
        .bind(end_ms)
        .bind(model_hashes)
        .bind(model_hashes)
        .fetch_all(&mut *tx)
        .await?;
        let mut model_usage = Vec::with_capacity(rows.len());
        let mut metrics = OverviewMetrics::default();
        for row in rows {
            let input_tokens = non_negative(row.try_get("input_tokens")?);
            let cache_read_tokens = non_negative(row.try_get("cache_read_tokens")?);
            let cache_write_tokens = non_negative(row.try_get("cache_write_tokens")?);
            let output_tokens = non_negative(row.try_get("output_tokens")?);
            let prompt_tokens =
                saturating_sum(&[input_tokens, cache_read_tokens, cache_write_tokens]);
            let usage = OverviewMetrics {
                llm_calls: row.try_get("llm_calls")?,
                successful_calls: row.try_get("successful_calls")?,
                failed_calls: row.try_get("failed_calls")?,
                token_usage: prompt_tokens.saturating_add(output_tokens),
                prompt_tokens,
                input_tokens,
                cache_read_tokens,
                cache_write_tokens,
                output_tokens,
            };
            metrics.llm_calls = metrics.llm_calls.saturating_add(usage.llm_calls);
            metrics.successful_calls = metrics
                .successful_calls
                .saturating_add(usage.successful_calls);
            metrics.failed_calls = metrics.failed_calls.saturating_add(usage.failed_calls);
            metrics.token_usage = metrics.token_usage.saturating_add(usage.token_usage);
            metrics.prompt_tokens = metrics.prompt_tokens.saturating_add(usage.prompt_tokens);
            metrics.input_tokens = metrics.input_tokens.saturating_add(usage.input_tokens);
            metrics.cache_read_tokens = metrics
                .cache_read_tokens
                .saturating_add(usage.cache_read_tokens);
            metrics.cache_write_tokens = metrics
                .cache_write_tokens
                .saturating_add(usage.cache_write_tokens);
            metrics.output_tokens = metrics.output_tokens.saturating_add(usage.output_tokens);
            model_usage.push(ModelUsage {
                model_hash: row.try_get("model_hash")?,
                model_id: row.try_get("model_id")?,
                display_name: row.try_get("display_name")?,
                provider_type: row.try_get("provider_type")?,
                provider_url: row.try_get("provider_url")?,
                metrics: usage,
            });
        }
        model_usage.sort_by_key(|row| std::cmp::Reverse(row.metrics.token_usage));

        let (token_usage_granularity, bucket_ms, series_start_ms, bucket_count) =
            token_usage_buckets(start_ms, end_ms, bucket_ms);
        let rows = sqlx::query(&format!(
            "SELECT
                (created_at_ms / {bucket_ms}) * {bucket_ms} AS bucket_start_ms,
                COALESCE(SUM({fresh_input}), 0) AS input_tokens,
                COALESCE(SUM(COALESCE(cache_read_tokens, 0)), 0) AS cache_read_tokens,
                COALESCE(SUM(COALESCE(cache_write_tokens, 0)), 0) AS cache_write_tokens,
                COALESCE(SUM(COALESCE(output_tokens, 0)), 0) AS output_tokens
             FROM llm_calls
             WHERE created_at_ms >= ?
               AND (? IS NULL OR created_at_ms < ?)
               AND (? IS NULL OR model_hash IN (SELECT value FROM json_each(?)))
             GROUP BY bucket_start_ms
             ORDER BY bucket_start_ms",
            fresh_input = fresh_input_sql(),
        ))
        .bind(start_ms.unwrap_or(series_start_ms).max(series_start_ms))
        .bind(end_ms)
        .bind(end_ms)
        .bind(model_hashes)
        .bind(model_hashes)
        .fetch_all(&mut *tx)
        .await?;
        let mut recorded = rows
            .into_iter()
            .map(|row| {
                let bucket_start_ms: i64 = row.try_get("bucket_start_ms")?;
                Ok((
                    bucket_start_ms,
                    TokenUsageBucket {
                        bucket_start_ms,
                        input_tokens: non_negative(row.try_get("input_tokens")?),
                        cache_read_tokens: non_negative(row.try_get("cache_read_tokens")?),
                        cache_write_tokens: non_negative(row.try_get("cache_write_tokens")?),
                        output_tokens: non_negative(row.try_get("output_tokens")?),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let token_usage_series = (0..bucket_count)
            .map(|offset| series_start_ms.saturating_add(offset.saturating_mul(bucket_ms)))
            .map(|bucket_start_ms| {
                recorded
                    .remove(&bucket_start_ms)
                    .unwrap_or(TokenUsageBucket {
                        bucket_start_ms,
                        ..TokenUsageBucket::default()
                    })
            })
            .collect();

        tx.commit().await?;

        Ok(Overview {
            metrics,
            model_usage,
            token_usage_granularity,
            token_usage_series,
        })
    }
}

fn token_usage_buckets(
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    requested_bucket_ms: Option<i64>,
) -> (TokenUsageGranularity, i64, i64, i64) {
    if let (Some(start_ms), Some(end_ms)) = (start_ms, end_ms) {
        let duration_ms = end_ms.saturating_sub(start_ms).max(1);
        let explicit_bucket_ms = requested_bucket_ms.filter(|bucket_ms| *bucket_ms >= MINUTE_MS);
        let bucket_ms = explicit_bucket_ms.unwrap_or({
            if duration_ms <= HOUR_MS {
                MINUTE_MS
            } else if duration_ms <= MAX_RANGE_BUCKETS * HOUR_MS {
                HOUR_MS
            } else {
                DAY_MS
            }
        });
        let granularity = if bucket_ms < HOUR_MS {
            TokenUsageGranularity::Minute
        } else if bucket_ms < DAY_MS {
            TokenUsageGranularity::Hour
        } else {
            TokenUsageGranularity::Day
        };
        let max_buckets = if explicit_bucket_ms.is_some() {
            MAX_EXPLICIT_BUCKETS
        } else {
            MAX_RANGE_BUCKETS
        };
        let last_bucket_ms = end_ms.saturating_sub(1).div_euclid(bucket_ms) * bucket_ms;
        let first_bucket_ms = start_ms.div_euclid(bucket_ms) * bucket_ms;
        let bucket_count =
            ((last_bucket_ms - first_bucket_ms).div_euclid(bucket_ms) + 1).clamp(1, max_buckets);
        let series_start_ms =
            last_bucket_ms.saturating_sub((bucket_count - 1).saturating_mul(bucket_ms));
        return (granularity, bucket_ms, series_start_ms, bucket_count);
    }

    let today_start_ms = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|value| value.and_utc().timestamp_millis())
        .unwrap_or(0);
    let series_start_ms = today_start_ms.saturating_sub(
        i64::try_from(OVERVIEW_DAYS - 1)
            .unwrap_or(0)
            .saturating_mul(DAY_MS),
    );
    (
        TokenUsageGranularity::Day,
        DAY_MS,
        series_start_ms,
        i64::try_from(OVERVIEW_DAYS).unwrap_or(0),
    )
}

fn fresh_input_sql() -> &'static str {
    "CASE
        WHEN request_type = 'anthropic' THEN MAX(0, COALESCE(input_tokens, 0))
        ELSE MAX(0, COALESCE(input_tokens, 0)
            - COALESCE(cache_read_tokens, 0)
            - COALESCE(cache_write_tokens, 0))
     END"
}

fn non_negative(value: i64) -> i64 {
    value.max(0)
}

fn saturating_sum(values: &[i64]) -> i64 {
    values
        .iter()
        .fold(0_i64, |total, value| total.saturating_add(*value))
}
