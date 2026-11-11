//! Tool results: text, JSON detail, images and errors. Failures are tool results with the error
//! flag, so the agent reads the message and can correct its call.

use rmcp::ErrorData;
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;

/// The result type of every tool.
pub type ToolResult = Result<CallToolResult, ErrorData>;

/// A successful result with one text block.
pub fn text(s: impl Into<String>) -> ToolResult {
    Ok(CallToolResult::success(vec![ContentBlock::text(s.into())]))
}

/// A failed result with a message.
pub fn fail(message: impl std::fmt::Display) -> ToolResult {
    Ok(CallToolResult::error(vec![ContentBlock::text(format!(
        "error: {message}"
    ))]))
}

/// Runs a tool body and turns its error into a failed result.
pub fn finish(result: anyhow::Result<CallToolResult>) -> ToolResult {
    match result {
        Ok(r) => Ok(r),
        Err(e) => fail(format!("{e:#}")),
    }
}

/// A successful tool body with one text block.
pub fn done(s: impl Into<String>) -> anyhow::Result<CallToolResult> {
    Ok(CallToolResult::success(vec![ContentBlock::text(s.into())]))
}

/// A successful tool body with a summary and the full value as JSON.
pub fn done_json<T: Serialize>(
    summary: impl Into<String>,
    value: &T,
) -> anyhow::Result<CallToolResult> {
    let json = serde_json::to_string_pretty(value)?;
    Ok(CallToolResult::success(vec![
        ContentBlock::text(summary.into()),
        ContentBlock::text(json),
    ]))
}
