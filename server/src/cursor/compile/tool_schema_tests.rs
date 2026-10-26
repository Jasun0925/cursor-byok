//! Checks MCP object schemas at the compiler and OpenAI HTTP boundaries.
use super::*;
use crate::{
    config::{ProviderConfig, ProviderKind},
    cursor::prompting::{Mode, PromptAssets, PromptCompiler},
    model::{ModelInvocation, ModelRequest, ModelSpec},
    provider::{ModelEvent, OpenAiChatProvider, OpenAiResponsesProvider, Provider},
};
use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use futures_util::StreamExt;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn responses_accepts_mcp_object_without_properties() {
    assert_responses_accepts(json!({"type":"object"})).await;
}

#[tokio::test]
async fn responses_accepts_empty_mcp_schema() {
    assert_responses_accepts(json!({})).await;
}

#[tokio::test]
async fn chat_accepts_mcp_null_required() {
    assert_openai_accepts(
        json!({"type":"object","properties":{},"required":null}),
        json!({"type":"object","properties":{},"required":[]}),
        true,
    )
    .await;
}

#[tokio::test]
async fn responses_accepts_mcp_null_required() {
    assert_openai_accepts(
        json!({"type":"object","properties":{},"required":null}),
        json!({"type":"object","properties":{},"required":[]}),
        false,
    )
    .await;
}

async fn assert_responses_accepts(schema: Value) {
    assert_openai_accepts(schema, json!({"type":"object","properties":{}}), false).await;
}

async fn assert_openai_accepts(schema: Value, expected: Value, chat: bool) {
    let wire = pb::McpToolDefinition {
        name: "mcp__local__status".into(),
        description: "Get local status".into(),
        input_schema_json: Some(schema.to_string()),
        ..Default::default()
    };
    let context = pb::RequestContext {
        tools: vec![wire.clone()],
        ..Default::default()
    };
    let compiled = dynamic_mcp(&pb::AgentRunRequest::default(), &context).unwrap();
    let (original, definition) = compiled.values().next().unwrap();
    assert_eq!(
        original, &wire,
        "client dispatch schema must remain untouched"
    );
    let assets =
        PromptAssets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor")).unwrap();
    let compiler = PromptCompiler::new(assets);
    let model = ModelSpec::new("test-model");
    let prompt = compiler
        .prompt_spec(Mode::Ask, &model, std::slice::from_ref(definition), false)
        .unwrap();
    let captured = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let requests = captured.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = if chat {
        "/v1/chat/completions"
    } else {
        "/v1/responses"
    };
    let app = Router::new().route(endpoint, post(move |Json(body): Json<Value>| {
        let requests = requests.clone();
        async move {
            requests.lock().unwrap().push(body.clone());
            // LM Studio's public llmToolParametersSchema requires an object
            // type and properties record; required is optional. This is a
            // focused contract double, not an actual LM Studio instance.
            // The required-array check reproduces the validation in #199;
            // it does not emulate the author's Grok gateway.
            for (index, tool) in body["tools"].as_array().unwrap().iter().enumerate() {
                let parameters = if chat { &tool["function"]["parameters"] } else { &tool["parameters"] };
                if parameters.get("required").is_some_and(|required| !required.is_array()) {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": {
                        "message": "Schema validation failed: [standard_violation] /required: null is not of type array"
                    }}))).into_response();
                }
                if parameters["type"] != "object"
                    || !parameters["properties"].is_object()
                {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error":{
                        "message":"Invalid input", "type":"invalid_request_error",
                        "param":format!("tools.{index}"), "code":"custom"
                    }}))).into_response();
                }
            }
            let sse = if chat {
                "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
            } else {
                "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"output\":[]}}\n\n"
            };
            ([("content-type", "text/event-stream")], sse).into_response()
        }
    }));
    let shutdown = CancellationToken::new();
    let stopped = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
            .unwrap();
    });
    let config = ProviderConfig {
        kind: if chat {
            ProviderKind::OpenAiChat
        } else {
            ProviderKind::OpenAiResponses
        },
        request_url: format!("http://{address}{endpoint}"),
        api_key: "test-only".into(),
        custom_headers: Default::default(),
        max_output_tokens: None,
        request_timeout: Duration::from_secs(5),
        allowed_body_fields: None,
    };
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let provider: Box<dyn Provider> = if chat {
        Box::new(OpenAiChatProvider::new(client, config))
    } else {
        Box::new(OpenAiResponsesProvider::new(client, config))
    };
    let invocation = ModelInvocation {
        call_id: "schema-call".into(),
        run_id: "schema-run".into(),
        conversation_id: "schema-conversation".into(),
        prompt_cache_enabled: true,
        provider_call_index: 0,
        request: ModelRequest {
            prompt,
            model,
            history: vec![],
        },
    };
    let results = tokio::time::timeout(
        Duration::from_secs(5),
        provider
            .stream(invocation.clone(), CancellationToken::new())
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    let retry = tokio::time::timeout(
        Duration::from_secs(5),
        provider
            .stream(invocation, CancellationToken::new())
            .collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    shutdown.cancel();
    server.await.unwrap();
    for events in [&results, &retry] {
        assert!(events.iter().all(Result::is_ok), "{events:?}");
        assert!(events
            .iter()
            .any(|event| matches!(event, Ok(ModelEvent::Done(_)))));
    }
    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0], requests[1],
        "retry must preserve tool definitions"
    );
    let tool = requests[0]["tools"].as_array().unwrap().last().unwrap();
    let parameters = if chat {
        &tool["function"]["parameters"]
    } else {
        &tool["parameters"]
    };
    assert_eq!(parameters, &expected);
}

