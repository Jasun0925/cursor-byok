//! Executes bounded summary calls and commits only the final complete summary.
use std::collections::HashSet;

use super::*;
use crate::run::compaction;

/// A summary call fails when the provider sends nothing for this long. Large
/// histories need time before the first token, and a long summary streams for
/// well over a minute, so the wait restarts on every streamed event.
const AUTO_COMPACTION_IDLE_LIMIT: Duration = Duration::from_secs(60);
/// Upper bound for one summary call even while it keeps streaming.
const AUTO_COMPACTION_CALL_LIMIT: Duration = Duration::from_secs(600);

fn compaction_timeout(message: String) -> Box<crate::run::ModelCycleFailure> {
    Box::new(crate::run::ModelCycleFailure {
        failure: RunFailure::Provider(message),
        partial_text: String::new(),
        partial_reasoning: String::new(),
        usage: None,
        retryable: false,
    })
}

impl RunEngine {
    pub(super) async fn auto_compact(
        &self,
        prepared: &PreparedRun,
        checkpoint: crate::model::CheckpointId,
        messages: &[CanonicalMessage],
        client: &mut RunPort,
        cancellation: &CancellationToken,
    ) -> std::result::Result<(crate::model::CheckpointId, Option<Usage>), RunOutcome> {
        let current_ids = prepared
            .initial_messages
            .iter()
            .map(|message| message.message_id.as_str())
            .collect::<HashSet<_>>();
        let (compactable, retained_request_context) = compaction::partition(messages, &current_ids);
        if compactable.is_empty() {
            let projected = crate::model::project_messages(messages)
                .map_err(|error| RunOutcome::Failed(error.into()))?;
            let message = compaction::validate_compacted(prepared, &projected)
                .err()
                .unwrap_or_else(|| {
                    "context overflow after compaction: no conversation history can be compacted"
                        .into()
                });
            return Err(RunOutcome::Failed(RunFailure::Protocol(message)));
        }

        emit(client, RunEvent::AutoCompactionStarted)
            .await
            .map_err(|_| client_failure())?;
        let mut history = crate::model::project_messages(&compactable)
            .map_err(|error| RunOutcome::Failed(error.into()))?;
        if !prepared.model.strip_images {
            hydrate_tool_images(&self.store, &mut history)
                .await
                .map_err(|error| RunOutcome::Failed(error.into()))?;
        }
        let mut batches = compaction::Batches::new(history, prepared.model.strip_images)
            .map_err(|error| RunOutcome::Failed(error.into()))?;
        let mut model = prepared.model.clone();
        model.max_output_tokens = Some(compaction::OUTPUT_TOKENS);
        model.reasoning.enabled = false;
        model.reasoning.effort = None;
        let prompt = crate::model::PromptSpec {
            instructions: format!(
                "{}\n\n{}",
                compaction::instructions().map_err(|error| RunOutcome::Failed(error.into()))?,
                compaction::TRUST_INSTRUCTION
            ),
            tools: Vec::new(),
        };
        let mut pending_insertions = Vec::new();
        let mut summary = String::new();
        let mut compaction_usage = None;
        let mut last_call_index = None;
        let mut retry_history = None;
        let mut summary_window = model
            .context_window_tokens
            .unwrap_or(compaction::DEFAULT_CONTEXT_WINDOW);
        let summary_result = loop {
            if cancellation.is_cancelled() {
                return Err(RunOutcome::Cancelled);
            }
            let next = match retry_history.take() {
                Some(history) => Ok(Some(history)),
                None => batches.next(&summary, &prompt, Some(summary_window)),
            };
            let history = match next {
                Ok(Some(history)) => history,
                Ok(None) => break Ok(summary),
                Err(message) => break Err(RunFailure::Protocol(message)),
            };
            let provider_call_index = self
                .store
                .begin_provider_call(&prepared.run_id)
                .await
                .map_err(|error| RunOutcome::Failed(error.into()))?;
            last_call_index = Some(provider_call_index);
            let invocation = crate::model::ModelInvocation {
                call_id: format!("{}:{provider_call_index}", prepared.run_id),
                run_id: prepared.run_id.to_string(),
                conversation_id: prepared.conversation_id.to_string(),
                prompt_cache_enabled: true,
                provider_call_index,
                request: crate::model::ModelRequest {
                    prompt: prompt.clone(),
                    model: model.clone(),
                    history: history.clone(),
                },
            };
            let cycle_cancellation = cancellation.child_token();
            let (silent_events, mut discarded_events) = tokio::sync::mpsc::channel(256);
            let progress = std::sync::Arc::new(tokio::sync::Notify::new());
            let drain = {
                let progress = progress.clone();
                tokio::spawn(async move {
                    while discarded_events.recv().await.is_some() {
                        progress.notify_one();
                    }
                })
            };
            let mut timed_out = false;
            let mut break_messages = None;
            let cycle = {
                let cycle = consume_model_cycle(
                    self.provider.stream(invocation, cycle_cancellation.clone()),
                    &silent_events,
                    &cycle_cancellation,
                );
                tokio::pin!(cycle);
                let idle = tokio::time::sleep(AUTO_COMPACTION_IDLE_LIMIT);
                tokio::pin!(idle);
                let call_limit = tokio::time::sleep(AUTO_COMPACTION_CALL_LIMIT);
                tokio::pin!(call_limit);
                loop {
                    tokio::select! {
                        biased;
                        command = client.commands.recv() => match command {
                            Some(RunCommand::InsertMessages(insertion)) => {
                                pending_insertions.push(insertion);
                            }
                            Some(RunCommand::BreakMessages(messages)) => {
                                cycle_cancellation.cancel();
                                break_messages = Some(messages);
                                break cycle.await;
                            }
                            Some(RunCommand::Cancel) => {
                                cycle_cancellation.cancel();
                                let _ = cycle.await;
                                return Err(RunOutcome::Cancelled);
                            }
                            Some(RunCommand::ToolResult(_)) => {
                                cycle_cancellation.cancel();
                                let _ = cycle.await;
                                return Err(RunOutcome::Failed(RunFailure::Protocol(
                                    "received a tool result while automatic compaction was running".into(),
                                )));
                            }
                            None => {
                                cycle_cancellation.cancel();
                                let _ = cycle.await;
                                return Err(client_failure());
                            }
                        },
                        result = &mut cycle => break result,
                        _ = progress.notified() => {
                            idle.as_mut().reset(
                                tokio::time::Instant::now() + AUTO_COMPACTION_IDLE_LIMIT,
                            );
                        }
                        _ = &mut idle => {
                            timed_out = true;
                            cycle_cancellation.cancel();
                            break Err(compaction_timeout(format!(
                                "automatic compaction received nothing from the model for {} seconds",
                                AUTO_COMPACTION_IDLE_LIMIT.as_secs()
                            )));
                        }
                        _ = &mut call_limit => {
                            timed_out = true;
                            cycle_cancellation.cancel();
                            break Err(compaction_timeout(format!(
                                "automatic compaction did not finish within {} seconds",
                                AUTO_COMPACTION_CALL_LIMIT.as_secs()
                            )));
                        }
                    }
                }
            };
            drop(silent_events);
            let _ = drain.await;
            let cycle_usage = match &cycle {
                Ok(cycle) => cycle.usage,
                Err(failure) => failure.usage,
            };
            if let Some(usage) = cycle_usage {
                accumulate_usage(&mut compaction_usage, usage);
            }
            if let Some(messages) = break_messages {
                // An injected user message interrupts this provider call, not the
                // compaction transaction. Retry the same source segment, then deliver
                // the new message after the complete summary has been committed.
                pending_insertions.push(messages);
                retry_history = Some(history);
                continue;
            }
            if let Ok(cycle) = &cycle {
                if !timed_out
                    && cycle.calls.is_empty()
                    && cycle.finish_reason == crate::provider::FinishReason::Stop
                    && !cycle.text.trim().is_empty()
                {
                    summary = cycle.text.trim().to_string();
                    continue;
                }
            }
            let failure = match cycle {
                Err(failure) => failure.failure,
                Ok(_) => RunFailure::Provider(
                    "automatic summary was truncated, empty, or requested tools".into(),
                ),
            };
            if compaction::is_context_overflow(&format!("{failure:?}"))
                && summary_window > compaction::OUTPUT_TOKENS * 2
            {
                summary_window /= 2;
                batches.rewind(history);
                tracing::warn!(
                    summary_window,
                    "summary exceeded provider context; retrying smaller batch"
                );
                continue;
            }
            tracing::warn!(
                ?failure,
                "automatic compaction failed; preserving original history"
            );
            break Err(failure);
        };
        let summary = match summary_result {
            Ok(summary) => summary,
            Err(failure) => {
                // Incoming messages belong to the conversation even if summarization
                // failed. No partial summary has been committed at this point.
                crate::run::messages::append_batches(
                    &self.store,
                    prepared,
                    client,
                    cancellation,
                    checkpoint,
                    pending_insertions,
                )
                .await?;
                return Err(RunOutcome::Failed(failure));
            }
        };
        let provider_call_index = last_call_index.expect("compaction produced a summary");
        let event_id = format!("summary:auto:{}:{provider_call_index}", prepared.run_id);
        let summary_message = CanonicalMessage {
            message_id: format!("runtime:{event_id}"),
            role: Role::User,
            origin: Origin::Runtime,
            content: MessageContent::Parts {
                parts: vec![crate::model::ContentPart::Text {
                    text: format!("<conversation_summary>\n{summary}\n</conversation_summary>"),
                }],
            },
            runtime_event_id: Some(event_id),
        };
        let mut replacement = retained_request_context.into_iter().collect::<Vec<_>>();
        replacement.push(summary_message);
        replacement.extend(prepared.initial_messages.iter().cloned());
        let projected_replacement = crate::model::project_messages(&replacement)
            .map_err(|error| RunOutcome::Failed(error.into()))?;
        compaction::validate_compacted(prepared, &projected_replacement)
            .map_err(|message| RunOutcome::Failed(RunFailure::Protocol(message)))?;
        let mut checkpoint = self
            .store
            .replace_checkpoint(
                &prepared.conversation_id,
                &prepared.run_id,
                checkpoint,
                &replacement,
            )
            .await
            .map_err(|error| RunOutcome::Failed(error.into()))?;
        let (barrier, ready) = CommitBarrier::before_continue();
        emit(
            client,
            RunEvent::MessagesCommitted(MessagesCommitted {
                checkpoint_id: checkpoint,
                tool_round_version: 0,
                cause: CommitCause::Compaction { summary },
                barrier,
            }),
        )
        .await
        .map_err(|_| client_failure())?;
        wait_for_state_ready(ready, cancellation).await?;
        emit(client, RunEvent::AutoCompactionCompleted)
            .await
            .map_err(|_| client_failure())?;
        emit(client, RunEvent::UsageSnapshot(context_usage_snapshot(0)))
            .await
            .map_err(|_| client_failure())?;
        checkpoint = crate::run::messages::append_batches(
            &self.store,
            prepared,
            client,
            cancellation,
            checkpoint,
            pending_insertions,
        )
        .await?
        .0;
        Ok((checkpoint, compaction_usage))
    }
}
