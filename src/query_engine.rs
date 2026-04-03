#![allow(dead_code)] // QueryEngine fields/variants used at runtime, not statically
//! QueryEngine: the core agentic loop extracted as a reusable module.
//!
//! Owns the query lifecycle:
//! - Message construction and system prompt
//! - API call with retry (429/529/prompt-too-long)
//! - Stream parsing (text/tool_use/thinking)
//! - Tool execution (parallel/serial partitioning)
//! - Auto-compact (micro + full + circuit breaker)
//! - Output continuation (max_tokens recovery)
//! - Tool result budget enforcement
//! - Hook lifecycle (PreTool/PostTool/Compact/Stop)

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use rclaude_core::auto_compact::AutoCompactState;
use rclaude_core::hooks::{HookEvent, HookRegistry};
use rclaude_core::message::{ContentBlock, Message};
use rclaude_core::model;
use rclaude_core::state::AppState;
use rclaude_core::streaming_executor::{OrderedToolResult, ToolCall};
use rclaude_core::tool::{Tool, ToolUseContext};
use rclaude_core::tool_result_storage::ContentReplacementState;

use rclaude_api::streaming::StreamContentEvent;
use rclaude_api::types::{CacheControl, CreateMessageRequest, SystemBlock, ToolDefinition, Usage};

pub type SharedState = Arc<RwLock<AppState>>;

/// Configuration for a QueryEngine instance.
pub struct QueryEngineConfig {
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub max_turns: usize,
    pub max_budget_usd: Option<f64>,
    pub fallback_model: Option<String>,
    pub system_prompt_override: Option<String>,
    pub append_system_prompt: Option<String>,
    pub output_format: String,
    pub verbose: bool,
}

impl Default for QueryEngineConfig {
    fn default() -> Self {
        Self {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            max_turns: 10,
            max_budget_usd: None,
            fallback_model: None,
            system_prompt_override: None,
            append_system_prompt: None,
            output_format: "text".into(),
            verbose: false,
        }
    }
}

/// Turn-scoped mutable state, reset per submit_message call where noted.
#[derive(Default)]
pub struct TurnState {
    pub max_output_recovery_count: u32,
    pub has_attempted_reactive_compact: bool,
    pub stream_retry_count: u32,
    pub consecutive_denials: u32,
}

/// Result of a single agentic turn.
pub struct TurnResult {
    pub stop_reason: TurnStopReason,
    pub tool_calls: usize,
}

/// Why a turn ended.
#[derive(Debug, Clone)]
pub enum TurnStopReason {
    EndTurn,
    MaxTurns,
    BudgetExceeded,
    PermissionDenials,
    Error(String),
}

/// Callback trait for UI rendering during a turn.
/// Implement this to customize how text, tool calls, and results are displayed.
pub trait TurnRenderer: Send + Sync {
    /// Called for each text delta from the stream.
    fn on_text_delta(&self, text: &str);
    /// Called for thinking deltas.
    fn on_thinking_delta(&self, _text: &str) {}
    /// Called when tool calls are about to execute.
    fn on_tool_calls(&self, calls: &[ToolCall]);
    /// Called with tool results after execution.
    fn on_tool_results(&self, results: &[OrderedToolResult]);
    /// Called when retry is happening.
    fn on_retry(&self, reason: &str, attempt: u32, max: u32, delay_secs: u64);
    /// Called when auto-compact happens.
    fn on_compact(&self, before: usize, after: usize);
    /// Called at end of turn with status bar info.
    fn on_status(&self, model: &str, cost: f64);
    /// Called on stream error.
    fn on_error(&self, message: &str);
}

/// The core query engine. One per conversation.
pub struct QueryEngine {
    pub config: QueryEngineConfig,
    pub hooks: HookRegistry,
    pub auto_compact: AutoCompactState,
    pub turn_state: TurnState,
    pub tool_stats: rclaude_services::tool_execution::ToolStats,
    pub content_replacement_state: ContentReplacementState,
}

impl QueryEngine {
    pub fn new(config: QueryEngineConfig, hooks: HookRegistry) -> Self {
        Self {
            config,
            hooks,
            auto_compact: AutoCompactState::default(),
            turn_state: TurnState::default(),
            tool_stats: rclaude_services::tool_execution::ToolStats::default(),
            content_replacement_state: ContentReplacementState::default(),
        }
    }

