//! IDE integration for detecting and launching editors.
//! Detects and integrates with IDEs (VS Code, JetBrains, etc.).

/// Supported IDE types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeType {
    VSCode,
    VSCodeInsiders,
    Cursor,
    Windsurf,
    IntelliJ,
    WebStorm,
    PyCharm,
    GoLand,
    RustRover,
    CLion,
    Zed,
    Sublime,
    Vim,
    Neovim,
    Emacs,
    Unknown,
}

impl IdeType {
    pub fn is_vscode(&self) -> bool {
        matches!(
            self,
            Self::VSCode | Self::VSCodeInsiders | Self::Cursor | Self::Windsurf
        )
    }

    pub fn is_jetbrains(&self) -> bool {
        matches!(
            self,
            Self::IntelliJ
                | Self::WebStorm
                | Self::PyCharm
                | Self::GoLand
                | Self::RustRover
                | Self::CLion
        )
    }
}

/// Detect IDE from terminal environment variables.
pub fn detect_terminal_ide() -> Option<IdeType> {
    // VS Code sets TERM_PROGRAM
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        if term == "vscode" {
            return Some(IdeType::VSCode);
        }
    }

    // Check TERMINAL_EMULATOR for JetBrains
    if let Ok(emulator) = std::env::var("TERMINAL_EMULATOR") {
        if emulator.contains("JetBrains") {
            return Some(IdeType::IntelliJ);
        }
    }

    // Check for Cursor
    if std::env::var("CURSOR_TRACE_ID").is_ok() {
        return Some(IdeType::Cursor);
    }

    None
}

/// Get the open command for an IDE.
pub fn get_open_command(ide: IdeType, path: &str, line: Option<u32>) -> Option<Vec<String>> {
    match ide {
        IdeType::VSCode | IdeType::VSCodeInsiders => {
            let cmd = if ide == IdeType::VSCodeInsiders {
                "code-insiders"
            } else {
                "code"
            };
            let target = match line {
                Some(l) => format!("{path}:{l}"),
                None => path.to_string(),
            };
            Some(vec![cmd.to_string(), "--goto".into(), target])
        }
        IdeType::Cursor => {
            let target = match line {
                Some(l) => format!("{path}:{l}"),
                None => path.to_string(),
            };
            Some(vec!["cursor".into(), "--goto".into(), target])
        }
        _ if ide.is_jetbrains() => {
            let mut args = vec!["idea".into(), path.to_string()];
            if let Some(l) = line {
                args.push(format!("--line={l}"));
            }
            Some(args)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ide_classification() {
        assert!(IdeType::VSCode.is_vscode());
        assert!(IdeType::Cursor.is_vscode());
        assert!(IdeType::IntelliJ.is_jetbrains());
        assert!(!IdeType::VSCode.is_jetbrains());
    }

    #[test]
    fn test_open_command() {
        let cmd = get_open_command(IdeType::VSCode, "/src/main.rs", Some(42)).unwrap();
        assert_eq!(cmd, vec!["code", "--goto", "/src/main.rs:42"]);
    }
}
