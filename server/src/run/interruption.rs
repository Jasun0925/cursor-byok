//! Preserves interrupted output and settles cancelled tools before publishing recovery state.
use tokio_util::sync::CancellationToken;

use super::{CommitBarrier, CommitCause, MessagesCommitted, RunEvent, RunOutcome, RunPort};
use crate::{
    model::{
        CanonicalMessage, CheckpointId, MessageContent, Origin, PreparedRun, Role, RunAction,
        ToolResult, ToolRoundId,
    },
    store::Store,
};

pub use crate::model::CANCELLED_TOOL_MESSAGE;

pub(super) async fn preserve_text(
    store: &Store,
    prepared: &PreparedRun,
    client: &RunPort,
    checkpoint: CheckpointId,
    attempt_id: &str,
    text: &str,
) -> Result<CheckpointId, RunOutcome> {
    // A partial compaction summary must never become ordinary assistant history.
    if text.is_empty() || prepared.action == RunAction::Compact {
        return Ok(checkpoint);
    }
    let message = CanonicalMessage {
        message_id: format!("{}:interrupted:{attempt_id}", prepared.run_id),
        role: Role::Assistant,
        origin: Origin::Assistant,
        content: MessageContent::Assistant {
            text: text.into(),
            // Incomplete thinking has no validated provider replay/signature state.
            thinking: String::new(),
            tool_round_id: None,
            replay_state: None,
            tool_calls: Vec::new(),
        },
        runtime_event_id: None,
    };
    // Persist the continuation cue as well, so another interrupted retry appends
    // after the same provider-visible prefix instead of replacing a transient tail.
    let continuation = CanonicalMessage::text(
        format!("{}:interrupted:{attempt_id}:continue", prepared.run_id),
        Role::User,
        Origin::Prompt,
        "The previous response was interrupted. Its saved text may be incomplete. Continue from the last completed work; do not repeat completed operations.",
    );
    let mut checkpoint = checkpoint;
    let mut changed = false;
    for message in [message, continuation] {
        let (next, inserted) = store
            .append_message_once(
                &prepared.conversation_id,
                &prepared.run_id,
                checkpoint,
                &message,
            )
            .await
            .map_err(failed)?;
        checkpoint = next;
        changed |= inserted;
    }
    if changed {
        publish(client, checkpoint, 0, CommitCause::Interrupted).await?;
    }
    Ok(checkpoint)
}

pub(super) async fn cancel_tools(
    store: &Store,
    prepared: &PreparedRun,
    client: &RunPort,
    round_id: &ToolRoundId,
) -> Result<(), RunOutcome> {
    let round = store
        .tool_round(round_id)
        .await
        .map_err(failed)?
        .ok_or_else(|| {
            failed(crate::Error::Store(
                "cancelled tool round disappeared".into(),
            ))
        })?;
    let mut last_commit = None;
    for call in &round.calls {
        if round.completed_call_ids.contains(&call.call_id) {
            continue;
        }
        last_commit = Some(
            store
                .commit_tool_result(
                    &prepared.conversation_id,
                    &prepared.run_id,
                    round_id,
                    &ToolResult {
                        call_id: call.call_id.clone(),
                        content: CANCELLED_TOOL_MESSAGE.into(),
                        is_error: true,
                        image: None,
                    },
                )
                .await
                .map_err(failed)?,
        );
    }
    if let Some(commit) = last_commit {
        publish(
            client,
            commit.checkpoint_id,
            commit.tool_round_version,
            CommitCause::ToolRoundCancelled(round_id.clone()),
        )
        .await?;
    }
    Ok(())
}

async fn publish(
    client: &RunPort,
    checkpoint_id: CheckpointId,
    tool_round_version: u64,
    cause: CommitCause,
) -> Result<(), RunOutcome> {
    let (barrier, ready) = CommitBarrier::before_continue();
    super::engine::emit(
        client,
        RunEvent::MessagesCommitted(MessagesCommitted {
            checkpoint_id,
            tool_round_version,
            cause,
            barrier,
        }),
    )
    .await
    .map_err(|_| super::engine::client_failure())?;
    // The run is already cancelled: allow its final recovery checkpoint to be
    // acknowledged. Transport shutdown or worker failure still closes the barrier.
    super::engine::wait_for_state_ready(ready, &CancellationToken::new()).await
}

fn failed(error: crate::Error) -> RunOutcome {
    RunOutcome::Failed(error.into())
}
