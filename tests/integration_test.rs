//! Integration tests for rclaude.
//!
//! Covers: tools, commands, config, permissions, messages, context window,
//! session, history, hooks, model resolution, error handling, tool validation,
//! compact, agent loading, and CLI compatibility.

use rclaude_core::config::Config;
use rclaude_core::message::{ContentBlock, Message, Role};
use rclaude_core::state::AppState;
use rclaude_core::tool::Tool as _;

// ─── Tools ───

#[test]
fn test_all_tools_have_unique_names() {
    let tools = rclaude_tools::get_all_tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "Duplicate tool names found");
}

#[test]
fn test_all_tools_have_descriptions() {
    for tool in &rclaude_tools::get_all_tools() {
        assert!(
            !tool.description().is_empty(),
            "Tool '{}' has empty description",
            tool.name()
        );
    }
}

#[test]
fn test_all_tools_have_valid_schemas() {
    for tool in &rclaude_tools::get_all_tools() {
        let schema = tool.input_schema();
        assert_eq!(
            schema.schema_type,
            "object",
            "Tool '{}' schema not object",
            tool.name()
        );
    }
}

#[test]
fn test_tool_count() {
    let tools = rclaude_tools::get_all_tools();
    assert!(
        tools.len() >= 45,
        "Expected >=45 tools, got {}",
        tools.len()
    );
}

// ─── Commands ───

#[test]
fn test_all_commands_have_unique_names() {
    let commands = rclaude_commands::get_all_commands();
    let mut names: Vec<&str> = commands.iter().map(|c| c.name()).collect();
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "Duplicate command names found");
}

#[test]
fn test_all_commands_have_descriptions() {
    for cmd in &rclaude_commands::get_all_commands() {
        assert!(
            !cmd.description().is_empty(),
            "Command '{}' has empty description",
            cmd.name()
        );
    }
}

#[test]
fn test_command_count() {
    let commands = rclaude_commands::get_all_commands();
    assert!(
        commands.len() >= 59,
        "Expected >=59 commands, got {}",
        commands.len()
    );
}

// ─── State ───

#[test]
fn test_app_state_creation() {
    let state = AppState::new(std::path::PathBuf::from("/tmp"), Config::default());
    assert_eq!(state.cwd, std::path::PathBuf::from("/tmp"));
    assert!(state.messages.is_empty());
    assert!(!state.session_id.is_empty());
}

#[test]
fn test_usage_tracking() {
    let mut state = AppState::new(std::path::PathBuf::from("/tmp"), Config::default());
    state.record_usage("claude-sonnet-4-20250514", 1000, 500, 0, 0);
    state.record_usage("claude-sonnet-4-20250514", 2000, 1000, 100, 500);
    let usage = state.model_usage.get("claude-sonnet-4-20250514").unwrap();
    assert_eq!(usage.input_tokens, 3000);
    assert_eq!(usage.output_tokens, 1500);
    assert!(state.total_cost_usd > 0.0);
}

// ─── Messages ───

#[test]
fn test_message_roundtrip_serialization() {
    let msg = Message::user("test message");
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.role, Role::User);
    assert_eq!(deserialized.text_content(), "test message");
}

#[test]
fn test_conversation_flow() {
    let mut state = AppState::new(std::path::PathBuf::from("/tmp"), Config::default());
    state.messages.push(Message::user("Hello"));
    state
        .messages
        .push(Message::assistant(vec![ContentBlock::Text {
            text: "Hi!".into(),
        }]));
    state.messages.push(Message::user("How are you?"));
    state
        .messages
        .push(Message::assistant(vec![ContentBlock::Text {
            text: "Good!".into(),
        }]));
    assert_eq!(state.messages.len(), 4);
    assert_eq!(state.messages[1].text_content(), "Hi!");
}

// ─── Context Window ───

#[test]
fn test_context_window_estimation() {
    let msgs: Vec<Message> = (0..100)
        .map(|i| Message::user(format!("Message {i} with content")))
        .collect();
    let tokens = rclaude_core::context_window::estimate_conversation_tokens(&msgs);
    assert!(tokens > 0 && tokens < 10000);
}

