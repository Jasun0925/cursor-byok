//! Temporary desired-behavior reproducer for upstream #505.
use base64::{engine::general_purpose::STANDARD, Engine};
use cursor_server::{cursor::checkpoint::messages::decode, model::ProviderReplayState};
use serde_json::json;

#[test]
fn multiple_opaque_cursor_signatures_should_recover() {
    let wire = json!({"id":"1","role":"assistant","content":[
        {"type":"reasoning","text":"first","signature":"opaque-first"},
        {"type":"reasoning","text":"second","signature":"opaque-second"},
        {"type":"tool-call","toolCallId":"read-1","toolName":"Read","args":{"path":"test.txt"}}
    ]});
    decode(
        &serde_json::to_vec(&wire).unwrap(),
        "issue505:opaque".into(),
    )
    .unwrap();
}

#[test]
fn multiple_byok_replay_envelopes_should_recover() {
    let reasoning = |text: &str, signature: &str| {
        let state = ProviderReplayState {
            provider_kind: "anthropic".into(),
            value: json!({"blocks":[
                {"type":"thinking","thinking":text,"signature":signature}
            ]}),
        };
        json!({"type":"reasoning","text":text,"signature":format!("cursor-byok:v1:{}", STANDARD.encode(serde_json::to_vec(&state).unwrap()))})
    };
    let wire = json!({"id":"1","role":"assistant","content":[
        reasoning("first", "sig-1"), reasoning("second", "sig-2"),
        {"type":"tool-call","toolCallId":"read-1","toolName":"Read","args":{"path":"test.txt"}}
    ]});
    decode(
        &serde_json::to_vec(&wire).unwrap(),
        "issue505:envelopes".into(),
    )
    .unwrap();
}
