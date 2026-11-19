//! Start one workspace-owned semantic search; no server-local path substitution.
use super::ToolStart;
use crate::{
    cursor::tools::{
        runtime::{CursorToolRuntime, ExecContext},
        semble,
        tool_call_result::ToolResultSender,
    },
    model::ToolCall,
    store::Store,
    Result,
};

pub(super) async fn start(
    runtime: &CursorToolRuntime,
    results: &ToolResultSender,
    call: &ToolCall,
    context: &ExecContext,
    store: Option<Store>,
) -> Result<ToolStart> {
    let messages = semble::start(runtime, results, call, context, store).await?;
    Ok(ToolStart {
        messages,
        completion: None,
    })
}