#[test]
fn test_compact_preserves_recent() {
    let msgs: Vec<Message> = (0..20)
        .map(|i| Message::user("x".repeat(50000) + &format!(" msg{i}")))
        .collect();
    let compacted = rclaude_core::context_window::compact_messages(&msgs, 4);
    assert!(compacted.len() < msgs.len());
    assert!(compacted.last().unwrap().text_content().contains("msg19"));
}

#[test]
fn test_micro_compact() {
    let mut msgs: Vec<Message> = Vec::new();
    for i in 0..20 {
        msgs.push(Message::user(format!("q{i}")));
        msgs.push(Message {
            uuid: uuid::Uuid::new_v4(),
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: format!("t{i}"),
                content: serde_json::Value::String("x".repeat(2000)),
                is_error: false,
            }],
            timestamp: chrono::Utc::now(),
            model: None,
        });
    }
    let count = rclaude_core::micro_compact::micro_compact(&mut msgs, 4);
    assert!(count > 0, "Should have compacted some results");
}

#[test]
fn test_auto_compact_circuit_breaker() {
    let mut state = rclaude_core::auto_compact::AutoCompactState::new();
    assert!(state.is_enabled());
    state.record_failure();
    state.record_failure();
    state.record_failure();
    assert!(!state.is_enabled());
    state.reset();
    assert!(state.is_enabled());
}

// ─── Permissions ───

#[test]
fn test_permission_modes() {
    use rclaude_core::permissions::*;

    // Read tools always allowed
    for mode in [
        PermissionMode::Default,
        PermissionMode::Auto,
        PermissionMode::Plan,
        PermissionMode::BypassPermissions,
    ] {
        assert!(matches!(
            check_permission("Read", mode),
            PermissionResult::Allowed
        ));
        assert!(matches!(
            check_permission("Glob", mode),
            PermissionResult::Allowed
        ));
    }

    // Auto mode: safe tools allowed, risky tools need approval
    assert!(matches!(
        check_permission("Agent", PermissionMode::Auto),
        PermissionResult::Allowed
    ));
    assert!(matches!(
        check_permission("Read", PermissionMode::Auto),
        PermissionResult::Allowed
    ));
    // Bash is not in safe list — needs approval in auto mode
    assert!(matches!(
        check_permission("Bash", PermissionMode::Auto),
        PermissionResult::NeedApproval { .. }
    ));

    // Plan mode denies writes
    assert!(matches!(
        check_permission("Bash", PermissionMode::Plan),
        PermissionResult::Denied(_)
    ));

    // Bypass allows everything
    assert!(matches!(
        check_permission("Bash", PermissionMode::BypassPermissions),
        PermissionResult::Allowed
    ));

    // Default asks for writes
    assert!(matches!(
        check_permission("Bash", PermissionMode::Default),
        PermissionResult::NeedApproval { .. }
    ));
}

// ─── Model Resolution ───

#[test]
fn test_model_resolve_aliases() {
    // Without env vars, aliases resolve to canonical names
    let resolved = rclaude_core::model::resolve_model("sonnet");
    assert!(
        resolved.contains("sonnet"),
        "sonnet should resolve to a sonnet model: {resolved}"
    );
}

#[test]
fn test_model_strip_1m_suffix() {
    let resolved = rclaude_core::model::resolve_model("opus[1m]");
    assert!(
        !resolved.contains("[1m]"),
        "Should strip [1m] suffix: {resolved}"
    );
}

#[test]
fn test_model_context_window() {
    let window = rclaude_core::model::context_window_for_model("claude-sonnet-4-20250514");
    assert!(
        window >= 100_000,
        "Sonnet should have >=100K context: {window}"
    );
}

// ─── Error Handling ───

#[test]
fn test_error_from_api_prompt_too_long() {
    let e = rclaude_core::error::RclaudeError::from_api_error(
        400,
        "prompt is too long: 300000 tokens > 200000",
    );
    assert!(matches!(
        e,
        rclaude_core::error::RclaudeError::PromptTooLong { .. }
    ));
}

#[test]
fn test_error_parse_token_counts() {
    let counts = rclaude_core::error::RclaudeError::parse_prompt_too_long("250000 tokens > 200000");
    assert_eq!(counts, Some((250000, 200000)));
}

