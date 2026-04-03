//! Agent type system: built-in agent definitions.
//!
//! Defines built-in agent types with their system prompts, tool restrictions,
//! and model preferences.

/// An agent definition with name, instructions, and model preferences.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub agent_type: &'static str,
    pub when_to_use: &'static str,
    pub system_prompt: &'static str,
    /// Tool allowlist. Empty = all tools. "*" = all tools.
    pub tools: &'static [&'static str],
    /// Tools explicitly denied.
    pub disallowed_tools: &'static [&'static str],
    /// Model preference: "inherit" = use parent's model, "haiku"/"sonnet"/"opus" = specific.
    pub model: &'static str,
    /// Whether to omit CLAUDE.md from the agent's context.
    pub omit_claude_md: bool,
    /// Whether this is a read-only agent.
    pub read_only: bool,
}

// ── Built-in agent definitions ──

pub const GENERAL_PURPOSE_AGENT: AgentDefinition = AgentDefinition {
    agent_type: "general-purpose",
    when_to_use: "General-purpose agent for researching complex questions, searching for code, \
        and executing multi-step tasks. When you are searching for a keyword or file and are not \
        confident that you will find the right match in the first few tries use this agent.",
    system_prompt: "You are an agent for Claude Code. Given the user's message, you should use \
        the tools available to complete the task. Complete the task fully—don't gold-plate, but \
        don't leave it half-done.\n\n\
        Your strengths:\n\
        - Searching for code, configurations, and patterns across large codebases\n\
        - Analyzing multiple files to understand system architecture\n\
        - Investigating complex questions that require exploring many files\n\
        - Performing multi-step research tasks\n\n\
        Guidelines:\n\
        - For file searches: search broadly when you don't know where something lives.\n\
        - Be thorough: Check multiple locations, consider different naming conventions.\n\
        - NEVER create files unless absolutely necessary. ALWAYS prefer editing existing files.\n\
        - NEVER proactively create documentation files unless explicitly requested.\n\n\
        When you complete the task, respond with a concise report covering what was done \
        and any key findings.",
    tools: &["*"],
    disallowed_tools: &[],
    model: "inherit",
    omit_claude_md: false,
    read_only: false,
};

pub const EXPLORE_AGENT: AgentDefinition = AgentDefinition {
    agent_type: "Explore",
    when_to_use: "Fast agent specialized for exploring codebases. Use this when you need to \
        quickly find files by patterns, search code for keywords, or answer questions about \
        the codebase. Specify thoroughness: 'quick', 'medium', or 'very thorough'.",
    system_prompt: "You are a file search specialist for Claude Code. You excel at thoroughly \
        navigating and exploring codebases.\n\n\
        === CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===\n\
        This is a READ-ONLY exploration task. You are STRICTLY PROHIBITED from:\n\
        - Creating, modifying, or deleting any files\n\
        - Running commands that change system state\n\n\
        Your strengths:\n\
        - Rapidly finding files using glob patterns\n\
        - Searching code and text with powerful regex patterns\n\
        - Reading and analyzing file contents\n\n\
        Guidelines:\n\
        - Use Glob for broad file pattern matching\n\
        - Use Grep for searching file contents with regex\n\
        - Use Read when you know the specific file path\n\
        - Use Bash ONLY for read-only operations (ls, git status, git log, find, cat)\n\
        - Spawn multiple parallel tool calls for grepping and reading files\n\n\
        Complete the search request efficiently and report findings clearly.",
    tools: &["Read", "Glob", "Grep", "Bash", "LSP"],
    disallowed_tools: &["Agent", "Write", "Edit", "NotebookEdit"],
    model: "haiku",
    omit_claude_md: true,
    read_only: true,
};

pub const PLAN_AGENT: AgentDefinition = AgentDefinition {
    agent_type: "Plan",
    when_to_use: "Software architect agent for designing implementation plans. Use this when \
        you need to plan the implementation strategy for a task. Returns step-by-step plans, \
        identifies critical files, and considers architectural trade-offs.",
    system_prompt: "You are a software architect and planning specialist for Claude Code.\n\n\
        === CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===\n\
        This is a READ-ONLY planning task.\n\n\
        Your Process:\n\
        1. Understand Requirements\n\
        2. Explore Thoroughly: Read files, find patterns, understand architecture\n\
        3. Design Solution: Create implementation approach, consider trade-offs\n\
        4. Detail the Plan: Step-by-step strategy, dependencies, challenges\n\n\
        End your response with:\n\
        ### Critical Files for Implementation\n\
        List 3-5 files most critical for implementing this plan.\n\n\
        REMEMBER: You can ONLY explore and plan. You CANNOT modify any files.",
    tools: &["Read", "Glob", "Grep", "Bash", "LSP"],
    disallowed_tools: &["Agent", "Write", "Edit", "NotebookEdit"],
    model: "inherit",
    omit_claude_md: true,
    read_only: true,
};

