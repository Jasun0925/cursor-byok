//! Converts between canonical Messages and Cursor checkpoint message data.

mod decode;
mod encode;

pub use decode::{decode, decode_pending};
pub use encode::{stable_messages, staged_final, staged_tool_round};

const REPLAY_ENVELOPE_PREFIX: &str = "cursor-byok:v1:";
/// Replay state for reasoning written by Cursor's own models: every reasoning
/// part as `{text, signature?}`, written back verbatim.
const CURSOR_OPAQUE_REPLAY_KIND: &str = "cursor_opaque";

#[cfg(test)]
mod tests;
