//! Command suggestions matching utils/suggestions/.
//! Provides autocomplete and fuzzy matching for slash commands.

/// A command suggestion.
#[derive(Debug, Clone)]
pub struct CommandSuggestion {
    pub command: String,
    pub description: String,
    pub score: f64,
}

/// Check if input starts with a slash command.
pub fn is_command_input(input: &str) -> bool {
    input.starts_with('/')
}

/// Check if a command input has arguments.
pub fn has_command_args(input: &str) -> bool {
    input.contains(' ')
}

/// Find the best matching command for partial input.
pub fn get_best_match<'a>(
    input: &str,
    commands: &'a [(String, String)],
) -> Option<&'a (String, String)> {
    let input_lower = input.trim_start_matches('/').to_lowercase();
    if input_lower.is_empty() {
        return None;
    }

    // Exact prefix match first
    if let Some(cmd) = commands
        .iter()
        .find(|(name, _)| name.to_lowercase() == input_lower)
    {
        return Some(cmd);
    }

    // Prefix match
    let mut matches: Vec<_> = commands
        .iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&input_lower))
        .collect();
    matches.sort_by_key(|(name, _)| name.len());
    matches.first().copied()
}

/// Generate command suggestions for partial input.
pub fn generate_suggestions(input: &str, commands: &[(String, String)]) -> Vec<CommandSuggestion> {
    let query = input.trim_start_matches('/').to_lowercase();
    if query.is_empty() {
        return commands
            .iter()
            .map(|(name, desc)| CommandSuggestion {
                command: format!("/{name}"),
                description: desc.clone(),
                score: 1.0,
            })
            .collect();
    }

    let mut suggestions: Vec<CommandSuggestion> = commands
        .iter()
        .filter_map(|(name, desc)| {
            let name_lower = name.to_lowercase();
            let score = if name_lower == query {
                1.0
            } else if name_lower.starts_with(&query) {
                0.8
            } else if name_lower.contains(&query) {
                0.5
            } else {
                return None;
            };
            Some(CommandSuggestion {
                command: format!("/{name}"),
                description: desc.clone(),
                score,
            })
        })
        .collect();

    suggestions.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions
}

/// Find slash command positions in text (for mid-input detection).
pub fn find_slash_positions(input: &str) -> Vec<usize> {
    input
        .char_indices()
        .filter(|(_, c)| *c == '/')
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_commands() -> Vec<(String, String)> {
        vec![
            ("help".into(), "Show help".into()),
            ("compact".into(), "Compact context".into()),
            ("cost".into(), "Show cost".into()),
            ("clear".into(), "Clear screen".into()),
        ]
    }

    #[test]
    fn test_best_match() {
        let cmds = test_commands();
        let m = get_best_match("/co", &cmds).unwrap();
        assert!(m.0 == "compact" || m.0 == "cost"); // both are valid prefix matches
    }

    #[test]
    fn test_suggestions() {
        let cmds = test_commands();
        let s = generate_suggestions("c", &cmds);
        assert_eq!(s.len(), 3); // compact, cost, clear
    }
}
