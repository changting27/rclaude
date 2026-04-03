//! Prompt suggestions matching services/PromptSuggestion/.
//! Suggests relevant prompts based on context.

/// A prompt suggestion.
#[derive(Debug, Clone)]
pub struct PromptSuggestion {
    pub text: String,
    pub category: SuggestionCategory,
    pub relevance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionCategory {
    Command,
    Question,
    Task,
    Fix,
}

/// Generate suggestions based on current context.
pub fn get_suggestions(
    _cwd: &std::path::Path,
    is_git: bool,
    has_errors: bool,
    _recent_tools: &[&str],
) -> Vec<PromptSuggestion> {
    let mut suggestions = Vec::new();

    // Git-related suggestions
    if is_git {
        suggestions.push(PromptSuggestion {
            text: "Review my recent changes and suggest improvements".into(),
            category: SuggestionCategory::Task,
            relevance: 0.8,
        });
        suggestions.push(PromptSuggestion {
            text: "Write a commit message for the staged changes".into(),
            category: SuggestionCategory::Task,
            relevance: 0.7,
        });
    }

    // Error-related suggestions
    if has_errors {
        suggestions.push(PromptSuggestion {
            text: "Help me fix the errors in the output above".into(),
            category: SuggestionCategory::Fix,
            relevance: 0.9,
        });
    }

    // General suggestions
    suggestions.push(PromptSuggestion {
        text: "Explain the architecture of this project".into(),
        category: SuggestionCategory::Question,
        relevance: 0.5,
    });
    suggestions.push(PromptSuggestion {
        text: "Find and fix any bugs in the codebase".into(),
        category: SuggestionCategory::Task,
        relevance: 0.4,
    });

    // Sort by relevance
    suggestions.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_suggestions() {
        let s = get_suggestions(std::path::Path::new("/tmp"), true, false, &[]);
        assert!(s.iter().any(|s| s.text.contains("commit")));
    }

    #[test]
    fn test_error_suggestions() {
        let s = get_suggestions(std::path::Path::new("/tmp"), false, true, &[]);
        assert!(s.iter().any(|s| s.category == SuggestionCategory::Fix));
    }
}
