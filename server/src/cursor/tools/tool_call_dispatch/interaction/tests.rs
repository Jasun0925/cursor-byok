//! Cache reads use exactly the same Cursor approval boundary as URL fetches.
use super::*;
use crate::{search::WebCache, store::Store};
use serde_json::json;

fn cache_call(content_id: &str) -> ToolCall {
    let arguments = json!({"url": "https://example.com", "content_id": content_id, "offset": 16384, "limit": 16384});
    ToolCall {
        index: 0,
        call_id: "cache-read".into(),
        model_call_id: "model-call".into(),
        name: "WebFetch".into(),
        arguments_text: arguments.to_string(),
        arguments,
        argument_error: None,
    }
}

fn response(id: u32, approved: bool) -> pb::InteractionResponse {
    pb::InteractionResponse {
        id,
        result: Some(pb::interaction_response::Result::WebFetchRequestResponse(
            pb::WebFetchRequestResponse {
                result: Some(if approved {
                    pb::web_fetch_request_response::Result::Approved(Default::default())
                } else {
                    pb::web_fetch_request_response::Result::Rejected(
                        pb::web_fetch_request_response::Rejected {
                            reason: "User denied cached content access".into(),
                        },
                    )
                }),
            },
        )),
    }
}

async fn reserve(runtime: &CursorToolRuntime, call: &ToolCall, owner: &str) -> u32 {
    let context = ExecContext {
        conversation_id: owner.into(),
        ..Default::default()
    };
    let started = start(runtime, call, &context).await.unwrap();
    assert!(started.completion.is_none());
    let Some(pb::agent_server_message::Message::InteractionQuery(query)) =
        started.messages[0].message.as_ref()
    else {
        panic!("expected approval query")
    };
    let Some(pb::interaction_query::Query::WebFetchRequestQuery(fetch)) = query.query.as_ref()
    else {
        panic!("expected WebFetch approval query")
    };
    assert!(!fetch.skip_approval);
    assert_eq!(fetch.args.as_ref().unwrap().url, call.arguments["url"]);
    query.id
}

#[tokio::test]
async fn web_fetch_download_pages_survive_restart_without_refetching() {
    use crate::store::{ProxyMode, ProxySettingsInput};
    use axum::{
        http::{StatusCode, Uri},
        response::IntoResponse,
        Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    // A real local HTTP proxy supplies a public-address URL. The production
    // resolver and SSRF checks remain enabled; no fixture-only client bypass.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let original = format!("FIRST\n{}\nLAST", "α日本🦀\n".repeat(7000));
    let count = Arc::new(AtomicUsize::new(0));
    let hits = count.clone();
    let body = original.clone();
    let app = Router::new().fallback(move |uri: Uri| {
        let hits = hits.clone();
        let body = body.clone();
        async move {
            hits.fetch_add(1, Ordering::SeqCst);
            if uri.path() == "/start" {
                return (StatusCode::FOUND, [("location", "/final")]).into_response();
            }
            assert_eq!(uri.path(), "/final");
            ([("content-type", "text/plain; charset=utf-8")], body).into_response()
        }
    });
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let store = Store::connect("sqlite::memory:").await.unwrap();
    store
        .set_proxy_settings(ProxySettingsInput {
            mode: ProxyMode::Custom,
            address: format!("http://{address}"),
            auth_enabled: false,
            username: String::new(),
            password: None,
        })
        .await
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let fetch = WebFetch::managed(
        store.clone(),
        WebCache::at(directory.path().into()).unwrap(),
    );
    let runtime = CursorToolRuntime::default();
    let (results, mut receiver) = crate::cursor::tools::tool_call_result::tool_result_channel();
    let mut call = cache_call("unused");
    call.arguments = json!({"url":"http://93.184.215.14/start"});
    call.arguments_text = call.arguments.to_string();
    let id = reserve(&runtime, &call, "owner").await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "no network before approval"
    );
    resume(
        &results,
        &WebSearch::built_in(),
        &fetch,
        runtime.take_interaction(id).await.unwrap(),
        &response(id, true),
    )
    .await
    .unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!first.result().is_error, "{}", first.result().content);
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "one fetch and one redirect"
    );
    let first_output = first.result().content.clone();
    drop(fetch);

    let fetch = WebFetch::managed(store, WebCache::at(directory.path().into()).unwrap());
    let mut output = first_output.clone();
    let mut reconstructed = String::new();
    let mut page_count = 0;
    loop {
        let (metadata, text) = output.split_once("</system_reminder>\n").unwrap();
        reconstructed.push_str(text);
        page_count += 1;
        assert!(text.len() <= 16384);
        assert!(!metadata.contains("127.0.0.1") && !metadata.contains("Shell"));
        let Some((_, next)) = metadata.split_once("Next page: call WebFetch with ") else {
            assert!(metadata.contains("End of cached content"));
            break;
        };
        call.arguments = serde_json::Deserializer::from_str(next)
            .into_iter::<serde_json::Value>()
            .next()
            .unwrap()
            .unwrap();
        call.call_id = format!("page-{page_count}");
        call.arguments_text = call.arguments.to_string();
        assert_eq!(call.arguments["url"], "http://93.184.215.14/final");
        assert_eq!(call.arguments["offset"], reconstructed.len());
        let id = reserve(&runtime, &call, "owner").await;
        resume(
            &results,
            &WebSearch::built_in(),
            &fetch,
            runtime.take_interaction(id).await.unwrap(),
            &response(id, true),
        )
        .await
        .unwrap();
        let completion = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            !completion.result().is_error,
            "{}",
            completion.result().content
        );
        output = completion.result().content.clone();
        let Some(pb::tool_call::Tool::WebFetchToolCall(tool)) =
            completion.tool_call().tool.as_ref()
        else {
            panic!("missing WebFetch card")
        };
        let Some(pb::web_fetch_result::Result::Success(success)) =
            tool.result.as_ref().and_then(|r| r.result.as_ref())
        else {
            panic!("missing successful page")
        };
        assert_eq!(success.markdown, output);
        assert!(success.output_location.is_none());
        assert_eq!(
            count.load(Ordering::SeqCst),
            2,
            "cached pages must not refetch"
        );
    }
    assert!(page_count > 3);
    assert_eq!(reconstructed, original);
    assert_eq!(
        first.result().content,
        first_output,
        "prior results remain immutable"
    );
    server.abort();
}