    /// Execute a full agentic turn: send user message, loop through tool calls until done.
    pub async fn submit_message(
        &mut self,
        user_input: &str,
        state: &SharedState,
        renderer: &dyn TurnRenderer,
    ) -> anyhow::Result<TurnResult> {
        // Reset per-turn state
        self.turn_state.has_attempted_reactive_compact = false;
        self.turn_state.stream_retry_count = 0;

        let (api_key, model_name, max_tokens, cwd, verbose) = {
            let s = state.read().await;
            let key = s
                .config
                .api_key
                .clone()
                .or_else(rclaude_core::auth::get_api_key)
                .unwrap_or_default();
            (
                key,
                model::resolve_model(&s.config.model),
                s.config.max_tokens,
                s.cwd.clone(),
                self.config.verbose || s.config.verbose,
            )
        };

        if api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "No API key. Set ANTHROPIC_API_KEY or run `rclaude login`"
            ));
        }

        let mut client = rclaude_api::client::AnthropicClient::new(&api_key);
        if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
            client = client.with_base_url(base_url);
        }

        let all_tools = rclaude_tools::get_all_tools();
        let tools = self.filter_tools(all_tools);
        let tool_defs = Self::build_tool_defs(&tools);

        let system_prompt = {
            let s = state.read().await;
            rclaude_core::system_prompt::build_system_prompt(
                &s,
                &tools,
                self.config.system_prompt_override.as_deref(),
                self.config.append_system_prompt.as_deref(),
            )
            .await
        };
        let system_blocks = vec![SystemBlock::Text {
            text: system_prompt,
            cache_control: Some(CacheControl {
                control_type: "ephemeral".to_string(),
            }),
        }];

        // Add user message
        {
            let mut s = state.write().await;
            s.messages.push(Message::user(user_input));
        }

        let (_abort_tx, abort_rx) = tokio::sync::watch::channel(false);
        let tool_ctx = ToolUseContext {
            cwd: cwd.clone(),
            permission_mode: state.read().await.permission_mode,
            debug: false,
            verbose,
            abort_signal: abort_rx,
            app_state: Some(state.clone()),
        };

        let context_window = model::context_window_for_model(&model_name);
        let max_output = model::max_output_for_model(&model_name);
        let mut total_tool_calls = 0usize;

        for _turn in 0..self.config.max_turns {
            // Phase 1: MicroCompact
            self.run_micro_compact(state, context_window, verbose).await;

            // Phase 2: AutoCompact
            self.run_auto_compact(
                state,
                context_window,
                max_output as usize,
                &cwd,
                verbose,
                renderer,
            )
            .await;

            // Build and send API request
            let api_messages = {
                let s = state.read().await;
                messages_to_api(&s.messages)
            };
            let request = CreateMessageRequest {
                model: model_name.clone(),
                max_tokens,
                messages: api_messages,
                system: Some(system_blocks.clone()),
                tools: Some(tool_defs.clone()),
                stream: true,
                temperature: None,
                top_p: None,
                metadata: None,
            };

            // Stream with retry
            let mut stream = self
                .call_api_with_retry(&client, &request, renderer)
                .await?;

            // Parse stream
            let parse = self.process_stream(&mut stream, verbose, renderer).await;

            // Record usage
            {
                let mut s = state.write().await;
                s.record_usage(
                    &model_name,
                    parse.usage.input_tokens,
                    parse.usage.output_tokens,
                    parse.usage.cache_creation_input_tokens,
                    parse.usage.cache_read_input_tokens,
                );
            }

            // Budget check
            if let Some(budget) = self.config.max_budget_usd {
                let s = state.read().await;
                if s.total_cost_usd >= budget {
                    return Ok(TurnResult {
                        stop_reason: TurnStopReason::BudgetExceeded,
                        tool_calls: total_tool_calls,
                    });
                }
            }

            // Store assistant message
            {
                let mut s = state.write().await;
                let msg = Message::assistant(parse.content_blocks.clone());
                let _ = rclaude_services::session_storage::append_to_transcript(
                    &s.session_id,
                    &msg,
                    &s.cwd,
                )
                .await;
                s.messages.push(msg);
            }

            // Handle prompt-too-long error from stream
            if let Some(ref err_msg) = parse.error {
                let is_ptl = err_msg.to_lowercase().contains("prompt is too long")
                    || err_msg.to_lowercase().contains("too many tokens");
                if is_ptl && !self.turn_state.has_attempted_reactive_compact {
                    self.turn_state.has_attempted_reactive_compact = true;
                    renderer.on_error("Prompt too long — auto-compacting...");
                    self.run_reactive_compact(state, verbose).await;
                    continue;
                }

                // Retryable stream errors
                let is_retryable = err_msg.contains("overloaded")
                    || err_msg.contains("rate limit")
                    || err_msg.contains("timeout")
                    || err_msg.contains("connection");
                if is_retryable && self.turn_state.stream_retry_count < 2 {
                    self.turn_state.stream_retry_count += 1;
                    renderer.on_error(&format!(
                        "Stream error, retrying ({}/2)...",
                        self.turn_state.stream_retry_count
                    ));
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                self.turn_state.stream_retry_count = 0;
                renderer.on_error(err_msg);
                break;
            }

            // Max tokens recovery (auto-continue)
            if rclaude_core::query::is_max_tokens_stop(parse.stop_reason.as_deref())
                && !parse.has_tool_use
            {
                self.turn_state.max_output_recovery_count += 1;
                if self.turn_state.max_output_recovery_count
                    <= rclaude_core::query::MAX_OUTPUT_RECOVERY_LIMIT
                {
                    let mut s = state.write().await;
                    s.messages
                        .push(rclaude_core::query::build_continue_message());
                    continue;
                }
            }

            if parse.has_tool_use {
                self.turn_state.max_output_recovery_count = 0;
            }

            // No tool use → turn complete
            if !parse.has_tool_use {
                let s = state.read().await;
                renderer.on_status(&model_name, s.total_cost_usd);
                let _ = self.hooks.run(HookEvent::Stop, &cwd, &HashMap::new()).await;
                return Ok(TurnResult {
                    stop_reason: TurnStopReason::EndTurn,
                    tool_calls: total_tool_calls,
                });
            }

            // Execute tools
            total_tool_calls += parse.tool_uses.len();
            renderer.on_tool_calls(&parse.tool_uses);

            // PreToolUse hooks
            for (_, name, input) in &parse.tool_uses {
                let env = HashMap::from([
                    ("TOOL_NAME".into(), name.clone()),
                    ("TOOL_INPUT".into(), input.to_string()),
                ]);
                let _ = self.hooks.run(HookEvent::PreToolUse, &cwd, &env).await;
            }

            let mut results = rclaude_core::streaming_executor::execute_streaming(
                &parse.tool_uses,
                &tools,
                &tool_ctx,
                verbose,
            )
            .await;

            // PostToolUse hooks
            for r in &results {
                let env = HashMap::from([
                    ("TOOL_NAME".into(), r.tool_name.clone()),
                    (
                        "TOOL_RESULT".into(),
                        if r.is_error { "error" } else { "success" }.into(),
                    ),
                    ("TOOL_DURATION_MS".into(), r.duration_ms.to_string()),
                ]);
                let _ = self.hooks.run(HookEvent::PostToolUse, &cwd, &env).await;
            }

            // Persist large results + budget enforcement
            {
                let s = state.read().await;
                let session_dir = rclaude_core::tool_result_storage::get_session_results_dir(
                    &s.session_id,
                    &s.cwd,
                );
                rclaude_core::tool_result_storage::process_results(
                    &mut results,
                    &mut self.content_replacement_state,
                    &session_dir,
                )
                .await;
            }
            rclaude_core::query::enforce_tool_result_budget(&mut results);

            renderer.on_tool_results(&results);

            // Denial tracking
            let all_denied = !results.is_empty()
                && results
                    .iter()
                    .all(|r| r.is_error && r.result_text.contains("Permission denied"));
            if all_denied {
                self.turn_state.consecutive_denials += 1;
                if self.turn_state.consecutive_denials >= 3 {
                    return Ok(TurnResult {
                        stop_reason: TurnStopReason::PermissionDenials,
                        tool_calls: total_tool_calls,
                    });
                }
            } else {
                self.turn_state.consecutive_denials = 0;
            }

            // Update tool stats
            for r in &results {
                self.tool_stats.total_calls += 1;
                self.tool_stats.total_duration_ms += r.duration_ms;
                *self
                    .tool_stats
                    .calls_by_tool
                    .entry(r.tool_name.clone())
                    .or_default() += 1;
                if r.is_error {
                    *self
                        .tool_stats
                        .errors_by_tool
                        .entry(r.tool_name.clone())
                        .or_default() += 1;
                }
            }

            // Store tool results
            let msg = rclaude_core::query::build_tool_result_message(&results);
            {
                let mut s = state.write().await;
                let _ = rclaude_services::session_storage::append_to_transcript(
                    &s.session_id,
                    &msg,
                    &s.cwd,
                )
                .await;
                s.messages.push(msg);
            }
        }

        // Post-turn: memory extraction
        self.extract_memories(state).await;

        Ok(TurnResult {
            stop_reason: TurnStopReason::MaxTurns,
            tool_calls: total_tool_calls,
        })
    }

    // ── Internal helpers ──

    fn filter_tools(&self, tools: Vec<Box<dyn Tool>>) -> Vec<Box<dyn Tool>> {
        tools
            .into_iter()
            .filter(|t| {
                let name = t.name();
                if !self.config.allowed_tools.is_empty()
                    && !self.config.allowed_tools.iter().any(|a| a == name)
                {
                    return false;
                }
                !self.config.disallowed_tools.iter().any(|d| d == name)
            })
            .collect()
    }

    fn build_tool_defs(tools: &[Box<dyn Tool>]) -> Vec<ToolDefinition> {
        tools
            .iter()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: serde_json::to_value(t.input_schema()).unwrap_or_default(),
            })
            .collect()
    }

    async fn run_micro_compact(&self, state: &SharedState, context_window: usize, verbose: bool) {
        let mut s = state.write().await;
        if rclaude_core::micro_compact::should_micro_compact(&s.messages, context_window) {
            let count = rclaude_core::micro_compact::micro_compact(&mut s.messages, 6);
            if verbose && count > 0 {
                tracing::info!("Micro-compacted {} tool results", count);
            }
        }
    }

    async fn run_auto_compact(
        &mut self,
        state: &SharedState,
        context_window: usize,
        max_output: usize,
        cwd: &std::path::Path,
        verbose: bool,
        renderer: &dyn TurnRenderer,
    ) {
        let mut s = state.write().await;
        if !rclaude_core::auto_compact::should_auto_compact(
            &s.messages,
            context_window,
            max_output,
            &self.auto_compact,
        ) {
            return;
        }

        let _ = self
            .hooks
            .run(HookEvent::PreCompact, cwd, &HashMap::new())
            .await;
        let before = s.messages.len();
        let api_key = s
            .config
            .api_key
            .clone()
            .or_else(rclaude_core::auth::get_api_key)
            .unwrap_or_default();
        let model_c = s.config.model.clone();

        match rclaude_services::compact::compact_conversation(&s.messages, &api_key, &model_c, 6)
            .await
        {
            Ok(result) => {
                s.messages = rclaude_services::compact::build_compacted_messages(
                    &s.messages,
                    &result.summary,
                    6,
                );
                self.auto_compact.record_success(
                    rclaude_core::context_window::estimate_conversation_tokens(&s.messages),
                );
                renderer.on_compact(before, s.messages.len());
            }
            Err(e) => {
                if self.auto_compact.record_failure() {
                    s.messages = rclaude_core::context_window::compact_messages(&s.messages, 8);
                    if verbose {
                        tracing::warn!("Compact API failed ({e}), used fallback");
                    }
                }
            }
        }

        let _ = self
            .hooks
            .run(HookEvent::PostCompact, cwd, &HashMap::new())
            .await;
    }

    async fn run_reactive_compact(&self, state: &SharedState, verbose: bool) {
        let mut s = state.write().await;
        let api_key = s
            .config
            .api_key
            .clone()
            .or_else(rclaude_core::auth::get_api_key)
            .unwrap_or_default();
        let model_c = s.config.model.clone();
        match rclaude_services::compact::compact_conversation(&s.messages, &api_key, &model_c, 6)
            .await
        {
            Ok(result) => {
                s.messages = rclaude_services::compact::build_compacted_messages(
                    &s.messages,
                    &result.summary,
                    6,
                );
                if verbose {
                    tracing::info!(
                        "Reactive compact: ~{} tokens saved",
                        result.tokens_saved_estimate
                    );
                }
            }
            Err(_) => {
                s.messages = rclaude_core::context_window::compact_messages(&s.messages, 8);
            }
        }
    }

    async fn call_api_with_retry(
        &self,
        client: &rclaude_api::client::AnthropicClient,
        request: &CreateMessageRequest,
        renderer: &dyn TurnRenderer,
    ) -> anyhow::Result<rclaude_api::streaming::MessageStream> {
        let mut retry_state =
            rclaude_api::retry::RetryState::new(rclaude_api::retry::default_max_retries());
        loop {
            match client.create_message_stream(request) {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    let status = match &e {
                        rclaude_core::error::RclaudeError::Api {
                            status: Some(s), ..
                        } => *s,
                        _ => 0,
                    };
                    let (retry_status, delay) =
                        retry_state.handle_error(status, None, &e.to_string());
                    match retry_status {
                        Some(rclaude_api::retry::RetryStatus::Retrying {
                            attempt,
                            max_attempts,
                            reason,
                            ..
                        }) => {
                            renderer.on_retry(&reason, attempt, max_attempts, delay.as_secs());
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        Some(rclaude_api::retry::RetryStatus::GaveUp { reason }) => {
                            return Err(anyhow::anyhow!("Gave up: {reason}"));
                        }
                        Some(rclaude_api::retry::RetryStatus::Fatal { message, .. }) => {
                            return Err(anyhow::anyhow!("{message}"));
                        }
                        None => return Err(anyhow::anyhow!("{e}")),
                    }
                }
            }
        }
    }

    async fn process_stream(
        &self,
        stream: &mut rclaude_api::streaming::MessageStream,
        verbose: bool,
        renderer: &dyn TurnRenderer,
    ) -> StreamParseResult {
        let mut result = StreamParseResult::default();
        let mut current_text = String::new();
        let mut current_tool_json = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_block_type = String::new();

        while let Some(event) = stream.next_event().await {
            match event {
                StreamContentEvent::TextDelta { text } => {
                    renderer.on_text_delta(&text);
                    current_text.push_str(&text);
                }
                StreamContentEvent::ThinkingDelta { thinking } => {
                    renderer.on_thinking_delta(&thinking);
                    if verbose {
                        tracing::debug!("thinking: {}", &thinking[..thinking.len().min(100)]);
                    }
                }
                StreamContentEvent::InputJsonDelta { partial_json } => {
                    current_tool_json.push_str(&partial_json);
                }
                StreamContentEvent::ContentBlockStart {
                    block_type,
                    tool_use_id,
                    tool_name,
                    ..
                } => {
                    current_block_type = block_type;
                    if let Some(id) = tool_use_id {
                        current_tool_id = id;
                    }
                    if let Some(name) = tool_name {
                        current_tool_name = name;
                    }
                }
                StreamContentEvent::ContentBlockStop { .. } => {
                    match current_block_type.as_str() {
                        "text" if !current_text.is_empty() => {
                            result.content_blocks.push(ContentBlock::Text {
                                text: std::mem::take(&mut current_text),
                            });
                        }
                        "tool_use" => {
                            result.has_tool_use = true;
                            let input: serde_json::Value =
                                serde_json::from_str(&current_tool_json).unwrap_or_default();
                            result.tool_uses.push((
                                current_tool_id.clone(),
                                current_tool_name.clone(),
                                input.clone(),
                            ));
                            result.content_blocks.push(ContentBlock::ToolUse {
                                id: std::mem::take(&mut current_tool_id),
                                name: std::mem::take(&mut current_tool_name),
                                input,
                            });
                            current_tool_json.clear();
                        }
                        _ => {}
                    }
                    current_block_type.clear();
                }
                StreamContentEvent::MessageComplete { stop_reason, usage } => {
                    result.stop_reason = stop_reason;
                    if let Some(u) = usage {
                        result.usage.output_tokens += u.output_tokens;
                        result.usage.input_tokens += u.input_tokens;
                    }
                }
                StreamContentEvent::Error { message } => {
                    result.error = Some(message);
                    break;
                }
            }
        }
        result
    }

    async fn extract_memories(&self, state: &SharedState) {
        let s = state.read().await;
        if rclaude_services::session_memory::should_extract_memory(&s.messages) {
            let facts = rclaude_services::session_memory::extract_facts_from_messages(&s.messages);
            if !facts.is_empty() {
                let entries: Vec<rclaude_services::session_memory::MemoryEntry> = facts
                    .into_iter()
                    .map(|f| rclaude_services::session_memory::MemoryEntry {
                        key: f.clone(),
                        value: f,
                        source: "auto".into(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    })
                    .collect();
                let _ = rclaude_services::session_memory::save_memory(&s.cwd, &entries).await;
            }
        }
    }
}

/// Internal: parsed result from streaming an API response.
#[derive(Default)]
struct StreamParseResult {
    content_blocks: Vec<ContentBlock>,
    tool_uses: Vec<ToolCall>,
    has_tool_use: bool,
    stop_reason: Option<String>,
    usage: Usage,
    error: Option<String>,
}

/// Convert internal messages to API format.
pub fn messages_to_api(messages: &[Message]) -> Vec<rclaude_api::types::ApiMessage> {
    use rclaude_api::types::{ApiContentBlock, ApiMessage};
    use rclaude_core::message::{ContentBlock, Role};

    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "user",
            };
            let content: Vec<ApiContentBlock> = m
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => ApiContentBlock::Text { text: text.clone() },
                    ContentBlock::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => ApiContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                        is_error: *is_error,
                    },
                    ContentBlock::Thinking { thinking } => ApiContentBlock::Thinking {
                        thinking: thinking.clone(),
                    },
                    ContentBlock::Image { source: _ } => ApiContentBlock::Text {
                        text: "[image]".to_string(),
                    },
                })
                .collect();
            ApiMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect()
}
