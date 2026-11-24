//! Regression tests for assistant turns that carry more than one replay state.
use cursor_server::{
    model::{merge_replay_states, ProviderReplayState},
    provider::{FinishReason, ModelEvent},
    run::consume_model_cycle,
};
use futures_util::stream;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn duplicate_provider_replay_states_are_kept_in_order() {
    let stream = Box::pin(stream::iter([
        Ok(ModelEvent::Start {
            model_call_id: "model-call".into(),
        }),
        Ok(ModelEvent::ProviderReplayState(ProviderReplayState {
            provider_kind: "openai_chat".into(),
            value: json!({"reasoning_content":"first"}),
        })),
        Ok(ModelEvent::ProviderReplayState(ProviderReplayState {
            provider_kind: "openai_chat".into(),
            value: json!({"reasoning_content":"second"}),
        })),
        Ok(ModelEvent::Done(FinishReason::Stop)),
    ]));
    let (sender, _receiver) = tokio::sync::mpsc::channel(16);
    let result = consume_model_cycle(stream, &sender, &CancellationToken::new())
        .await
        .expect("duplicate replay states must not fail the model cycle");

    assert_eq!(
        result.replay_state.unwrap().value,
        json!({"reasoning_content":"firstsecond"})
    );
}

#[test]
fn cursor_opaque_reasoning_part_lists_are_concatenated_in_order() {
    let merged = merge_replay_states([
        ProviderReplayState {
            provider_kind: "cursor_opaque".into(),
            value: json!([{"signature":"sig-1","text":"first"}]),
        },
        ProviderReplayState {
            provider_kind: "cursor_opaque".into(),
            value: json!([{"text":"narration"},{"signature":"sig-2","text":"second"}]),
        },
    ])
    .expect("replay state");

    assert_eq!(merged.provider_kind, "cursor_opaque");
    assert_eq!(
        merged.value,
        json!([
            {"signature":"sig-1","text":"first"},
            {"text":"narration"},
            {"signature":"sig-2","text":"second"}
        ])
    );
}

#[test]
fn anthropic_replay_blocks_are_concatenated_without_reordering() {
    let merged = merge_replay_states([
        ProviderReplayState {
            provider_kind: "anthropic".into(),
            value: json!({"blocks":[{"type":"thinking","signature":"sig-1"}]}),
        },
        ProviderReplayState {
            provider_kind: "anthropic".into(),
            value: json!({"blocks":[{"type":"thinking","signature":"sig-2"}]}),
        },
    ])
    .expect("replay state");

    assert_eq!(
        merged.value["blocks"],
        json!([
            {"type":"thinking","signature":"sig-1"},
            {"type":"thinking","signature":"sig-2"}
        ])
    );
}