#[test]
fn mcp_object_union_preserves_branches_and_constraints() {
    let original = json!({"oneOf":[
        {"type":"object","properties":{"a":{"type":"string"}},"required":["a"],"additionalProperties":false},
        {"type":"object","properties":{"b":{"type":"integer"}},"required":["b"],"additionalProperties":false}
    ]});
    let normalized = normalize_mcp_parameters("union", original.clone()).unwrap();
    assert_eq!(normalized["type"], "object");
    assert_eq!(normalized["properties"], json!({}));
    assert_eq!(normalized["oneOf"], original["oneOf"]);
    assert_eq!(
        normalize_mcp_parameters("union", normalized.clone()).unwrap(),
        normalized
    );
}

#[test]
fn mcp_explicit_object_constraints_are_not_rewritten() {
    let schema = json!({"type":"object", "properties":{"id":{"type":"string"}},
        "required":["id"], "additionalProperties":false, "$defs":{"value":{"type":"number"}}});
    assert_eq!(
        normalize_mcp_parameters("valid", schema.clone()).unwrap(),
        schema
    );
    for invalid in [
        json!(false),
        json!({"type":"string"}),
        json!({"type":"object","properties":[]}),
        json!({"anyOf":[{"type":"object"},{"type":"string"}]}),
    ] {
        assert!(
            normalize_mcp_parameters("invalid", invalid.clone()).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn mcp_null_required_preserves_property_values_and_is_idempotent() {
    let original = json!({"type":"object","required":null,
        "properties":{"required":{"type":["string","null"],"default":null}},
        "default":{"required":null},"examples":[{"required":null}],
        "additionalProperties":false});
    let mut expected = original.clone();
    expected["required"] = json!([]);
    let normalized = normalize_mcp_parameters("nullable", original).unwrap();
    assert_eq!(normalized, expected);
    assert_eq!(
        normalize_mcp_parameters("nullable", normalized).unwrap(),
        expected
    );
}

#[test]
fn mcp_protobuf_schema_normalizes_null_required() {
    use prost_types::{value::Kind, Struct};
    let wire = pb::McpToolDefinition {
        name: "protobuf_tool".into(),
        input_schema: Some(prost_types::Value {
            kind: Some(Kind::StructValue(Struct {
                fields: [
                    (
                        "type".into(),
                        prost_types::Value {
                            kind: Some(Kind::StringValue("object".into())),
                        },
                    ),
                    (
                        "required".into(),
                        prost_types::Value {
                            kind: Some(Kind::NullValue(0)),
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            })),
        }),
        ..Default::default()
    };
    let context = pb::RequestContext {
        tools: vec![wire.clone()],
        ..Default::default()
    };
    let compiled = dynamic_mcp(&pb::AgentRunRequest::default(), &context).unwrap();
    let (original, tool) = compiled.values().next().unwrap();
    assert_eq!(original, &wire);
    assert_eq!(
        tool.parameters,
        json!({"type":"object","properties":{},"required":[]})
    );
}