#[tokio::test]
async fn web_fetch_cached_pages_require_approval_and_preserve_conversation_ownership() {
    let cache = WebCache::default();
    let original = format!("{}TAIL", "é".repeat(10_000));
    let (_, entry) = cache
        .store("owner", "https://example.com", original.clone())
        .await
        .unwrap();
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let fetch = WebFetch::managed(store, cache);
    let search = WebSearch::built_in();
    let runtime = CursorToolRuntime::default();
    let call = cache_call(&entry.content_id);
    let (results, mut receiver) = crate::cursor::tools::tool_call_result::tool_result_channel();
    let id = reserve(&runtime, &call, "owner").await;
    // No result is available before the remote approval response.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), receiver.recv())
            .await
            .is_err()
    );
    let pending = runtime.take_interaction(id).await.unwrap();
    assert!(matches!(
        resume(&results, &search, &fetch, pending, &response(id, true))
            .await
            .unwrap(),
        InteractionContinuation::Pending
    ));
    let completion = receiver.recv().await.unwrap().unwrap();
    assert!(!completion.result().is_error);
    assert!(completion.result().content.ends_with(&original[16384..]));
    assert!(completion
        .result()
        .content
        .contains("End of cached content"));
    assert!(
        runtime.take_interaction(id).await.is_none(),
        "approval can only be consumed once"
    );

    let id = reserve(&runtime, &call, "other-conversation").await;
    let pending = runtime.take_interaction(id).await.unwrap();
    resume(&results, &search, &fetch, pending, &response(id, true))
        .await
        .unwrap();
    let completion = receiver.recv().await.unwrap().unwrap();
    assert!(completion.result().is_error);
    assert!(!completion.result().content.contains("TAIL"));
}

#[tokio::test]
async fn web_fetch_denial_and_preapproval_cancellation_never_read_cache() {
    let runtime = CursorToolRuntime::default();
    let call = cache_call("550e8400-e29b-41d4-a716-446655440000");
    let (results, mut receiver) = crate::cursor::tools::tool_call_result::tool_result_channel();
    let id = reserve(&runtime, &call, "owner").await;
    let pending = runtime.take_interaction(id).await.unwrap();
    let result = resume(
        &results,
        &WebSearch::built_in(),
        &WebFetch::built_in(),
        pending,
        &response(id, false),
    )
    .await
    .unwrap();
    let InteractionContinuation::Completed(completion) = result else {
        panic!("denial must be terminal")
    };
    assert!(completion.result().is_error);
    assert_eq!(
        completion.result().content,
        "User denied cached content access"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), receiver.recv())
            .await
            .is_err()
    );

    let id = reserve(&runtime, &call, "owner").await;
    runtime.interrupt_for_message().await;
    assert!(runtime.is_interrupted(id).await);
    assert!(runtime.take_interaction(id).await.is_none());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), receiver.recv())
            .await
            .is_err()
    );
}
