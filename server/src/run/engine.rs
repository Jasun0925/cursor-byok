//! Executes one Run across model cycles, Tool rounds, and message commits.
use std::sync::Arc;
use std::time::Duration;

mod compaction;

use compaction::{CompactionFailure, FailedSummary};
use tokio_util::sync::CancellationToken;

use crate::{
    model::{
        CanonicalMessage, MessageContent, Origin, PreparedRun, Role, RunAction, ToolRoundAssistant,
        ToolRoundId, Usage,
    },
    provider::Provider,
    store::{ContextUsageAnchor, RunStatus, Store},
};

use super::{
    consume_model_cycle,
    model_retry::{should_retry, MODEL_RETRY_DELAY},
    CommitBarrier, CommitCause, MessagesCommitted, RunCommand, RunEvent, RunFailure, RunOutcome,
    RunPort,
};

pub struct RunEngine {
    store: Store,
    provider: Arc<dyn Provider>,
}

/// Provider-visible tail appended when the committed history ends with the
/// assistant, which happens when Cursor resumes a turn that had already
/// finished (for example after a cancelled follow-up).
const CONTINUE_MESSAGE_ID: &str = "runtime:continue";
const CONTINUE_INSTRUCTION: &str = "Continue from where you left off.";

impl RunEngine {
    pub fn new(store: Store, provider: Arc<dyn Provider>) -> Self {
        Self { store, provider }
    }