#[test]
fn test_error_user_messages() {
    let e = rclaude_core::error::RclaudeError::Api {
        message: "x".into(),
        status: Some(429),
    };
    assert!(e.user_message().contains("Rate limited"));
    let e = rclaude_core::error::RclaudeError::Api {
        message: "x".into(),
        status: Some(529),
    };
    assert!(e.user_message().contains("overloaded"));
}

#[test]
fn test_error_is_retryable() {
    assert!(rclaude_core::error::RclaudeError::Api {
        message: "".into(),
        status: Some(429)
    }
    .is_retryable());
    assert!(rclaude_core::error::RclaudeError::Api {
        message: "".into(),
        status: Some(529)
    }
    .is_retryable());
    assert!(!rclaude_core::error::RclaudeError::Api {
        message: "".into(),
        status: Some(401)
    }
    .is_retryable());
}

// ─── Tool Result Budget ───

#[test]
fn test_tool_result_budget_small_passthrough() {
    let mut results = vec![rclaude_core::streaming_executor::OrderedToolResult {
        tool_use_id: "1".into(),
        tool_name: "Bash".into(),
        result_text: "short".into(),
        is_error: false,
        duration_ms: 10,
    }];
    rclaude_core::query::enforce_tool_result_budget(&mut results);
    assert_eq!(results[0].result_text, "short");
}

#[test]
fn test_tool_result_budget_large_truncated() {
    let mut results = vec![rclaude_core::streaming_executor::OrderedToolResult {
        tool_use_id: "1".into(),
        tool_name: "Bash".into(),
        result_text: "x".repeat(100_000),
        is_error: false,
        duration_ms: 10,
    }];
    rclaude_core::query::enforce_tool_result_budget(&mut results);
    assert!(results[0].result_text.len() < 60_000);
    assert!(results[0].result_text.contains("Truncated"));
}

// ─── Query Engine ───

#[test]
fn test_max_tokens_stop_detection() {
    assert!(rclaude_core::query::is_max_tokens_stop(Some("max_tokens")));
    assert!(!rclaude_core::query::is_max_tokens_stop(Some("end_turn")));
    assert!(!rclaude_core::query::is_max_tokens_stop(None));
}

#[test]
fn test_fallback_model() {
    assert!(rclaude_core::query::get_fallback_model("claude-opus-4-20250514").is_some());
    assert!(rclaude_core::query::get_fallback_model("claude-haiku-3-5-20241022").is_none());
}

#[test]
fn test_continue_message() {
    let msg = rclaude_core::query::build_continue_message();
    assert_eq!(msg.role, Role::User);
    assert!(msg.text_content().contains("Resume"));
}

// ─── Tool Input Validation ───

#[test]
fn test_validate_require_string() {
    let input = serde_json::json!({"name": "hello"});
    assert_eq!(
        rclaude_core::tool_input_validation::require_string(&input, "name").unwrap(),
        "hello"
    );
    assert!(rclaude_core::tool_input_validation::require_string(&input, "missing").is_err());
    let empty = serde_json::json!({"name": ""});
    assert!(rclaude_core::tool_input_validation::require_string(&empty, "name").is_err());
}

#[test]
fn test_validate_file_path() {
    let input = serde_json::json!({"path": "src/main.rs"});
    let result = rclaude_core::tool_input_validation::validate_file_path(
        &input,
        "path",
        std::path::Path::new("/home/user/project"),
    )
    .unwrap();
    assert_eq!(
        result,
        std::path::PathBuf::from("/home/user/project/src/main.rs")
    );

    let blocked = serde_json::json!({"path": "/dev/zero"});
    assert!(rclaude_core::tool_input_validation::validate_file_path(
        &blocked,
        "path",
        std::path::Path::new("/tmp"),
    )
    .is_err());
}

// ─── Hooks ───

#[test]
fn test_hook_registry() {
    use rclaude_core::hooks::*;
    let mut reg = HookRegistry::new();
    reg.register(
        HookEvent::PreToolUse,
        HookMatcher {
            action: HookAction::Command {
                command: "echo pre".into(),
                shell: None,
            },
            tool_name: Some("Bash".into()),
            timeout: 5000,
            condition: None,
            once: false,
            status_message: None,
        },
    );
    assert_eq!(reg.get(HookEvent::PreToolUse).len(), 1);
    assert_eq!(reg.get(HookEvent::PostToolUse).len(), 0);
}

