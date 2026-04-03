//! AgentTool: spawn sub-agents for parallel task execution.
//! Q03: In-process execution via query loop (not subprocess).

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::task;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

use crate::agent_loader::{self, ActiveAgent};
use crate::agent_types;

pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }

    fn description(&self) -> &str {
        "Launch a new agent to handle complex, multi-step tasks autonomously. \
         Each agent runs with its own context and tool restrictions."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform"
                },
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Agent type: general-purpose, Explore, Plan, Verification, or custom"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run in background (default: false)"
                },
                "model": {
                    "type": "string",
                    "description": "Model override: sonnet, opus, haiku, or inherit"
                }
            },
            "required": ["description", "prompt"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: prompt".into()))?;
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Agent task");
        let agent_type_str = input
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general-purpose");
        let run_in_background = input
            .get("run_in_background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let model_override = input.get("model").and_then(|v| v.as_str());

        // Load custom agents and merge with built-ins
        let custom_agents = agent_loader::load_all_agents(&ctx.cwd).await;
        let active_agents = agent_loader::get_active_agents(&custom_agents);

        // Find the requested agent
        let agent = active_agents
            .iter()
            .find(|a| a.name().eq_ignore_ascii_case(agent_type_str))
            .unwrap_or_else(|| {
                active_agents
                    .iter()
                    .find(|a| a.name() == "general-purpose")
                    .unwrap_or(&active_agents[0])
            });

        // Resolve model
        let model = match model_override {
            Some(m) => m.to_string(),
            None => {
                let agent_model = agent.model();
                if agent_model == "inherit" {
                    std::env::var("CLAUDE_MODEL")
                        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string())
                } else {
                    rclaude_core::model::resolve_model(agent_model)
                }
            }
        };

        // Background: still use subprocess (can't share tokio runtime easily)
        if run_in_background {
            return run_background_agent(agent, prompt, description, &model, ctx).await;
        }

        // Q03: In-process execution — run a query loop with agent's tools
        // Pass parent messages for fork context (cache-sharing)
        let parent_messages = if let Some(ref state) = ctx.app_state {
            let s = state.read().await;
            Some(s.messages.clone())
        } else {
            None
        };
        run_inprocess_agent(
            agent,
            prompt,
            &model,
            &ctx.cwd,
            ctx.verbose,
            parent_messages,
        )
        .await
    }
}

