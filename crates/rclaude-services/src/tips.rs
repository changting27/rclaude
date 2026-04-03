//! Tips service matching services/tips/.
//! Shows contextual tips to help users learn features.

use std::collections::HashSet;

/// A tip to show the user.
#[derive(Debug, Clone)]
pub struct Tip {
    pub id: &'static str,
    pub message: &'static str,
    pub category: TipCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipCategory {
    Workflow,
    Performance,
    Feature,
    Shortcut,
}

const TIPS: &[Tip] = &[
    Tip { id: "parallel_tools", message: "Tip: Claude can call multiple tools in parallel. Ask it to search and read files simultaneously for faster results.", category: TipCategory::Performance },
    Tip { id: "slash_commands", message: "Tip: Use /help to see all available commands. Try /compact to free up context space.", category: TipCategory::Feature },
    Tip { id: "at_mentions", message: "Tip: Use @filename to attach file contents to your message.", category: TipCategory::Feature },
    Tip { id: "auto_mode", message: "Tip: Use --permission-mode auto to skip permission prompts for safe commands.", category: TipCategory::Workflow },
    Tip { id: "claude_md", message: "Tip: Create a CLAUDE.md file in your project root with project-specific instructions.", category: TipCategory::Workflow },
    Tip { id: "skills", message: "Tip: Create custom skills in .claude/skills/ to automate repetitive tasks.", category: TipCategory::Feature },
    Tip { id: "agents", message: "Tip: Use the Explore agent type for fast read-only codebase searches.", category: TipCategory::Performance },
    Tip { id: "vim_mode", message: "Tip: Use /vim to toggle vim keybindings in the input.", category: TipCategory::Shortcut },
    Tip { id: "resume", message: "Tip: Use --continue to resume your most recent conversation.", category: TipCategory::Workflow },
    Tip { id: "cost", message: "Tip: Use /cost to see your current session's token usage and cost.", category: TipCategory::Feature },
];

/// Tips manager tracks which tips have been shown.
pub struct TipsManager {
    shown: HashSet<String>,
}

impl TipsManager {
    pub fn new() -> Self {
        Self {
            shown: HashSet::new(),
        }
    }

    /// Get the next unshown tip.
    pub fn next_tip(&mut self) -> Option<&'static Tip> {
        for tip in TIPS {
            if !self.shown.contains(tip.id) {
                self.shown.insert(tip.id.to_string());
                return Some(tip);
            }
        }
        None
    }

    /// Get a random unshown tip.
    pub fn random_tip(&mut self) -> Option<&'static Tip> {
        let unshown: Vec<_> = TIPS.iter().filter(|t| !self.shown.contains(t.id)).collect();
        if unshown.is_empty() {
            return None;
        }
        let idx = rand::random::<usize>() % unshown.len();
        let tip = unshown[idx];
        self.shown.insert(tip.id.to_string());
        Some(tip)
    }

    /// Mark all tips as shown.
    pub fn dismiss_all(&mut self) {
        for tip in TIPS {
            self.shown.insert(tip.id.to_string());
        }
    }

    pub fn remaining_count(&self) -> usize {
        TIPS.len() - self.shown.len()
    }
}

impl Default for TipsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_tip() {
        let mut mgr = TipsManager::new();
        let tip = mgr.next_tip().unwrap();
        assert!(!tip.id.is_empty());
        assert!(mgr.remaining_count() < TIPS.len());
    }

    #[test]
    fn test_dismiss_all() {
        let mut mgr = TipsManager::new();
        mgr.dismiss_all();
        assert_eq!(mgr.remaining_count(), 0);
        assert!(mgr.next_tip().is_none());
    }
}