// ─── Config Compatibility ───

#[test]
fn test_config_dir_default() {
    let dir = Config::config_dir();
    assert!(dir.to_string_lossy().contains(".claude"));
}

#[test]
fn test_config_default_values() {
    let cfg = Config::default();
    assert!(cfg.api_key.is_none());
    assert!(cfg.model.contains("sonnet"));
    assert_eq!(cfg.max_tokens, 16384);
}

#[test]
fn test_config_deserialize_with_env() {
    let json = r#"{"model":"opus","env":{"ANTHROPIC_BASE_URL":"http://custom"}}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.model, "opus");
    assert_eq!(cfg.env.get("ANTHROPIC_BASE_URL").unwrap(), "http://custom");
}

// ─── Retry ───

#[test]
fn test_retry_state_429() {
    let mut state = rclaude_api::retry::RetryState::new(3);
    let (status, delay) = state.handle_error(429, None, "rate limited");
    assert!(matches!(
        status,
        Some(rclaude_api::retry::RetryStatus::Retrying { .. })
    ));
    assert!(delay > std::time::Duration::ZERO);
}

#[test]
fn test_retry_state_529_max() {
    let mut state = rclaude_api::retry::RetryState::new(10);
    for _ in 0..3 {
        state.handle_error(529, None, "overloaded");
    }
    let (status, _) = state.handle_error(529, None, "overloaded");
    assert!(matches!(
        status,
        Some(rclaude_api::retry::RetryStatus::GaveUp { .. })
    ));
}

#[test]
fn test_retry_auth_error_no_retry() {
    let mut state = rclaude_api::retry::RetryState::new(3);
    let (status, _) = state.handle_error(401, None, "unauthorized");
    assert!(matches!(
        status,
        Some(rclaude_api::retry::RetryStatus::Fatal { .. })
    ));
}

// ─── Conversation Recovery ───

#[test]
fn test_find_incomplete_tool_calls() {
    let messages = vec![Message::assistant(vec![ContentBlock::ToolUse {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    }])];
    let orphans = rclaude_services::conversation_recovery::find_incomplete_tool_calls(&messages);
    assert_eq!(orphans.len(), 1);
}

#[test]
fn test_fix_incomplete_tool_calls() {
    let mut messages = vec![Message::assistant(vec![ContentBlock::ToolUse {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    }])];
    rclaude_services::conversation_recovery::fix_incomplete_tool_calls(&mut messages);
    assert_eq!(messages.len(), 2);
}

// ─── Agent Loading ───

#[test]
fn test_agent_builtins() {
    let agents = rclaude_tools::agent_loader::get_active_agents(&[]);
    assert!(agents.iter().any(|a| a.name() == "general-purpose"));
    assert!(agents.iter().any(|a| a.name() == "Explore"));
    assert!(agents.iter().any(|a| a.name() == "Plan"));
    assert!(agents.iter().any(|a| a.name() == "Verification"));
}

#[test]
fn test_agent_tool_filtering() {
    use rclaude_tools::agent_types::*;
    assert!(is_tool_allowed(&EXPLORE_AGENT, "Read"));
    assert!(is_tool_allowed(&EXPLORE_AGENT, "Grep"));
    assert!(!is_tool_allowed(&EXPLORE_AGENT, "Write"));
    assert!(!is_tool_allowed(&EXPLORE_AGENT, "Agent"));
}

// ─── Path Normalization ───

#[test]
fn test_path_normalization() {
    let p = rclaude_utils::path::resolve_file_path(
        "../etc/passwd",
        std::path::Path::new("/home/user/project"),
    );
    assert_eq!(p, std::path::PathBuf::from("/home/user/etc/passwd"));
    let p =
        rclaude_utils::path::resolve_file_path("/absolute/path", std::path::Path::new("/ignored"));
    assert_eq!(p, std::path::PathBuf::from("/absolute/path"));
}

// ─── MCP Config ───

#[test]
fn test_mcp_env_expansion() {
    std::env::set_var("TEST_RCLAUDE_MCP_VAR", "expanded");
    let result = rclaude_mcp::config::expand_env_vars("${TEST_RCLAUDE_MCP_VAR}");
    assert_eq!(result, "expanded");
    std::env::remove_var("TEST_RCLAUDE_MCP_VAR");
}

