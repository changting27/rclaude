//! SkillTool: discover and invoke skills (slash commands).
//!
//! Skills are markdown files in .claude/skills/ or ~/.claude/skills/ with
//! optional frontmatter (description, tools, model) and a prompt body.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// A loaded skill definition.
#[derive(Debug, Clone)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub prompt: String,
    pub allowed_tools: Vec<String>,
    pub model: Option<String>,
    pub source: SkillSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    Project, // .claude/skills/
    User,    // ~/.claude/skills/
    Bundled, // built-in
}

/// Load all skills from disk.
pub async fn load_all_skills(cwd: &Path) -> Vec<SkillDefinition> {
    let mut skills = Vec::new();

    // 1. User skills (~/.claude/skills/ and ~/.claude/commands/)
    if let Some(home) = dirs::home_dir() {
        for dir_name in ["skills", "commands"] {
            let user_dir = home.join(".claude").join(dir_name);
            if let Ok(entries) = load_skills_from_dir(&user_dir, SkillSource::User).await {
                skills.extend(entries);
            }
        }
    }

    // 2. Project skills (.claude/skills/ and .claude/commands/)
    for dir_name in ["skills", "commands"] {
        let project_dir = cwd.join(".claude").join(dir_name);
        if let Ok(entries) = load_skills_from_dir(&project_dir, SkillSource::Project).await {
            skills.extend(entries);
        }
    }

    // 3. Bundled skills
    skills.extend(get_bundled_skills());

    skills
}

/// Load skills from a directory of .md files.
async fn load_skills_from_dir(
    dir: &Path,
    source: SkillSource,
) -> std::io::Result<Vec<SkillDefinition>> {
    let mut skills = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Some(skill) = parse_skill_file(&path, &content, source.clone()) {
                skills.push(skill);
            }
        }
    }

    // Also check subdirectories for SKILL.md files
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if let Ok(content) = tokio::fs::read_to_string(&skill_md).await {
                if let Some(skill) = parse_skill_file(&skill_md, &content, source.clone()) {
                    skills.push(skill);
                }
            }
        }
    }

    Ok(skills)
}

/// Parse a skill markdown file with optional frontmatter.
/// Frontmatter format:
/// ```text
/// ---
/// description: Short description
/// when-to-use: When the model should use this
/// tools: Read, Grep, Bash
/// model: sonnet
/// ---
/// Prompt body here...
/// ```
fn parse_skill_file(path: &Path, content: &str, source: SkillSource) -> Option<SkillDefinition> {
    let name = path.file_stem()?.to_str()?.to_string();
    // Handle SKILL.md in subdirectory — use parent dir name
    let name = if name == "SKILL" {
        path.parent()?.file_name()?.to_str()?.to_string()
    } else {
        name
    };

    let (frontmatter, body) = parse_frontmatter(content);

    let description = frontmatter
        .get("description")
        .cloned()
        .unwrap_or_else(|| extract_first_line(&body));

    Some(SkillDefinition {
        name,
        description,
        when_to_use: frontmatter
            .get("when-to-use")
            .or(frontmatter.get("whenToUse"))
            .cloned(),
        prompt: body,
        allowed_tools: frontmatter
            .get("tools")
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        model: frontmatter.get("model").cloned(),
        source,
        path: path.to_path_buf(),
    })
}

/// Parse YAML-like frontmatter from markdown.
fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let mut fm = HashMap::new();
    if !content.starts_with("---") {
        return (fm, content.to_string());
    }

    let rest = &content[3..];
    if let Some(end) = rest.find("\n---") {
        let fm_text = &rest[..end];
        let body = rest[end + 4..].trim_start().to_string();

        for line in fm_text.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if !key.is_empty() && !value.is_empty() {
                    fm.insert(key, value);
                }
            }
        }
        (fm, body)
    } else {
        (fm, content.to_string())
    }
}

fn extract_first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .trim_start_matches('#')
        .trim()
        .to_string()
}