    #[tracing::instrument(
        skip_all,
        fields(run_id = %prepared.run_id, conversation_id = %prepared.conversation_id)
    )]
    pub async fn run(
        &self,
        prepared: PreparedRun,
        mut client: RunPort,
        cancellation: CancellationToken,
    ) -> RunOutcome {
        let claimed = match self.store.claim_run(&prepared).await {
            Ok(claimed) => claimed,
            Err(error) => {
                let outcome = RunOutcome::Failed(error.into());
                client.phase.finish();
                let _ = client.events.send(RunEvent::Ended(outcome.clone())).await;
                tracing::info!(outcome = ?outcome, "Run claim failed");
                return outcome;
            }
        };
        let outcome = self
            .run_claimed(
                &prepared,
                claimed.head_checkpoint_id,
                &mut client,
                &cancellation,
            )
            .await;
        let usage = outcome.1;
        // A replacement transport may close the receiver while a cancelled run
        // publishes its final recovery checkpoint. That remains a cancellation.
        let outcome = match outcome.0 {
            RunOutcome::Failed(RunFailure::Client(_))
                if cancellation.is_cancelled() && client.events.is_closed() =>
            {
                RunOutcome::Cancelled
            }
            outcome => outcome,
        };
        let (status, failure) = match &outcome {
            RunOutcome::Completed => (RunStatus::Completed, None),
            RunOutcome::Cancelled => (RunStatus::Cancelled, None),
            RunOutcome::Failed(failure) => (
                RunStatus::Failed,
                Some((failure.category(), failure_message(failure))),
            ),
        };
        let failure_ref = failure
            .as_ref()
            .map(|(category, summary)| (*category, summary.as_str()));
        if let Err(error) = self
            .store
            .finish_run(&prepared.run_id, status, usage, failure_ref)
            .await
        {
            tracing::error!(run_id = %prepared.run_id, %error, "failed to persist Run outcome");
        }
        client.phase.finish();
        let _ = client.events.send(RunEvent::Ended(outcome.clone())).await;
        tracing::info!(outcome = ?outcome, usage = ?usage, "Run ended");
        outcome
    }

    async fn run_claimed(
        &self,
        prepared: &PreparedRun,
        mut checkpoint: crate::model::CheckpointId,
        client: &mut RunPort,
        cancellation: &CancellationToken,
    ) -> (RunOutcome, Option<Usage>) {
        let mut usage = None;
        let mut context_usage_anchor = match self
            .store
            .latest_context_usage(prepared.conversation_id.as_str())
            .await
        {
            Ok(anchor) => anchor,
            Err(error) => return (RunOutcome::Failed(error.into()), usage),
        };
        tracing::info!(
            checkpoint_id = checkpoint.0,
            "Run claimed conversation ownership"
        );
        if !prepared.initial_messages.is_empty() {
            let mut changed = false;
            for message in &prepared.initial_messages {
                match self
                    .store
                    .append_message_once(
                        &prepared.conversation_id,
                        &prepared.run_id,
                        checkpoint,
                        message,
                    )
                    .await
                {
                    Ok((next, inserted)) => {
                        checkpoint = next;
                        changed |= inserted;
                    }
                    Err(error) => return (RunOutcome::Failed(error.into()), usage),
                }
            }
            if changed {
                let (barrier, ready) = CommitBarrier::before_continue();
                if emit(
                    client,
                    RunEvent::MessagesCommitted(MessagesCommitted {
                        checkpoint_id: checkpoint,
                        tool_round_version: 0,
                        cause: CommitCause::InitialMessages,
                        barrier,
                    }),
                )
                .await
                .is_err()
                {
                    return (client_failure(), usage);
                }
                if let Err(outcome) = wait_for_state_ready(ready, cancellation).await {
                    return (outcome, usage);
                }
            }
        }

        if let RunAction::Resume {
            pending_tool_round: Some(round),
        } = &prepared.action
        {
            checkpoint = match super::tool_round::execute(
                &self.store,
                prepared,
                client,
                cancellation,
                checkpoint,
                super::tool_round::ToolRound {
                    id: ToolRoundId::new(format!("{}:round:resume", prepared.run_id)),
                    assistant: round.assistant.clone(),
                    calls: round.calls.clone(),
                    recovered_started_at_ms: Some(round.started_at_ms),
                },
                Vec::new(),
            )
            .await
            {
                Ok(checkpoint) => checkpoint,
                Err(outcome) => return (outcome, usage),
            };
        }

        // A provider refusal for an over-limit prompt triggers one compaction
        // per run. A second refusal after compacting means the current input
        // itself does not fit, and compacting again would only destroy history.
        let mut overflow_compacted = false;
        // A failed preflight summary is not retried within the run: the model
        // is called with the full history instead, and each tool round would
        // otherwise pay for another summary attempt that is likely to fail.
        let mut preflight_compaction_failed = false;
        'model: loop {
            if cancellation.is_cancelled() {
                return (RunOutcome::Cancelled, usage);
            }
            let messages = match self.store.load_checkpoint_messages(checkpoint).await {
                Ok(messages) => messages,
                Err(error) => return (RunOutcome::Failed(error.into()), usage),
            };
            let history = match crate::model::project_messages(&messages) {
                Ok(history) => history,
                Err(error) => return (RunOutcome::Failed(error.into()), usage),
            };
            let compaction_estimate = (prepared.action != RunAction::Compact)
                .then(|| {
                    super::compaction::compaction_estimate(prepared, &history, context_usage_anchor)
                })
                .flatten();
            if let Some(estimated_tokens) = compaction_estimate {
                if emit(
                    client,
                    RunEvent::UsageSnapshot(context_usage_snapshot(estimated_tokens)),
                )
                .await
                .is_err()
                {
                    return (client_failure(), usage);
                }
            }
            if let Some(estimated_tokens) =
                compaction_estimate.filter(|_| !preflight_compaction_failed)
            {
                match self
                    .auto_compact(prepared, checkpoint, &messages, client, cancellation)
                    .await
                {
                    Ok((next_checkpoint, compaction_usage)) => {
                        checkpoint = next_checkpoint;
                        context_usage_anchor = None;
                        if let Some(compaction_usage) = compaction_usage {
                            accumulate_usage(&mut usage, compaction_usage);
                        }
                        continue 'model;
                    }
                    Err(CompactionFailure::Summary(failed)) => {
                        let FailedSummary {
                            checkpoint: next_checkpoint,
                            failure,
                            usage: compaction_usage,
                        } = *failed;
                        // The original history is intact. Answer from it; if the
                        // provider then refuses it as too long, the overflow path
                        // below compacts once more and fails the turn if that
                        // summary fails too.
                        tracing::warn!(
                            ?failure,
                            estimated_tokens,
                            checkpoint_id = next_checkpoint.0,
                            "automatic compaction failed; continuing with the full history"
                        );
                        checkpoint = next_checkpoint;
                        preflight_compaction_failed = true;
                        if let Some(compaction_usage) = compaction_usage {
                            accumulate_usage(&mut usage, compaction_usage);
                        }
                        if emit(client, RunEvent::AutoCompactionFailed).await.is_err() {
                            return (client_failure(), usage);
                        }
                        continue 'model;
                    }
                    Err(CompactionFailure::Run(outcome)) => return (outcome, usage),
                }
            }
            let provider_call_index = match self.store.begin_provider_call(&prepared.run_id).await {
                Ok(index) => index,
                Err(error) => return (RunOutcome::Failed(error.into()), usage),
            };
            tracing::debug!(
                provider_call_index,
                checkpoint_id = checkpoint.0,
                "starting model call"
            );
            let mut history = history;
            if let Err(error) = hydrate_tool_images(&self.store, &mut history).await {
                return (RunOutcome::Failed(error.into()), usage);
            }
            // The usage anchor counts persisted messages only; a transient
            // tail is provider-visible but never committed.
            let mut anchored_messages = history.len();
            let history = if prepared.action == RunAction::Compact {
                super::history::user_terminated(
                    history,
                    "compaction:instruction",
                    super::compaction::INSTRUCTIONS,
                )
            } else {
                super::history::user_terminated(history, CONTINUE_MESSAGE_ID, CONTINUE_INSTRUCTION)
            };
            let mut request = crate::model::ModelRequest {
                prompt: prepared.prompt.clone(),
                model: prepared.model.clone(),
                history,
            };
            let mut retries = 0_u32;
            let mut pending_insertions = Vec::new();
            let cycle = 'attempt: loop {
                let call_id = if retries == 0 {
                    format!("{}:{provider_call_index}", prepared.run_id)
                } else {
                    format!("{}:{provider_call_index}:retry-{retries}", prepared.run_id)
                };
                let invocation = crate::model::ModelInvocation {
                    call_id,
                    run_id: prepared.run_id.to_string(),
                    conversation_id: prepared.conversation_id.to_string(),
                    prompt_cache_enabled: true,
                    provider_call_index,
                    request: request.clone(),
                };
                let cycle_cancellation = cancellation.child_token();
                let cycle_events = client.events.clone();
                let cycle = consume_model_cycle(
                    self.provider.stream(invocation, cycle_cancellation.clone()),
                    &cycle_events,
                    &cycle_cancellation,
                );
                tokio::pin!(cycle);
                let cycle = loop {
                    tokio::select! {
                        biased;
                        command = client.commands.recv() => {
                            let interruption = match command {
                                Some(RunCommand::InsertMessages(insertion)) => {
                                    pending_insertions.push(insertion);
                                    continue;
                                }
                                Some(RunCommand::BreakMessages(messages)) => messages,
                                Some(RunCommand::Cancel) => {
                                    cycle_cancellation.cancel();
                                    let text = match cycle.await {
                                        Ok(cycle) => cycle.text,
                                        Err(failure) => failure.partial_text,
                                    };
                                    if let Err(outcome) = super::interruption::preserve_text(
                                        &self.store, prepared, client, checkpoint,
                                        &format!("{provider_call_index}:{retries}"), &text,
                                    ).await {
                                        return (outcome, usage);
                                    }
                                    let _ = emit(client, RunEvent::CycleInterrupted).await;
                                    return (RunOutcome::Cancelled, usage);
                                }
                                Some(RunCommand::ToolResult(_)) => {
                                    cycle_cancellation.cancel();
                                    let _ = cycle.await;
                                    let _ = emit(client, RunEvent::CycleInterrupted).await;
                                    return (
                                        RunOutcome::Failed(RunFailure::Protocol(
                                            "received a tool result while the model was running".into(),
                                        )),
                                        usage,
                                    );
                                }
                                None => {
                                    cycle_cancellation.cancel();
                                    let _ = cycle.await;
                                    let _ = emit(client, RunEvent::CycleInterrupted).await;
                                    return (client_failure(), usage);
                                }
                            };
                            cycle_cancellation.cancel();
                            let interrupted = cycle.await;
                            match interrupted {
                                Ok(cycle) => {
                                    if let Some(cycle_usage) = cycle.usage {
                                        update_context_usage_anchor(
                                            &mut context_usage_anchor,
                                            cycle_usage,
                                            anchored_messages,
                                        );
                                        accumulate_usage(&mut usage, cycle_usage);
                                    }
                                }
                                Err(failure) => {
                                    if let Some(cycle_usage) = failure.usage {
                                        update_context_usage_anchor(
                                            &mut context_usage_anchor,
                                            cycle_usage,
                                            anchored_messages,
                                        );
                                        accumulate_usage(&mut usage, cycle_usage);
                                    }
                                }
                            }
                            if emit(client, RunEvent::CycleInterrupted).await.is_err() {
                                return (client_failure(), usage);
                            }
                            checkpoint = match super::messages::append_batches(
                                &self.store,
                                prepared,
                                client,
                                cancellation,
                                checkpoint,
                                std::mem::take(&mut pending_insertions),
                            )
                            .await
                            {
                                Ok((checkpoint, _)) => checkpoint,
                                Err(outcome) => return (outcome, usage),
                            };
                            checkpoint = match super::messages::append_batches(
                                &self.store,
                                prepared,
                                client,
                                cancellation,
                                checkpoint,
                                vec![interruption],
                            )
                            .await
                            {
                                Ok((checkpoint, _)) => checkpoint,
                                Err(outcome) => return (outcome, usage),
                            };
                            continue 'model;
                        },
                        result = &mut cycle => break result,
                    }
                };
                match cycle {
                    Ok(cycle) => break 'attempt cycle,
                    Err(cycle_failure) => {
                        if let Some(cycle_usage) = cycle_failure.usage {
                            update_context_usage_anchor(
                                &mut context_usage_anchor,
                                cycle_usage,
                                anchored_messages,
                            );
                            accumulate_usage(&mut usage, cycle_usage);
                        }
                        let previous_checkpoint = checkpoint;
                        checkpoint = match super::interruption::preserve_text(
                            &self.store,
                            prepared,
                            client,
                            checkpoint,
                            &format!("{provider_call_index}:{retries}"),
                            &cycle_failure.partial_text,
                        )
                        .await
                        {
                            Ok(checkpoint) => checkpoint,
                            Err(outcome) => return (outcome, usage),
                        };
                        if checkpoint != previous_checkpoint {
                            let messages =
                                match self.store.load_checkpoint_messages(checkpoint).await {
                                    Ok(messages) => messages,
                                    Err(error) => return (RunOutcome::Failed(error.into()), usage),
                                };
                            let mut history = match crate::model::project_messages(&messages) {
                                Ok(history) => history,
                                Err(error) => return (RunOutcome::Failed(error.into()), usage),
                            };
                            if let Err(error) = hydrate_tool_images(&self.store, &mut history).await
                            {
                                return (RunOutcome::Failed(error.into()), usage);
                            }
                            anchored_messages = history.len();
                            request.history = super::history::user_terminated(
                                history,
                                CONTINUE_MESSAGE_ID,
                                CONTINUE_INSTRUCTION,
                            );
                        }
                        if cancellation.is_cancelled() {
                            let _ = emit(client, RunEvent::CycleInterrupted).await;
                            return (RunOutcome::Cancelled, usage);
                        }
                        // The estimate that cleared the compaction check can
                        // still land over the real limit: it trails the
                        // provider's own count by whatever the request adds
                        // after the anchor was taken. The provider is the
                        // authority, so treat its refusal as the trigger the
                        // estimate missed. Without this a conversation that
                        // crosses the line is wedged: every retry rebuilds the
                        // same prompt and gets the same refusal.
                        if !overflow_compacted
                            && prepared.action != RunAction::Compact
                            && matches!(&cycle_failure.failure, RunFailure::Provider(message)
                                if super::compaction::is_context_overflow(message))
                        {
                            tracing::warn!(
                                provider_call_index,
                                checkpoint_id = checkpoint.0,
                                "provider rejected the prompt as over-limit; compacting and retrying"
                            );
                            overflow_compacted = true;
                            checkpoint = match super::messages::append_batches(
                                &self.store,
                                prepared,
                                client,
                                cancellation,
                                checkpoint,
                                std::mem::take(&mut pending_insertions),
                            )
                            .await
                            {
                                Ok((checkpoint, _)) => checkpoint,
                                Err(outcome) => return (outcome, usage),
                            };
                            let messages =
                                match self.store.load_checkpoint_messages(checkpoint).await {
                                    Ok(messages) => messages,
                                    Err(error) => return (RunOutcome::Failed(error.into()), usage),
                                };
                            match self
                                .auto_compact(prepared, checkpoint, &messages, client, cancellation)
                                .await
                            {
                                Ok((next_checkpoint, compaction_usage)) => {
                                    checkpoint = next_checkpoint;
                                    context_usage_anchor = None;
                                    if let Some(compaction_usage) = compaction_usage {
                                        accumulate_usage(&mut usage, compaction_usage);
                                    }
                                    continue 'model;
                                }
                                Err(CompactionFailure::Summary(failed)) => {
                                    let FailedSummary {
                                        failure,
                                        usage: compaction_usage,
                                        ..
                                    } = *failed;
                                    if let Some(compaction_usage) = compaction_usage {
                                        accumulate_usage(&mut usage, compaction_usage);
                                    }
                                    return (RunOutcome::Failed(failure), usage);
                                }
                                Err(CompactionFailure::Run(outcome)) => return (outcome, usage),
                            }
                        }
                        if !should_retry(&cycle_failure, retries) {
                            return (RunOutcome::Failed(cycle_failure.failure), usage);
                        }
                        retries += 1;
                        let message = failure_message(&cycle_failure.failure);
                        tracing::warn!(
                            provider_call_index,
                            retries,
                            max_retries = super::model_retry::MAX_MODEL_RETRIES,
                            delay_ms = MODEL_RETRY_DELAY.as_millis() as u64,
                            %message,
                            checkpoint_id = checkpoint.0,
                            "model attempt failed; retrying from current checkpoint"
                        );
                        if emit(
                            client,
                            RunEvent::ModelAttemptFailed {
                                attempt: retries,
                                message,
                            },
                        )
                        .await
                        .is_err()
                        {
                            return (client_failure(), usage);
                        }

                        let delay = tokio::time::sleep(MODEL_RETRY_DELAY);
                        tokio::pin!(delay);
                        loop {
                            tokio::select! {
                                biased;
                                command = client.commands.recv() => {
                                    let interruption = match command {
                                        Some(RunCommand::InsertMessages(insertion)) => {
                                            pending_insertions.push(insertion);
                                            continue;
                                        }
                                        Some(RunCommand::BreakMessages(messages)) => messages,
                                        Some(RunCommand::Cancel) => {
                                            let _ = emit(client, RunEvent::CycleInterrupted).await;
                                            return (RunOutcome::Cancelled, usage);
                                        }
                                        Some(RunCommand::ToolResult(_)) => {
                                            return (
                                                RunOutcome::Failed(RunFailure::Protocol(
                                                    "received a tool result while waiting to retry the model".into(),
                                                )),
                                                usage,
                                            );
                                        }
                                        None => return (client_failure(), usage),
                                    };
                                    if emit(client, RunEvent::CycleInterrupted).await.is_err() {
                                        return (client_failure(), usage);
                                    }
                                    checkpoint = match super::messages::append_batches(
                                        &self.store,
                                        prepared,
                                        client,
                                        cancellation,
                                        checkpoint,
                                        std::mem::take(&mut pending_insertions),
                                    )
                                    .await
                                    {
                                        Ok((checkpoint, _)) => checkpoint,
                                        Err(outcome) => return (outcome, usage),
                                    };
                                    checkpoint = match super::messages::append_batches(
                                        &self.store,
                                        prepared,
                                        client,
                                        cancellation,
                                        checkpoint,
                                        vec![interruption],
                                    )
                                    .await
                                    {
                                        Ok((checkpoint, _)) => checkpoint,
                                        Err(outcome) => return (outcome, usage),
                                    };
                                    continue 'model;
                                }
                                _ = cancellation.cancelled() => {
                                    let _ = emit(client, RunEvent::CycleInterrupted).await;
                                    return (RunOutcome::Cancelled, usage);
                                }
                                _ = &mut delay => break,
                            }
                        }
                    }
                }
            };
            if let Some(cycle_usage) = cycle.usage {
                update_context_usage_anchor(
                    &mut context_usage_anchor,
                    cycle_usage,
                    anchored_messages,
                );
                accumulate_usage(&mut usage, cycle_usage);
            }

            if prepared.action == RunAction::Compact {
                if !cycle.calls.is_empty() {
                    return (
                        RunOutcome::Failed(RunFailure::Protocol(
                            "compaction model returned tool calls".into(),
                        )),
                        usage,
                    );
                }
                let summary = cycle.text.trim().to_string();
                if summary.is_empty() {
                    return (
                        RunOutcome::Failed(RunFailure::Protocol(
                            "compaction model returned an empty summary".into(),
                        )),
                        usage,
                    );
                }
                let event_id = format!("summary:{}", prepared.run_id);
                let summary_message = CanonicalMessage {
                    message_id: format!("runtime:{event_id}"),
                    role: Role::User,
                    origin: Origin::Runtime,
                    content: MessageContent::Parts {
                        parts: vec![crate::model::ContentPart::Text {
                            text: format!(
                                "<conversation_summary>\n{summary}\n</conversation_summary>"
                            ),
                        }],
                    },
                    runtime_event_id: Some(event_id),
                };
                checkpoint = match self
                    .store
                    .replace_checkpoint(
                        &prepared.conversation_id,
                        &prepared.run_id,
                        checkpoint,
                        &[summary_message],
                    )
                    .await
                {
                    Ok(checkpoint) => checkpoint,
                    Err(error) => return (RunOutcome::Failed(error.into()), usage),
                };
                if !pending_insertions.is_empty() {
                    checkpoint = match super::messages::append_batches(
                        &self.store,
                        prepared,
                        client,
                        cancellation,
                        checkpoint,
                        pending_insertions,
                    )
                    .await
                    {
                        Ok((next, _)) => next,
                        Err(outcome) => return (outcome, usage),
                    };
                }
                client.phase.begin_finalizing();
                let closing_insertions = match super::messages::drain_accepted(client) {
                    Ok(insertions) => insertions,
                    Err(outcome) => return (outcome, usage),
                };
                if !closing_insertions.is_empty() {
                    checkpoint = match super::messages::append_batches(
                        &self.store,
                        prepared,
                        client,
                        cancellation,
                        checkpoint,
                        closing_insertions,
                    )
                    .await
                    {
                        Ok((next, _)) => next,
                        Err(outcome) => return (outcome, usage),
                    };
                }
                let (barrier, ready) = CommitBarrier::before_continue();
                if emit(
                    client,
                    RunEvent::MessagesCommitted(MessagesCommitted {
                        checkpoint_id: checkpoint,
                        tool_round_version: 0,
                        cause: CommitCause::Compaction { summary },
                        barrier,
                    }),
                )
                .await
                .is_err()
                {
                    return (client_failure(), usage);
                }
                if let Err(outcome) = wait_for_state_ready(ready, cancellation).await {
                    return (outcome, usage);
                }
                return (RunOutcome::Completed, usage);
            }

            if cycle.calls.is_empty() {
                let assistant = CanonicalMessage {
                    message_id: format!("{}:assistant:{provider_call_index}", prepared.run_id),
                    role: Role::Assistant,
                    origin: Origin::Assistant,
                    content: MessageContent::Assistant {
                        text: cycle.text,
                        thinking: cycle.reasoning,
                        tool_round_id: None,
                        replay_state: cycle.replay_state,
                        tool_calls: Vec::new(),
                    },
                    runtime_event_id: None,
                };
                checkpoint = match self
                    .store
                    .append_checkpoint(
                        &prepared.conversation_id,
                        &prepared.run_id,
                        checkpoint,
                        &[assistant],
                    )
                    .await
                {
                    Ok(checkpoint) => checkpoint,
                    Err(error) => return (RunOutcome::Failed(error.into()), usage),
                };
                if !pending_insertions.is_empty() {
                    let inserted = match super::messages::append_batches(
                        &self.store,
                        prepared,
                        client,
                        cancellation,
                        checkpoint,
                        pending_insertions,
                    )
                    .await
                    {
                        Ok((next, inserted)) => {
                            checkpoint = next;
                            inserted
                        }
                        Err(outcome) => return (outcome, usage),
                    };
                    if inserted {
                        continue 'model;
                    }
                }
                client.phase.begin_finalizing();
                let closing_insertions = match super::messages::drain_accepted(client) {
                    Ok(insertions) => insertions,
                    Err(outcome) => return (outcome, usage),
                };
                if !closing_insertions.is_empty() {
                    checkpoint = match super::messages::append_batches(
                        &self.store,
                        prepared,
                        client,
                        cancellation,
                        checkpoint,
                        closing_insertions,
                    )
                    .await
                    {
                        Ok((next, _)) => next,
                        Err(outcome) => return (outcome, usage),
                    };
                    client.phase.resume_running();
                    continue 'model;
                }
                let (barrier, ready) = CommitBarrier::before_continue();
                if emit(
                    client,
                    RunEvent::MessagesCommitted(MessagesCommitted {
                        checkpoint_id: checkpoint,
                        tool_round_version: 0,
                        cause: CommitCause::FinalTurn,
                        barrier,
                    }),
                )
                .await
                .is_err()
                {
                    return (client_failure(), usage);
                }
                if let Err(outcome) = wait_for_state_ready(ready, cancellation).await {
                    return (outcome, usage);
                }
                return (RunOutcome::Completed, usage);
            }

            let round_id =
                ToolRoundId::new(format!("{}:round:{provider_call_index}", prepared.run_id));
            checkpoint = match super::tool_round::execute(
                &self.store,
                prepared,
                client,
                cancellation,
                checkpoint,
                super::tool_round::ToolRound {
                    id: round_id,
                    assistant: ToolRoundAssistant {
                        text: cycle.text,
                        thinking: cycle.reasoning,
                        model_call_id: cycle.model_call_id,
                        replay_state: cycle.replay_state,
                    },
                    calls: cycle.calls,
                    recovered_started_at_ms: None,
                },
                pending_insertions,
            )
            .await
            {
                Ok(checkpoint) => checkpoint,
                Err(outcome) => return (outcome, usage),
            };
        }
    }
}