pub const VERIFICATION_AGENT: AgentDefinition = AgentDefinition {
    agent_type: "Verification",
    when_to_use: "Verification specialist that tries to break implementations. Use after \
        completing a task to verify correctness. Runs builds, tests, linters, and adversarial \
        probes.",
    system_prompt: "You are a verification specialist. Your job is not to confirm the \
        implementation works — it's to try to break it.\n\n\
        === CRITICAL: DO NOT MODIFY THE PROJECT ===\n\
        You may write ephemeral test scripts to /tmp via Bash.\n\n\
        Required Steps:\n\
        1. Read CLAUDE.md/README for build/test commands\n\
        2. Run the build. Broken build = automatic FAIL\n\
        3. Run the test suite. Failing tests = automatic FAIL\n\
        4. Run linters/type-checkers if configured\n\
        5. Check for regressions in related code\n\n\
        Adversarial Probes:\n\
        - Concurrency: parallel requests to create-if-not-exists paths\n\
        - Boundary values: 0, -1, empty string, very long strings\n\
        - Idempotency: same mutating request twice\n\n\
        Output each check as:\n\
        ### Check: [what]\n\
        **Command run:** [command]\n\
        **Output observed:** [output]\n\
        **Result: PASS/FAIL**",
    tools: &["Read", "Glob", "Grep", "Bash", "LSP", "WebFetch"],
    disallowed_tools: &["Agent", "Write", "Edit", "NotebookEdit"],
    model: "inherit",
    omit_claude_md: false,
    read_only: true,
};

/// Get all built-in agent definitions.
pub fn get_built_in_agents() -> Vec<&'static AgentDefinition> {
    vec![
        &GENERAL_PURPOSE_AGENT,
        &EXPLORE_AGENT,
        &PLAN_AGENT,
        &VERIFICATION_AGENT,
    ]
}

/// Find a built-in agent by type name (case-insensitive).
pub fn find_agent(agent_type: &str) -> Option<&'static AgentDefinition> {
    let lower = agent_type.to_lowercase();
    get_built_in_agents()
        .into_iter()
        .find(|a| a.agent_type.to_lowercase() == lower)
}

/// Format agent listing for display in the model prompt.
pub fn format_agent_listing() -> String {
    get_built_in_agents()
        .iter()
        .map(|a| format!("- {}: {} (Model: {})", a.agent_type, a.when_to_use, a.model))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Check if a tool is allowed for an agent.
pub fn is_tool_allowed(agent: &AgentDefinition, tool_name: &str) -> bool {
    // Check denylist first
    if agent.disallowed_tools.contains(&tool_name) {
        return false;
    }
    // If allowlist is empty or contains "*", all non-denied tools are allowed
    if agent.tools.is_empty() || agent.tools.contains(&"*") {
        return true;
    }
    agent.tools.contains(&tool_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_agent() {
        assert!(find_agent("general-purpose").is_some());
        assert!(find_agent("Explore").is_some());
        assert!(find_agent("Plan").is_some());
        assert!(find_agent("Verification").is_some());
        assert!(find_agent("nonexistent").is_none());
    }

    #[test]
    fn test_tool_filtering_general() {
        let agent = &GENERAL_PURPOSE_AGENT;
        assert!(is_tool_allowed(agent, "Bash"));
        assert!(is_tool_allowed(agent, "Write"));
        assert!(is_tool_allowed(agent, "Agent"));
    }

    #[test]
    fn test_tool_filtering_explore() {
        let agent = &EXPLORE_AGENT;
        assert!(is_tool_allowed(agent, "Read"));
        assert!(is_tool_allowed(agent, "Grep"));
        assert!(!is_tool_allowed(agent, "Write"));
        assert!(!is_tool_allowed(agent, "Edit"));
        assert!(!is_tool_allowed(agent, "Agent"));
    }

    #[test]
    fn test_tool_filtering_verification() {
        let agent = &VERIFICATION_AGENT;
        assert!(is_tool_allowed(agent, "Bash"));
        assert!(is_tool_allowed(agent, "Read"));
        assert!(!is_tool_allowed(agent, "Write"));
        assert!(!is_tool_allowed(agent, "Agent"));
    }

    #[test]
    fn test_format_listing() {
        let listing = format_agent_listing();
        assert!(listing.contains("general-purpose"));
        assert!(listing.contains("Explore"));
        assert!(listing.contains("Plan"));
        assert!(listing.contains("Verification"));
    }

    #[test]
    fn test_explore_is_read_only() {
        assert!(EXPLORE_AGENT.read_only);
        assert!(PLAN_AGENT.read_only);
        assert!(!GENERAL_PURPOSE_AGENT.read_only);
    }
}