/// Built-in skills shipped with the CLI.
fn get_bundled_skills() -> Vec<SkillDefinition> {
    vec![
        SkillDefinition {
            name: "commit".into(),
            description: "Generate a git commit with a descriptive message".into(),
            when_to_use: Some("When the user wants to commit their changes".into()),
            prompt: "Review the staged changes (git diff --cached) and create a concise, \
                descriptive commit message following conventional commit format. \
                Then run git commit with that message."
                .into(),
            allowed_tools: vec!["Bash".into()],
            model: None,
            source: SkillSource::Bundled,
            path: PathBuf::from("bundled://commit"),
        },
        SkillDefinition {
            name: "review".into(),
            description: "Review code changes and provide feedback".into(),
            when_to_use: Some("When the user wants a code review of their changes".into()),
            prompt: "Review the current changes (git diff) and provide constructive feedback. \
                Focus on: correctness, security, performance, readability, and best practices. \
                Be specific about issues and suggest improvements."
                .into(),
            allowed_tools: vec!["Bash".into(), "Read".into(), "Grep".into()],
            model: None,
            source: SkillSource::Bundled,
            path: PathBuf::from("bundled://review"),
        },
        SkillDefinition {
            name: "debug".into(),
            description: "Debug an issue by analyzing errors and suggesting fixes".into(),
            when_to_use: Some("When the user encounters an error or bug".into()),
            prompt: "Help debug the issue. Steps:\n\
                1. Understand the error message or unexpected behavior\n\
                2. Search for relevant code using Grep/Read\n\
                3. Identify the root cause\n\
                4. Suggest a fix with specific code changes\n\
                5. Verify the fix if possible"
                .into(),
            allowed_tools: vec!["Bash".into(), "Read".into(), "Grep".into(), "Glob".into()],
            model: None,
            source: SkillSource::Bundled,
            path: PathBuf::from("bundled://debug"),
        },
        SkillDefinition {
            name: "stuck".into(),
            description: "Help when you're stuck on a problem".into(),
            when_to_use: Some("When the user is stuck and needs a fresh perspective".into()),
            prompt: "The user is stuck. Help by:\n\
                1. Understanding what they've tried so far\n\
                2. Exploring the codebase for relevant patterns\n\
                3. Suggesting alternative approaches\n\
                4. Breaking the problem into smaller steps"
                .into(),
            allowed_tools: vec!["Read".into(), "Grep".into(), "Glob".into(), "Bash".into()],
            model: None,
            source: SkillSource::Bundled,
            path: PathBuf::from("bundled://stuck"),
        },
        SkillDefinition {
            name: "remember".into(),
            description: "Review and organize auto-memory entries".into(),
            when_to_use: Some("When the user wants to review or organize memory entries".into()),
            prompt: "Review the memory landscape:\n\
                1. Read CLAUDE.md and CLAUDE.local.md\n\
                2. Classify entries by destination (CLAUDE.md, CLAUDE.local.md, stay)\n\
                3. Identify duplicates, outdated entries, conflicts\n\
                4. Present proposals grouped by action type\n\
                Do NOT modify files without explicit user approval."
                .into(),
            allowed_tools: vec!["Read".into(), "Edit".into(), "Write".into()],
            model: None,
            source: SkillSource::Bundled,
            path: PathBuf::from("bundled://remember"),
        },
    ]
}

// ── SkillTool ──

pub struct SkillTool;

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Invoke a skill (slash command) by name. Skills are reusable prompts \
         defined in .claude/skills/ directories."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The skill name (e.g., 'commit', 'review-pr')"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let skill_name = input
            .get("skill")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing skill name".into()))?;
        let args = input.get("args").and_then(|v| v.as_str()).unwrap_or("");

        // Strip leading slash
        let skill_name = skill_name.strip_prefix('/').unwrap_or(skill_name);

        // Load all skills
        let skills = load_all_skills(&ctx.cwd).await;

        // Find matching skill
        let skill = skills
            .iter()
            .find(|s| s.name == skill_name)
            .ok_or_else(|| {
                let available: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
                RclaudeError::Tool(format!(
                    "Skill '{}' not found. Available: {}",
                    skill_name,
                    available.join(", ")
                ))
            })?;

        // Substitute $ARGUMENTS in prompt
        let prompt = if args.is_empty() {
            skill.prompt.clone()
        } else {
            skill
                .prompt
                .replace("$ARGUMENTS", args)
                .replace("${ARGUMENTS}", args)
        };

        // Return the skill prompt as a message for the model to process
        let mut result = format!("Skill '{}' loaded.\n\n", skill_name);
        if !skill.allowed_tools.is_empty() {
            result.push_str(&format!(
                "Allowed tools: {}\n",
                skill.allowed_tools.join(", ")
            ));
        }
        if let Some(ref model) = skill.model {
            result.push_str(&format!("Model: {model}\n"));
        }
        result.push_str(&format!("\n{prompt}"));

        Ok(ToolResult::text(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content =
            "---\ndescription: Test skill\ntools: Read, Grep\nmodel: haiku\n---\nDo the thing.";
        let (fm, body) = parse_frontmatter(content);
        assert_eq!(fm.get("description").unwrap(), "Test skill");
        assert_eq!(fm.get("tools").unwrap(), "Read, Grep");
        assert_eq!(fm.get("model").unwrap(), "haiku");
        assert_eq!(body, "Do the thing.");
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "Just a prompt with no frontmatter.";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_skill_file() {
        let content = "---\ndescription: My skill\nwhen-to-use: When needed\n---\nDo stuff.";
        let skill = parse_skill_file(
            Path::new("/tmp/test-skill.md"),
            content,
            SkillSource::Project,
        )
        .unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "My skill");
        assert_eq!(skill.when_to_use.as_deref(), Some("When needed"));
        assert_eq!(skill.prompt, "Do stuff.");
    }

    #[test]
    fn test_bundled_skills() {
        let skills = get_bundled_skills();
        assert!(skills.iter().any(|s| s.name == "commit"));
        assert!(skills.iter().any(|s| s.name == "review"));
    }

    #[test]
    fn test_argument_substitution() {
        let prompt = "Review the PR: $ARGUMENTS";
        let result = prompt.replace("$ARGUMENTS", "fix-bug-123");
        assert_eq!(result, "Review the PR: fix-bug-123");
    }
}