async fn hydrate_tool_images(
    store: &Store,
    messages: &mut [crate::model::ProjectedMessage],
) -> crate::Result<()> {
    use crate::{
        model::{ContentPart, ProjectedContent},
        store::BlobId,
        Error,
    };

    for message in messages {
        let ProjectedContent::ToolResult(result) = &mut message.content else {
            continue;
        };
        let Some(image) = &result.image else {
            continue;
        };
        let id = BlobId::from_base64(&image.blob_id)?;
        let data = store.get_blob(&id).await?.ok_or_else(|| {
            Error::Protocol(format!("Read image Blob is missing: {}", image.blob_id))
        })?;
        result.provider_parts = vec![
            ContentPart::Text {
                text: result.content.clone(),
            },
            ContentPart::Image {
                mime_type: image.mime_type.clone(),
                data,
            },
        ];
    }
    Ok(())
}

fn context_usage_snapshot(tokens: u64) -> Usage {
    Usage {
        input_tokens: Some(tokens),
        context_input_tokens: Some(tokens),
        output_tokens: Some(0),
        total_tokens: Some(tokens),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        reasoning_tokens: Some(0),
    }
}

fn update_context_usage_anchor(
    anchor: &mut Option<ContextUsageAnchor>,
    usage: Usage,
    message_count: usize,
) {
    if let Some(context_input_tokens) = usage.context_input_tokens {
        *anchor = Some(ContextUsageAnchor {
            context_input_tokens,
            message_count,
        });
    }
}

fn accumulate_usage(total: &mut Option<Usage>, usage: Usage) {
    match total {
        Some(total) => *total += usage,
        None => *total = Some(usage),
    }
}

pub(super) async fn wait_for_state_ready(
    ready: tokio::sync::oneshot::Receiver<std::result::Result<(), String>>,
    cancellation: &CancellationToken,
) -> std::result::Result<(), RunOutcome> {
    let result = tokio::select! {
        biased;
        result = ready => result,
        _ = cancellation.cancelled() => return Err(RunOutcome::Cancelled),
    };
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(RunOutcome::Failed(RunFailure::Client(error))),
        Err(_) => Err(client_failure()),
    }
}

pub(super) async fn emit(client: &RunPort, event: RunEvent) -> Result<(), ()> {
    client.events.send(event).await.map_err(|_| ())
}

pub(super) fn client_failure() -> RunOutcome {
    RunOutcome::Failed(RunFailure::Client("client event channel closed".into()))
}

fn failure_message(failure: &RunFailure) -> String {
    match failure {
        RunFailure::Protocol(message)
        | RunFailure::Provider(message)
        | RunFailure::ProviderRefusal(message)
        | RunFailure::Store(message)
        | RunFailure::Client(message) => message.clone(),
    }
}
