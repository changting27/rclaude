# 🦀 rclaude

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-452%20passing-green.svg)]()

A Rust CLI for interacting with Anthropic's Claude API — built from public API docs, community resources, and the official CLI `--help` output.

[中文文档](README_zh.md)

> 33,600+ lines of Rust · 8 crates · 452 tests · 0 clippy warnings

## Quick Start

```bash
# Install
cargo install --path .

# Set your API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Start coding
rclaude                        # Interactive REPL
rclaude -p "explain this code" # One-shot query
rclaude --tui                  # TUI mode
```

Requirements: Rust 1.75+. Linux needs `pkg-config` + `openssl-dev`.

Also supports **AWS Bedrock** (`AWS_PROFILE`) and **Google Vertex AI** (`CLOUD_ML_REGION`) — provider auto-detected.

## Highlights

**Fast startup** — ~50ms cold start. Single ~15MB binary, no Node.js runtime.

**48 tools, 73 commands** — File read/write/edit, Bash (with tree-sitter AST security analysis), Grep, Glob, LSP, MCP, Agent system with memory snapshots, 6 task management tools, and more.

**Smart agentic loop** — Streaming output, auto-retry (429/529), auto-compact when context fills up, output continuation on truncation, parallel tool execution for safe tools.

**Plugin ecosystem** — Browse/search/install plugins from marketplace. Enable/disable via settings. MCP server and hooks integration from plugins.

**4 permission modes** — Default / Auto / Bypass / Plan. Auto mode uses a safe-tool allowlist. Interactive Allow/Always/Deny prompts with persistent rules.

## Usage

```bash
rclaude "refactor this function"          # Direct prompt
rclaude -p "what does main.rs do"         # Print mode (no follow-up)
rclaude -m claude-sonnet-4-20250514       # Specify model
rclaude -r                                # Resume last session
rclaude --permission-mode auto            # Auto-approve safe tools
rclaude --max-budget-usd 1.0             # Set cost cap
```

Inside the REPL: `/help` `/compact` `/model` `/cost` `/commit` `/diff` `/review` `/plugin` `/mcp` `/tasks` and 60+ more slash commands.

Full options: `rclaude --help`

## Architecture

```
rclaude/
├── src/main.rs + query_engine.rs    1,813  # CLI + core agentic loop
├── crates/
│   ├── rclaude-core/                8,568  # Messages, config, permissions, hooks, plugins
│   ├── rclaude-tools/              10,095  # 48 tool modules
│   ├── rclaude-services/            4,903  # Compact, session, LSP, setup
│   ├── rclaude-commands/            3,486  # 73 slash commands
│   ├── rclaude-api/                 1,222  # Anthropic/Bedrock/Vertex clients
│   ├── rclaude-utils/               1,207  # Git, shell, path utilities
│   ├── rclaude-mcp/                 1,654  # MCP stdio/SSE/OAuth client
│   └── rclaude-tui/                   689  # ratatui TUI
└── tests/                           1,015  # Integration tests
```

## Development

```bash
cargo check --workspace                   # 0 errors, 0 warnings
cargo test --workspace                    # 452 tests
cargo clippy --workspace -- -D warnings   # 0 warnings
cargo fmt --check                         # Consistent formatting
```

## References

This project is built entirely from publicly available resources:

- [Anthropic Messages API documentation](https://docs.anthropic.com/en/api/messages)
- [Anthropic Claude CLI official documentation](https://docs.anthropic.com/en/docs/claude-code)
- Claude CLI `--help` output and public user guides
- Community discussions, blog posts, and open-source tooling around the Anthropic API

## License

[MIT](LICENSE)