// ─── Session ───

#[test]
fn test_session_title_generation() {
    let messages = vec![Message::user("fix the login bug in auth.rs")];
    let title = rclaude_services::session::generate_session_title(&messages);
    assert!(title.is_some());
    assert!(title.unwrap().contains("fix the login"));
}

// ─── Denial Tracking ───

#[test]
fn test_denial_tracking_limits() {
    use rclaude_core::denial_tracking::*;
    let mut state = DenialTrackingState::new();
    assert!(!state.should_fallback_to_prompting());
    state.record_denial();
    state.record_denial();
    state.record_denial();
    assert!(state.should_fallback_to_prompting());
}

#[test]
fn test_denial_tracking_reset_on_success() {
    use rclaude_core::denial_tracking::*;
    let mut state = DenialTrackingState::new();
    state.record_denial();
    state.record_denial();
    state.record_success();
    assert!(!state.should_fallback_to_prompting());
}

// ─── Config Env Override ───

#[test]
fn test_config_deserialize_preserves_api_key() {
    let json = r#"{"apiKey":"sk-test-key"}"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key.as_deref(), Some("sk-test-key"));
}

// ─── Streaming Executor ───

#[test]
fn test_streaming_executor_partition() {
    let uses: Vec<rclaude_core::streaming_executor::ToolCall> = vec![];
    let tools: Vec<Box<dyn rclaude_core::tool::Tool>> = vec![];
    let (safe, seq) = rclaude_core::streaming_executor::partition_tool_calls(&uses, &tools);
    assert!(safe.is_empty());
    assert!(seq.is_empty());
}

// ─── Compact Recovery ───

#[test]
fn test_compact_build_with_recovery() {
    let msgs: Vec<Message> = (0..10)
        .map(|i| Message::user(format!("Message {i}")))
        .collect();
    let compacted = rclaude_services::compact::build_compacted_messages(&msgs, "summary text", 3);
    assert!(compacted
        .iter()
        .any(|m| m.text_content().contains("summary")));
    assert!(compacted
        .last()
        .unwrap()
        .text_content()
        .contains("Message 9"));
}

// ─── Config camelCase Compatibility ───

#[test]
fn test_config_camelcase_all_fields() {
    let json = r#"{
        "apiKey": "sk-test",
        "maxTokens": 8192,
        "allowedTools": ["Read", "Glob"],
        "deniedTools": ["Bash"],
        "env": {"FOO": "bar"},
        "permissionMode": "auto",
        "outputStyle": "explanatory",
        "systemPrompt": "be concise",
        "maxTurns": 10
    }"#;
    let cfg: Config = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.api_key.as_deref(), Some("sk-test"));
    assert_eq!(cfg.max_tokens, 8192);
    assert_eq!(cfg.allowed_tools, vec!["Read", "Glob"]);
    assert_eq!(cfg.denied_tools, vec!["Bash"]);
    assert_eq!(cfg.env.get("FOO").unwrap(), "bar");
    assert_eq!(cfg.permission_mode.as_deref(), Some("auto"));
    assert_eq!(cfg.output_style.as_deref(), Some("explanatory"));
    assert_eq!(cfg.system_prompt.as_deref(), Some("be concise"));
    assert_eq!(cfg.max_turns, Some(10));
}

// ─── Auth Multi-Source ───

#[test]
fn test_auth_env_var_source() {
    // When ANTHROPIC_API_KEY is set, it should be the source
    // (Can't fully test without controlling env, but verify the function exists)
    let result = rclaude_core::auth::get_api_key_with_source();
    // In test env, key may or may not be set — just verify no panic
    let _ = result.source;
}

#[test]
fn test_auth_validate_key_format() {
    assert!(rclaude_core::auth::validate_api_key(
        "sk-ant-api03-abcdefghijklmnopqrstuvwxyz"
    ));
    assert!(!rclaude_core::auth::validate_api_key("invalid"));
    assert!(!rclaude_core::auth::validate_api_key(""));
}

// ─── Session Save/Load Roundtrip ───

