use super::*;
use serde_json::{json, Value};

async fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::connect(&format!(
        "sqlite://{}",
        dir.path().join("test.db").display()
    ))
    .await
    .unwrap();
    (dir, store)
}

// Deliberately no model_configs row: historical usage must survive removed configurations.
async fn call(
    store: &Store,
    id: &str,
    model: Option<&str>,
    protocol: &str,
    status: &str,
    time: i64,
) {
    sqlx::query("INSERT INTO llm_calls (call_id, run_id, conversation_id, provider_call_index, model_hash, provider_type, provider_url, request_type, request_url, model_id, display_name, status, created_at_ms, request_started_at_ms, message_count, tool_count, detailed, input_tokens, cache_read_tokens, cache_write_tokens, output_tokens) VALUES (?, 'run', 'conversation', 0, ?, 'openai', 'https://one.example/v1', ?, 'https://one.example/v1/responses', 'same-model', 'Historical name', ?, ?, ?, 1, 0, 0, 100, 30, 10, 20)")
        .bind(id).bind(model).bind(protocol).bind(status).bind(time).bind(time)
        .execute(&store.pool).await.unwrap();
}

async fn overview(
    store: &Store,
    start: Option<i64>,
    end: Option<i64>,
    models: Option<&str>,
) -> Value {
    serde_json::to_value(store.overview(start, end, models, None).await.unwrap()).unwrap()
}

fn rows(value: &Value) -> &Vec<Value> {
    value["model_usage"]
        .as_array()
        .expect("overview must expose per-model usage")
}

#[tokio::test]
async fn model_totals_preserve_protocol_cache_and_running_call_semantics() {
    let (_dir, store) = store().await;
    call(&store, "openai", Some("a"), "openai", "completed", 1000).await;
    call(&store, "anthropic", Some("b"), "anthropic", "failed", 1100).await;
    call(&store, "running", Some("a"), "openai", "running", 1200).await;
    let value = overview(&store, None, None, None).await;
    assert_eq!(rows(&value).len(), 2);
    let a = rows(&value)
        .iter()
        .find(|r| r["model_hash"] == "a")
        .unwrap();
    let b = rows(&value)
        .iter()
        .find(|r| r["model_hash"] == "b")
        .unwrap();
    assert_eq!(
        a["metrics"],
        json!({"llm_calls":1,"successful_calls":1,"failed_calls":0,"token_usage":240,"prompt_tokens":200,"input_tokens":120,"cache_read_tokens":60,"cache_write_tokens":20,"output_tokens":40})
    );
    assert_eq!(b["metrics"]["input_tokens"], 100);
    assert_eq!(b["metrics"]["token_usage"], 160);
    assert_eq!(b["metrics"]["failed_calls"], 1);
    assert_eq!(value["metrics"]["llm_calls"], 2);
    assert_eq!(value["metrics"]["token_usage"], 400);
    for (key, total) in value["metrics"].as_object().unwrap() {
        let sum: i64 = rows(&value)
            .iter()
            .map(|r| r["metrics"][key].as_i64().unwrap())
            .sum();
        assert_eq!(total.as_i64().unwrap(), sum, "{key}");
    }
}

#[tokio::test]
async fn model_details_share_half_open_time_and_model_filters() {
    let (_dir, store) = store().await;
    for (id, time, model) in [
        ("before", 999, "a"),
        ("start", 1000, "a"),
        ("inside", 1500, "b"),
        ("end", 2000, "a"),
    ] {
        call(&store, id, Some(model), "openai", "completed", time).await;
    }
    let value = overview(&store, Some(1000), Some(2000), Some(r#"["a"]"#)).await;
    assert_eq!(rows(&value).len(), 1);
    assert_eq!(rows(&value)[0]["model_hash"], "a");
    assert_eq!(value["metrics"]["llm_calls"], 1);
    assert_eq!(value["metrics"]["token_usage"], 120);
    assert_eq!(value["token_usage_series"][0]["input_tokens"], 60);
    let empty = overview(&store, Some(1000), Some(2000), Some("[]")).await;
    assert!(rows(&empty).is_empty());
    assert_eq!(empty["metrics"]["token_usage"], 0);
}

#[tokio::test]
async fn historical_models_keep_distinct_provider_and_model_identities() {
    let (_dir, store) = store().await;
    call(
        &store,
        "original",
        Some("deleted"),
        "openai",
        "completed",
        1000,
    )
    .await;
    call(
        &store,
        "renamed",
        Some("deleted"),
        "openai",
        "completed",
        1100,
    )
    .await;
    sqlx::query("UPDATE llm_calls SET display_name = 'Zeta old' WHERE call_id = 'original'")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE llm_calls SET display_name = 'Alpha latest' WHERE call_id = 'renamed'")
        .execute(&store.pool)
        .await
        .unwrap();
    call(
        &store,
        "moved",
        Some("deleted"),
        "openai",
        "completed",
        1200,
    )
    .await;
    sqlx::query(
        "UPDATE llm_calls SET provider_url = 'https://two.example/v1' WHERE call_id = 'moved'",
    )
    .execute(&store.pool)
    .await
    .unwrap();
    call(
        &store,
        "changed-model",
        Some("deleted"),
        "openai",
        "completed",
        1300,
    )
    .await;
    sqlx::query("UPDATE llm_calls SET model_id = 'other-model' WHERE call_id = 'changed-model'")
        .execute(&store.pool)
        .await
        .unwrap();
    call(
        &store,
        "plugin",
        Some("plugin:test/model"),
        "openai",
        "completed",
        1400,
    )
    .await;
    call(&store, "unknown", None, "openai", "completed", 1500).await;
    let value = overview(&store, None, None, None).await;
    assert_eq!(rows(&value).len(), 5);
    assert_eq!(rows(&value)[0]["metrics"]["llm_calls"], 2);
    assert_eq!(rows(&value)[0]["provider_url"], "https://one.example/v1");
    assert_eq!(rows(&value)[0]["display_name"], "Alpha latest");
    assert!(rows(&value).iter().any(|r| r["model_hash"].is_null()));
    assert!(rows(&value)
        .iter()
        .any(|r| r["model_hash"] == "plugin:test/model"));
    assert_eq!(value["metrics"]["token_usage"], 720);
}

#[tokio::test]
async fn empty_and_unreported_usage_return_zero_without_losing_calls() {
    let (_dir, store) = store().await;
    let empty = overview(&store, None, None, None).await;
    assert!(rows(&empty).is_empty());
    assert_eq!(empty["metrics"]["llm_calls"], 0);
    call(&store, "no-usage", Some("a"), "openai", "cancelled", 1000).await;
    sqlx::query("UPDATE llm_calls SET input_tokens = NULL, output_tokens = NULL, cache_read_tokens = NULL, cache_write_tokens = NULL").execute(&store.pool).await.unwrap();
    let value = overview(&store, None, None, None).await;
    assert_eq!(rows(&value)[0]["metrics"]["token_usage"], 0);
    assert_eq!(rows(&value)[0]["metrics"]["failed_calls"], 1);
}