/// Run agent in-process using the API client directly.
/// This avoids subprocess overhead and enables streaming output.
async fn run_inprocess_agent(
    agent: &ActiveAgent,
    prompt: &str,
    model: &str,
    cwd: &std::path::Path,
    verbose: bool,
    parent_messages: Option<Vec<rclaude_core::message::Message>>,
) -> Result<ToolResult> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| {
            rclaude_core::config::Config::load()
                .api_key
                .ok_or(std::env::VarError::NotPresent)
        })
        .map_err(|_| RclaudeError::Config("No API key available for agent".into()))?;

    let client = rclaude_api::client::AnthropicClient::new(&api_key);

    // Build agent-specific system prompt
    let system = agent.system_prompt().to_string();

    // Load agent memory if configured, with snapshot sync
    let memory = if let ActiveAgent::Custom(c) = agent {
        if let Some(ref scope) = c.memory_scope {
            // Check and sync memory snapshot before loading
            let snap_action =
                agent_loader::check_agent_memory_snapshot(&c.agent_type, scope, cwd).await;
            match snap_action {
                agent_loader::SnapshotAction::Initialize { ref timestamp } => {
                    if verbose {
                        eprintln!("Initializing agent memory from project snapshot...");
                    }
                    agent_loader::initialize_from_snapshot(&c.agent_type, scope, cwd, timestamp)
                        .await;
                }
                agent_loader::SnapshotAction::PromptUpdate { ref timestamp } => {
                    if verbose {
                        eprintln!("Updating agent memory from newer project snapshot...");
                    }
                    agent_loader::replace_from_snapshot(&c.agent_type, scope, cwd, timestamp).await;
                }
                agent_loader::SnapshotAction::None => {}
            }
            agent_loader::load_agent_memory_prompt(scope, &c.agent_type, cwd).await
        } else {
            None
        }
    } else {
        None
    };

    let full_system = if let Some(mem) = memory {
        format!("{system}\n\n<agent-memory>\n{mem}\n</agent-memory>")
    } else {
        system
    };

    // Get available tools, filtered by agent restrictions
    let all_tools = crate::get_all_tools();
    let tool_defs: Vec<rclaude_api::types::ToolDefinition> = all_tools
        .iter()
        .filter(|t| {
            let name = t.name();
            match agent {
                ActiveAgent::BuiltIn(def) => agent_types::is_tool_allowed(def, name),
                ActiveAgent::Custom(c) => {
                    if let Some(ref denied) = c.disallowed_tools {
                        if denied.iter().any(|d| d == name) {
                            return false;
                        }
                    }
                    if let Some(ref allowed) = c.tools {
                        if allowed.iter().any(|a| a == "*") {
                            return true;
                        }
                        return allowed.iter().any(|a| a == name);
                    }
                    true
                }
            }
        })
        .map(|t| rclaude_api::types::ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            input_schema: serde_json::to_value(t.input_schema()).unwrap_or_default(),
        })
        .collect();

    let system_blocks = vec![rclaude_api::types::SystemBlock::Text {
        text: full_system,
        cache_control: None,
    }];

    let mut messages = if let Some(ref parent_msgs) = parent_messages {
        // Fork path: use parent messages for cache-sharing
        let parent_api_msgs: Vec<rclaude_api::types::ApiMessage> = parent_msgs
            .iter()
            .filter(|m| m.role != rclaude_core::message::Role::System)
            .map(|m| rclaude_api::types::ApiMessage {
                role: match m.role {
                    rclaude_core::message::Role::User => "user",
                    rclaude_core::message::Role::Assistant => "assistant",
                    rclaude_core::message::Role::System => "user",
                }
                .into(),
                content: m
                    .content
                    .iter()
                    .map(|b| match b {
                        rclaude_core::message::ContentBlock::Text { text } => {
                            rclaude_api::types::ApiContentBlock::Text { text: text.clone() }
                        }
                        rclaude_core::message::ContentBlock::ToolUse { id, name, input } => {
                            rclaude_api::types::ApiContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                            }
                        }
                        rclaude_core::message::ContentBlock::ToolResult {
                            tool_use_id,
                            is_error,
                            ..
                        } => rclaude_api::types::ApiContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: serde_json::Value::String("[see parent]".into()),
                            is_error: *is_error,
                        },
                        _ => rclaude_api::types::ApiContentBlock::Text {
                            text: String::new(),
                        },
                    })
                    .collect(),
            })
            .collect();
        let directive = crate::agent_fork::build_child_directive(prompt, None);
        crate::agent_fork::build_forked_messages(&parent_api_msgs, &directive)
    } else {
        vec![rclaude_api::types::ApiMessage {
            role: "user".into(),
            content: vec![rclaude_api::types::ApiContentBlock::Text {
                text: prompt.to_string(),
            }],
        }]
    };

    let max_turns = match agent {
        ActiveAgent::Custom(c) => c.max_turns.unwrap_or(15),
        ActiveAgent::BuiltIn(_) => 15,
    };

    let mut total_tool_uses = 0u32;
    let start = std::time::Instant::now();
    let mut final_text = String::new();

    let (_abort_tx, abort_rx) = tokio::sync::watch::channel(false);
    let tool_ctx = ToolUseContext {
        cwd: cwd.to_path_buf(),
        permission_mode: rclaude_core::permissions::PermissionMode::Default,
        debug: false,
        verbose,
        abort_signal: abort_rx,
        app_state: None,
    };

    for _turn in 0..max_turns {
        let request = rclaude_api::types::CreateMessageRequest {
            model: model.to_string(),
            max_tokens: 16384,
            messages: messages.clone(),
            system: Some(system_blocks.clone()),
            tools: if tool_defs.is_empty() {
                None
            } else {
                Some(tool_defs.clone())
            },
            stream: true,
            temperature: None,
            top_p: None,
            metadata: None,
        };

        let mut stream = client.create_message_stream(&request)?;

        let mut assistant_content = Vec::new();
        let mut tool_uses = Vec::new();
        let mut current_text = String::new();
        let mut current_tool_json = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_block_type = String::new();
        let mut has_tool_use = false;

        use rclaude_api::streaming::StreamContentEvent;
        while let Some(event) = stream.next_event().await {
            match event {
                StreamContentEvent::TextDelta { text } => {
                    if verbose {
                        eprint!("{text}");
                    }
                    current_text.push_str(&text);
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
                            final_text = current_text.clone();
                            assistant_content.push(rclaude_api::types::ApiContentBlock::Text {
                                text: std::mem::take(&mut current_text),
                            });
                        }
                        "tool_use" => {
                            has_tool_use = true;
                            total_tool_uses += 1;
                            let input: Value = serde_json::from_str(&current_tool_json)
                                .unwrap_or(Value::Object(Default::default()));
                            tool_uses.push((
                                std::mem::take(&mut current_tool_id),
                                std::mem::take(&mut current_tool_name),
                                input.clone(),
                            ));
                            assistant_content.push(rclaude_api::types::ApiContentBlock::ToolUse {
                                id: tool_uses.last().unwrap().0.clone(),
                                name: tool_uses.last().unwrap().1.clone(),
                                input,
                            });
                            current_tool_json.clear();
                        }
                        _ => {}
                    }
                    current_block_type.clear();
                }
                StreamContentEvent::Error { message } => {
                    return Ok(ToolResult::error(format!("Agent API error: {message}")));
                }
                _ => {}
            }
        }

        // Add assistant message
        messages.push(rclaude_api::types::ApiMessage {
            role: "assistant".into(),
            content: assistant_content,
        });

        if !has_tool_use {
            break;
        }

        // Execute tools
        let mut tool_result_blocks = Vec::new();
        for (id, name, input) in &tool_uses {
            let result =
                rclaude_core::query::execute_tool(name, input, &all_tools, &tool_ctx, verbose)
                    .await;
            let text = rclaude_core::query::extract_result_text(&result);
            // Truncate large results
            let text = if text.len() > 50_000 {
                format!("{}...\n[Truncated: {} chars]", &text[..50_000], text.len())
            } else {
                text
            };
            tool_result_blocks.push(rclaude_api::types::ApiContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: Value::String(text),
                is_error: result.is_error,
            });
        }

        messages.push(rclaude_api::types::ApiMessage {
            role: "user".into(),
            content: tool_result_blocks,
        });
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    // Build structured result
    let result = format!(
        "[Agent '{}' completed in {}ms, {} tool calls]\n\n{}",
        agent.name(),
        duration_ms,
        total_tool_uses,
        final_text
    );

    Ok(ToolResult::text(result))
}

/// Run agent as background subprocess.
async fn run_background_agent(
    agent: &ActiveAgent,
    prompt: &str,
    description: &str,
    model: &str,
    ctx: &ToolUseContext,
) -> Result<ToolResult> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("rclaude"));
    let escaped_prompt = prompt.replace('\'', "'\\''");
    let escaped_system = agent.system_prompt().replace('\'', "'\\''");

    let command = format!(
        "{} --print --model={} --system-prompt='{}' '{}'",
        exe.display(),
        model,
        escaped_system,
        escaped_prompt
    );

    let output_dir = rclaude_core::config::Config::config_dir().join("tasks");
    let (state, _handle) =
        task::spawn_shell_task(&command, description, &ctx.cwd, &output_dir).await?;

    Ok(ToolResult::text(format!(
        "Agent '{}' launched in background.\ntaskId: {}\ndescription: {description}\nmodel: {model}",
        agent.name(),
        state.id,
    )))
}