#[tokio::test]
async fn test_session_save_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path();
    let session_id = "test-session-001";
    let messages = vec![
        Message::user("hello"),
        Message::assistant(vec![ContentBlock::Text { text: "hi".into() }]),
    ];
    let path = rclaude_services::session::save_session(session_id, "sonnet", &messages, cwd)
        .await
        .unwrap();
    assert!(path.exists());

    let loaded = rclaude_services::session::load_session(session_id, cwd)
        .await
        .unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.session_id, session_id);
}

#[tokio::test]
async fn test_session_list() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path();
    rclaude_services::session::save_session("s1", "sonnet", &[Message::user("a")], cwd)
        .await
        .unwrap();
    rclaude_services::session::save_session("s2", "opus", &[Message::user("b")], cwd)
        .await
        .unwrap();
    let list = rclaude_services::session::list_sessions(cwd).await.unwrap();
    assert_eq!(list.len(), 2);
}

// ─── History JSONL ───

#[test]
fn test_history_push_dedup() {
    let mut h = rclaude_services::history::InputHistory::load();
    h.push("unique_test_aaa_xyz".into());
    let after_first = h.len();
    h.push("unique_test_aaa_xyz".into()); // duplicate
    assert_eq!(
        h.len(),
        after_first,
        "Consecutive duplicates should be deduped"
    );
    h.push("unique_test_bbb_xyz".into()); // different
    assert_eq!(h.len(), after_first + 1, "Non-duplicate should be added");
}

// ─── Hooks Condition + Once ───

#[test]
fn test_hook_condition_matching() {
    use rclaude_core::hooks::*;
    let mut reg = HookRegistry::new();
    reg.register(
        HookEvent::PreToolUse,
        HookMatcher {
            action: HookAction::Command {
                command: "echo matched".into(),
                shell: None,
            },
            tool_name: None,
            timeout: 5000,
            condition: Some("Bash(npm *)".into()),
            once: false,
            status_message: None,
        },
    );
    // Condition is checked at runtime, not at registration
    assert_eq!(reg.get(HookEvent::PreToolUse).len(), 1);
}

#[tokio::test]
async fn test_hook_once_flag() {
    use rclaude_core::hooks::*;
    let mut reg = HookRegistry::new();
    reg.register(
        HookEvent::Stop,
        HookMatcher {
            action: HookAction::Command {
                command: "echo once".into(),
                shell: None,
            },
            tool_name: None,
            timeout: 5000,
            condition: None,
            once: true,
            status_message: None,
        },
    );
    let r1 = reg
        .run(
            HookEvent::Stop,
            std::path::Path::new("/tmp"),
            &std::collections::HashMap::new(),
        )
        .await;
    assert_eq!(r1[0].outcome, HookOutcome::Success);
    let r2 = reg
        .run(
            HookEvent::Stop,
            std::path::Path::new("/tmp"),
            &std::collections::HashMap::new(),
        )
        .await;
    assert_eq!(r2[0].outcome, HookOutcome::Skipped);
}

// ─── Agent Memory Paths ───

#[test]
fn test_agent_memory_dir() {
    use rclaude_tools::agent_loader::*;
    let dir = get_agent_memory_dir(
        &AgentMemoryScope::Project,
        std::path::Path::new("/tmp/proj"),
    );
    assert_eq!(
        dir,
        std::path::PathBuf::from("/tmp/proj/.claude/agent-memory")
    );

    let dir = get_agent_memory_dir(&AgentMemoryScope::Local, std::path::Path::new("/tmp/proj"));
    assert_eq!(
        dir,
        std::path::PathBuf::from("/tmp/proj/.claude/agent-memory-local")
    );
}

#[test]
fn test_agent_memory_entrypoint() {
    use rclaude_tools::agent_loader::*;
    let path = get_agent_memory_entrypoint(
        &AgentMemoryScope::Project,
        "Explore",
        std::path::Path::new("/tmp"),
    );
    assert!(path.to_string_lossy().contains("Explore/MEMORY.md"));
}

// ─── Skill Loading ───

