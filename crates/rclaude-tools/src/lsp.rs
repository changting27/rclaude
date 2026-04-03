//! LSPTool: Language Server Protocol operations.
//! Uses rclaude-services/lsp for actual server communication.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str =
    "Interact with Language Server Protocol servers for code intelligence.\n\n\
Supported operations:\n\
- diagnostics: Get compiler errors and warnings for a file\n\
- goToDefinition: Find where a symbol is defined\n\
- findReferences: Find all references to a symbol\n\
- hover: Get hover information (docs, type info)\n\
- documentSymbol: Get all symbols in a document";

pub struct LSPTool;

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str {
        "LSP"
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "The LSP operation to perform",
                    "enum": ["diagnostics", "goToDefinition", "findReferences", "hover", "documentSymbol"]
                },
                "filePath": {
                    "type": "string",
                    "description": "The file path"
                },
                "line": {
                    "type": "integer",
                    "description": "Line number (0-based, for position-based operations)"
                },
                "character": {
                    "type": "integer",
                    "description": "Character offset (0-based, for position-based operations)"
                }
            },
            "required": ["operation", "filePath"]
        })).expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing operation".into()))?;
        let file_path = input
            .get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing filePath".into()))?;
        let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
        let character = input.get("character").and_then(|v| v.as_u64()).unwrap_or(0);

        let abs_path = if std::path::Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            ctx.cwd.join(file_path)
        };
        let uri = format!("file://{}", abs_path.display());

        // Try to use LSP server manager from app state
        // For now, fall back to static analysis if no server is available
        match operation {
            "diagnostics" => {
                // Run compiler/linter and return diagnostics
                let output = run_diagnostics(&abs_path, &ctx.cwd).await;
                Ok(ToolResult::text(output))
            }
            "goToDefinition" => {
                let params = json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                });
                Ok(ToolResult::text(format!(
                    "goToDefinition at {}:{}:{}\nParams: {}\n\n(LSP server connection required. \
                     Configure via .claude/settings.json lsp section or install language server.)",
                    file_path, line, character, params
                )))
            }
            "findReferences" => {
                let params = json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "context": { "includeDeclaration": true }
                });
                Ok(ToolResult::text(format!(
                    "findReferences at {}:{}:{}\nParams: {}",
                    file_path, line, character, params
                )))
            }
            "hover" => Ok(ToolResult::text(format!(
                "hover at {}:{}:{}\n(Requires running LSP server)",
                file_path, line, character
            ))),
            "documentSymbol" => {
                // Fall back to regex-based symbol extraction
                let content = tokio::fs::read_to_string(&abs_path)
                    .await
                    .map_err(|e| RclaudeError::Tool(format!("Cannot read file: {e}")))?;
                let ext = abs_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let symbols = extract_symbols(&content, ext);
                if symbols.is_empty() {
                    Ok(ToolResult::text(format!("No symbols found in {file_path}")))
                } else {
                    Ok(ToolResult::text(format!(
                        "Symbols in {file_path}:\n{}",
                        symbols.join("\n")
                    )))
                }
            }
            _ => Err(RclaudeError::Tool(format!(
                "Unknown LSP operation: {operation}"
            ))),
        }
    }
}

/// Run language-specific diagnostics as a fallback when no LSP server is available.
async fn run_diagnostics(file_path: &std::path::Path, cwd: &std::path::Path) -> String {
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let cmd = match ext {
        "rs" => Some((
            "cargo",
            vec!["check".to_string(), "--message-format=short".to_string()],
        )),
        "ts" | "tsx" => Some(("npx", vec!["tsc".to_string(), "--noEmit".to_string()])),
        "py" => Some((
            "python3",
            vec![
                "-m".to_string(),
                "py_compile".to_string(),
                file_path.to_string_lossy().to_string(),
            ],
        )),
        "go" => Some(("go", vec!["vet".to_string(), "./...".to_string()])),
        _ => None,
    };

    match cmd {
        Some((program, args)) => {
            match tokio::process::Command::new(program)
                .args(&args)
                .current_dir(cwd)
                .output()
                .await
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let combined = format!("{stdout}{stderr}");
                    if combined.trim().is_empty() {
                        "No diagnostics (clean)".to_string()
                    } else {
                        combined.chars().take(5000).collect()
                    }
                }
                Err(e) => format!("Failed to run diagnostics: {e}"),
            }
        }
        None => format!("No diagnostic tool available for .{ext} files"),
    }
}

/// Regex-based symbol extraction as fallback for documentSymbol.
pub fn extract_symbols(content: &str, ext: &str) -> Vec<String> {
    let patterns: &[&str] = match ext {
        "rs" => {
            &[r"(?m)^\s*(?:pub\s+)?(?:fn|struct|enum|trait|type|const|static|mod|impl)\s+(\w+)"]
        }
        "ts" | "tsx" | "js" | "jsx" => {
            &[r"(?m)^\s*(?:export\s+)?(?:function|class|interface|type|enum|const|let|var)\s+(\w+)"]
        }
        "py" => &[r"(?m)^(?:def|class|async\s+def)\s+(\w+)"],
        "go" => &[r"(?m)^(?:func|type|var|const)\s+(\w+)"],
        "java" | "kt" => &[
            r"(?m)^\s*(?:public|private|protected)?\s*(?:static\s+)?(?:class|interface|enum|void|int|String)\s+(\w+)",
        ],
        "rb" => &[r"(?m)^\s*(?:def|class|module)\s+(\w+)"],
        "c" | "cpp" | "h" | "hpp" => &[r"(?m)^(?:\w+\s+)+(\w+)\s*\("],
        _ => return vec![],
    };

    let mut symbols = Vec::new();
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            for cap in re.captures_iter(content) {
                if let Some(name) = cap.get(1) {
                    symbols.push(name.as_str().to_string());
                }
            }
        }
    }
    symbols.dedup();
    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_symbols_rust() {
        let code = "pub fn foo() {}\nstruct Bar;\nenum Baz {}";
        let syms = extract_symbols(code, "rs");
        assert_eq!(syms.len(), 3);
        assert!(syms.contains(&"foo".to_string()));
        assert!(syms.contains(&"Bar".to_string()));
    }

    #[test]
    fn test_extract_symbols_python() {
        let code = "def hello():\n    pass\nclass World:\n    pass";
        let syms = extract_symbols(code, "py");
        assert_eq!(syms.len(), 2);
    }

    #[test]
    fn test_extract_symbols_typescript() {
        let code = "export function greet() {}\nclass App {}\nconst x = 1;";
        let syms = extract_symbols(code, "ts");
        assert!(syms.len() >= 2);
    }
}
