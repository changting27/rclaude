//! Theme system for terminal color customization.

/// Available themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

/// Theme colors.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub muted: &'static str,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        match self {
            Self::Dark | Self::System => ThemeColors {
                primary: "\x1b[36m",
                secondary: "\x1b[34m",
                success: "\x1b[32m",
                warning: "\x1b[33m",
                error: "\x1b[31m",
                muted: "\x1b[90m",
            },
            Self::Light => ThemeColors {
                primary: "\x1b[96m",
                secondary: "\x1b[94m",
                success: "\x1b[92m",
                warning: "\x1b[93m",
                error: "\x1b[91m",
                muted: "\x1b[37m",
            },
        }
    }

    pub fn parse_theme(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => Self::Light,
            "system" => Self::System,
            _ => Self::Dark,
        }
    }
}

pub const RESET: &str = "\x1b[0m";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_parse() {
        assert_eq!(Theme::parse_theme("dark"), Theme::Dark);
        assert_eq!(Theme::parse_theme("light"), Theme::Light);
        assert_eq!(Theme::parse_theme("unknown"), Theme::Dark);
    }

    #[test]
    fn test_default() {
        assert_eq!(Theme::default(), Theme::Dark);
    }
}