#[tokio::test]
async fn test_skill_loading() {
    // Load skills from current directory (may find project skills)
    let skills = rclaude_tools::skill::load_all_skills(std::path::Path::new(".")).await;
    // Bundled skills should always be present
    assert!(
        skills
            .iter()
            .any(|s| s.name == "commit" || s.name == "review" || s.name == "debug"),
        "Should have at least one bundled skill, got: {:?}",
        skills.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

// ─── Tool Actual Execution ───

#[tokio::test]
async fn test_glob_tool_execution() {
    let tools = rclaude_tools::get_all_tools();
    let glob_tool = tools.iter().find(|t| t.name() == "Glob").unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let ctx = rclaude_core::tool::ToolUseContext {
        cwd: std::path::PathBuf::from("."),
        permission_mode: rclaude_core::permissions::PermissionMode::Auto,
        debug: false,
        verbose: false,
        abort_signal: rx,
        app_state: None,
    };
    let result = glob_tool
        .execute(serde_json::json!({"pattern": "Cargo.toml"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    let text = result
        .content
        .iter()
        .filter_map(|c| match c {
            rclaude_core::tool::ToolResultContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert!(text.contains("Cargo.toml"));
}

#[tokio::test]
async fn test_read_tool_execution() {
    let tools = rclaude_tools::get_all_tools();
    let read_tool = tools.iter().find(|t| t.name() == "Read").unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let ctx = rclaude_core::tool::ToolUseContext {
        cwd: std::path::PathBuf::from("."),
        permission_mode: rclaude_core::permissions::PermissionMode::Auto,
        debug: false,
        verbose: false,
        abort_signal: rx,
        app_state: None,
    };
    let result = read_tool
        .execute(serde_json::json!({"file_path": "Cargo.toml"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
}

#[tokio::test]
async fn test_write_edit_tool_execution() {
    let tools = rclaude_tools::get_all_tools();
    let write_tool = tools.iter().find(|t| t.name() == "Write").unwrap();
    let edit_tool = tools.iter().find(|t| t.name() == "Edit").unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let dir = tempfile::tempdir().unwrap();
    let ctx = rclaude_core::tool::ToolUseContext {
        cwd: dir.path().to_path_buf(),
        permission_mode: rclaude_core::permissions::PermissionMode::Auto,
        debug: false,
        verbose: false,
        abort_signal: rx,
        app_state: None,
    };
    let file_path = dir.path().join("test.txt");

    // Write
    let r = write_tool
        .execute(
            serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "hello world"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!r.is_error);
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");

    // Edit
    let r = edit_tool
        .execute(
            serde_json::json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "hello",
                "new_string": "goodbye"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!r.is_error);
    assert_eq!(
        std::fs::read_to_string(&file_path).unwrap(),
        "goodbye world"
    );
}

#[tokio::test]
async fn test_grep_tool_execution() {
    let tools = rclaude_tools::get_all_tools();
    let grep_tool = tools.iter().find(|t| t.name() == "Grep").unwrap();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let ctx = rclaude_core::tool::ToolUseContext {
        cwd: std::path::PathBuf::from("."),
        permission_mode: rclaude_core::permissions::PermissionMode::Auto,
        debug: false,
        verbose: false,
        abort_signal: rx,
        app_state: None,
    };
    let result = grep_tool
        .execute(
            serde_json::json!({
                "pattern": "fn main",
                "path": "src/"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
}

// ─── Bash Tool Security ───

#[test]
fn test_bash_read_only_detection() {
    assert!(rclaude_tools::bash::is_read_only_command("ls -la"));
    assert!(rclaude_tools::bash::is_read_only_command("git status"));
    assert!(rclaude_tools::bash::is_read_only_command("cat file.txt"));
    assert!(!rclaude_tools::bash::is_read_only_command("rm -rf /"));
    assert!(!rclaude_tools::bash::is_read_only_command("npm install"));
}

// ─── Session Storage JSONL ───

#[tokio::test]
async fn test_session_storage_jsonl_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path();
    let messages = vec![
        Message::user("hello"),
        Message::assistant(vec![ContentBlock::Text { text: "hi".into() }]),
    ];
    rclaude_services::session_storage::save_transcript_jsonl("test-jsonl", &messages, cwd)
        .await
        .unwrap();
    let loaded = rclaude_services::session_storage::load_transcript_jsonl("test-jsonl", cwd)
        .await
        .unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].role, Role::User);
}
