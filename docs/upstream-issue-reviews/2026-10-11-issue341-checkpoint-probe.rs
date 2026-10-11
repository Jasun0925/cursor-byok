//! Diagnostic: supplies complete edit snapshots to isolate CheckpointBuilder.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use cursor_server::{
    cursor::{
        checkpoint::{CheckpointBuilder, PendingSteps},
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        services::blob_sync::BlobSynchronizer,
        TransportRegistry,
    },
    store::BlobId,
};
use prost::Message;
use std::{collections::HashSet, sync::Arc};

#[tokio::test]
async fn completed_edit_with_full_snapshots_should_update_file_states() {
    let (_directory, store) = fixtures::temp_store().await;
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(fake_provider::FakeProvider::default()),
        PromptCompiler::new(assets),
    );
    let handle = registry
        .get_or_create("issue341-checkpoint-probe")
        .await
        .unwrap();
    let mut output = handle.subscribe();
    let sync = BlobSynchronizer::new("issue341-checkpoint-probe".into(), store.clone(), handle);
    let mut builder = CheckpointBuilder::new(store.clone(), sync.clone(), None, None);
    builder.configure(
        "test-model".into(),
        None,
        String::new(),
        Vec::new(),
        HashSet::new(),
        Some(pb::UserMessage::default()),
    );
    let before = "value=1\n";
    let after = "value=2\n";
    let presentation = PendingSteps {
        steps: vec![pb::ConversationStep {
            message: Some(pb::conversation_step::Message::ToolCall(pb::ToolCall {
                tool_call_id: Some("issue341-edit".into()),
                tool: Some(pb::tool_call::Tool::EditToolCall(pb::EditToolCall {
                    args: Some(pb::EditArgs {
                        path: "/isolated/example.txt".into(),
                        ..Default::default()
                    }),
                    result: Some(pb::EditResult {
                        result: Some(pb::edit_result::Result::Success(pb::EditSuccess {
                            path: "/isolated/example.txt".into(),
                            before_full_file_content: Some(before.into()),
                            after_full_file_content: after.into(),
                            ..Default::default()
                        })),
                    }),
                })),
                ..Default::default()
            })),
        }],
        read_paths: Vec::new(),
    };
    let state = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let build = builder.settled(&[], 1, &presentation);
        tokio::pin!(build);
        loop {
            tokio::select! {
                state = &mut build => break state.unwrap(),
                frame = output.recv() => {
                    for (_, bytes) in connect::decode_frames(&frame.unwrap()).unwrap() {
                        let server = pb::AgentServerMessage::decode(bytes).unwrap();
                        if let Some(pb::agent_server_message::Message::KvServerMessage(kv)) = server.message {
                            assert!(matches!(kv.message, Some(pb::kv_server_message::Message::SetBlobArgs(_))));
                            sync.handle_client(pb::KvClientMessage { id: kv.id, message: Some(pb::kv_client_message::Message::SetBlobResult(pb::SetBlobResult { error: None })) }).await.unwrap();
                        }
                    }
                }
            }
        }
    }).await.unwrap();
    assert_eq!(state.turns.len(), 1);
    let turn_blob = store
        .get_blob(&BlobId::from_bytes(&state.turns[0]).unwrap())
        .await
        .unwrap()
        .unwrap();
    let turn = pb::ConversationTurnStructure::decode(turn_blob.as_slice()).unwrap();
    let Some(pb::conversation_turn_structure::Turn::AgentConversationTurn(turn)) = turn.turn else {
        panic!("agent turn")
    };
    assert_eq!(turn.steps.len(), 1);
    let edit_blob = store
        .get_blob(&BlobId::from_bytes(&turn.steps[0]).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        pb::ConversationStep::decode(edit_blob.as_slice()).unwrap(),
        presentation.steps[0]
    );
    println!(
        "complete_edit_step_persisted=true file_states={} file_states_v2={}",
        state.file_states.len(),
        state.file_states_v2.len()
    );
    assert!(
        state.file_states_v2.contains_key("/isolated/example.txt"),
        "complete edit snapshot was persisted but no file state was derived"
    );
}
