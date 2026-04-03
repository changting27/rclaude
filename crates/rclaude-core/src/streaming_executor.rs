//! StreamingToolExecutor: concurrent tool execution with ordered result emission.
//!
//! Key behaviors:
//! - Concurrent-safe tools run in parallel; non-concurrent get exclusive access
//! - Results are emitted in original request order (not completion order)
//! - Bash errors cascade to cancel sibling tools

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::tool::{Tool, ToolResult, ToolResultContent, ToolUseContext};

/// Ordered tool result matching the original request order.
#[derive(Debug, Clone)]
pub struct OrderedToolResult {
    pub tool_use_id: String,
    pub tool_name: String,
    pub result_text: String,
    pub is_error: bool,
    pub duration_ms: u64,
}

/// A tool call reference: (tool_use_id, tool_name, input).
pub type ToolCall = (String, String, serde_json::Value);

/// Partition tool calls into concurrent-safe batches and sequential items.
/// Consecutive concurrent-safe tools form a batch; once a non-safe tool is hit,
/// everything after goes sequential (matching StreamingToolExecutor).
pub fn partition_tool_calls<'a>(
    tool_uses: &'a [ToolCall],
    tools: &[Box<dyn Tool>],
) -> (Vec<&'a ToolCall>, Vec<&'a ToolCall>) {
    let mut safe_batch = Vec::new();
    let mut sequential = Vec::new();

    for item in tool_uses {
        let is_safe = tools
            .iter()
            .find(|t| t.name() == item.1)
            .is_some_and(|t| t.is_concurrency_safe());
        if is_safe && sequential.is_empty() {
            safe_batch.push(item);
        } else {
            sequential.push(item);
        }
    }

    (safe_batch, sequential)
}

/// Execute tools with ordered results, respecting concurrency safety.
/// Bash errors cascade to cancel sibling tools via a shared cancelled flag.
pub async fn execute_streaming(
    tool_uses: &[ToolCall],
    tools: &[Box<dyn Tool>],
    ctx: &ToolUseContext,
    verbose: bool,
) -> Vec<OrderedToolResult> {
    let (safe_batch, sequential) = partition_tool_calls(tool_uses, tools);
    let mut results: Vec<Option<OrderedToolResult>> = vec![None; tool_uses.len()];

    let index_of = |id: &str| -> usize {
        tool_uses
            .iter()
            .position(|(tid, _, _)| tid == id)
            .unwrap_or(0)
    };

    // Shared cancellation flag for bash error cascade
    let cancelled = Arc::new(AtomicBool::new(false));

    // Execute safe batch in parallel
    if !safe_batch.is_empty() {
        let futs: Vec<_> = safe_batch
            .iter()
            .map(|(id, name, input)| {
                let id = id.clone();
                let name = name.clone();
                let input = input.clone();
                let cancelled = cancelled.clone();
                async move {
                    if cancelled.load(Ordering::Relaxed) {
                        return OrderedToolResult {
                            tool_use_id: id,
                            tool_name: name,
                            result_text: "Cancelled (sibling error)".into(),
                            is_error: true,
                            duration_ms: 0,
                        };
                    }
                    let start = std::time::Instant::now();
                    let result =
                        crate::query::execute_tool(&name, &input, tools, ctx, verbose).await;
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let text = extract_result_text(&result);

                    // Bash error cascade
                    if result.is_error && name == "Bash" {
                        cancelled.store(true, Ordering::Relaxed);
                    }

                    OrderedToolResult {
                        tool_use_id: id,
                        tool_name: name,
                        result_text: text,
                        is_error: result.is_error,
                        duration_ms,
                    }
                }
            })
            .collect();

        let batch_results = futures::future::join_all(futs).await;
        for r in batch_results {
            let idx = index_of(&r.tool_use_id);
            results[idx] = Some(r);
        }
    }

    // Execute sequential tools
    for (id, name, input) in sequential {
        if cancelled.load(Ordering::Relaxed) {
            let idx = index_of(id);
            results[idx] = Some(OrderedToolResult {
                tool_use_id: id.clone(),
                tool_name: name.clone(),
                result_text: "Cancelled (sibling error)".into(),
                is_error: true,
                duration_ms: 0,
            });
            continue;
        }

        let start = std::time::Instant::now();
        let result = crate::query::execute_tool(name, input, tools, ctx, verbose).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        let text = extract_result_text(&result);

        if result.is_error && name == "Bash" {
            cancelled.store(true, Ordering::Relaxed);
        }

        let idx = index_of(id);
        results[idx] = Some(OrderedToolResult {
            tool_use_id: id.clone(),
            tool_name: name.clone(),
            result_text: text,
            is_error: result.is_error,
            duration_ms,
        });
    }

    results
        .into_iter()
        .map(|r| {
            r.unwrap_or(OrderedToolResult {
                tool_use_id: String::new(),
                tool_name: String::new(),
                result_text: "Internal error: missing result".into(),
                is_error: true,
                duration_ms: 0,
            })
        })
        .collect()
}

fn extract_result_text(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_empty() {
        let uses: Vec<ToolCall> = vec![];
        let tools: Vec<Box<dyn Tool>> = vec![];
        let (safe, seq) = partition_tool_calls(&uses, &tools);
        assert!(safe.is_empty());
        assert!(seq.is_empty());
    }

    #[test]
    fn test_partition_no_tools_all_sequential() {
        let uses: Vec<ToolCall> = vec![
            ("1".into(), "Glob".into(), serde_json::json!({})),
            ("2".into(), "Grep".into(), serde_json::json!({})),
        ];
        let tools: Vec<Box<dyn Tool>> = vec![];
        let (safe, seq) = partition_tool_calls(&uses, &tools);
        assert!(safe.is_empty());
        assert_eq!(seq.len(), 2);
    }
}
