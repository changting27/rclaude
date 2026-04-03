# 🦀 rclaude

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-452%20passing-green.svg)]()

A fast, native CLI for working with Anthropic's Claude — **~50ms startup**, single binary, no Node.js required.

[中文文档](README_zh.md)

## Why rclaude?

- ⚡ **Fast** — ~50ms cold start, ~15MB binary. No runtime overhead.
- 🔧 **45 built-in tools** — File ops, Bash (with AST security analysis), Grep, Glob, LSP, MCP, Agents, and more.
- 🔄 **Smart agentic loop** — Auto-retry on rate limits, auto-compact on context overflow, output continuation on truncation.
- 🔌 **Extensible** — Plugin marketplace, MCP servers (stdio/SSE/WebSocket), custom hooks and skills.
- 🔒 **4 permission modes** — From fully interactive to fully autonomous, with persistent rules.
- 🏗️ **Multi-provider** — Anthropic, AWS Bedrock, Google Vertex AI — auto-detected from environment.

> 34,600+ lines of Rust · 8 crates · 452 tests · 0 clippy warnings

## Quick Start

```bash
# Build from source (requires Rust 1.75+, see "Prerequisites" below)
git clone <repo-url> rclaude
cd rclaude
cargo install --path .

# Set your API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Go
rclaude                        # Interactive REPL
rclaude -p "explain this code" # One-shot query
rclaude --tui                  # Terminal UI mode
```

## Usage

```bash
rclaude "refactor this function"          # Direct prompt
rclaude -p "what does main.rs do"         # Print mode (no follow-up)
rclaude -m claude-sonnet-4-20250514       # Specify model
rclaude -r                                # Resume last session
rclaude --permission-mode auto            # Auto-approve safe tools
rclaude --max-budget-usd 1.0             # Set cost cap
```

### Permission Modes

| Mode | Behavior |
|------|----------|
| `default` | Interactive Allow/Always/Deny prompts for risky tools |
| `auto` | Safe tools (read-only) auto-approved, risky tools still prompt |
| `bypass` | All tools auto-approved (use with caution) |
| `plan` | Read-only tools only, no write operations |

### REPL Commands

73 slash commands organized by category:

| Category | Commands |
|----------|----------|
| Session | `/compact` `/resume` `/clear` `/export` `/session` `/rewind` |
| Code | `/commit` `/diff` `/review` `/pr-comments` `/release-notes` |
| Model | `/model` `/cost` `/stats` `/usage` `/effort` |
| Config | `/config` `/permissions` `/memory` `/hooks` `/theme` |
| Tools | `/mcp` `/plugin` `/skills` `/agents` `/tasks` |
| Info | `/help` `/doctor` `/version` `/context` `/files` |

Type `/help` in the REPL for the full list.

## Features

### Tools (45)

| Category | Tools |
|----------|-------|
| File system | Read, Write, Edit, Glob, Grep |
| Execution | Bash (tree-sitter AST security), PowerShell |
| Code intelligence | LSP integration, notebook support |
| Agents | Sub-agent spawning, memory snapshots, forked agents |
| MCP | Tool proxy, resource read, OAuth authentication |
| Web | Search, Fetch |
| Workflow | Task management, plan mode, todo tracking |

### Agentic Loop

- **Streaming output** with real-time token display
- **Auto-retry** on 429 (rate limit) with countdown timer, 529 with exponential backoff
- **Auto-compact** when context window fills up — restores recent files, active plan, skills, and session memory
- **Output continuation** — auto-continues up to 3 times on truncated responses
- **Parallel tool execution** for safe (read-only) tools

### Plugin & MCP Ecosystem

- Browse, search, and install plugins from marketplace
- MCP server support: **stdio**, **SSE**, and **WebSocket** transports
- OAuth (PKCE) authentication for remote MCP servers
- Auto-discovery of `.mcp.json` configs in project tree
- Custom hooks (6 event types) with conditional filtering

### Session Management

- JSONL transcript persistence with message chain linking
- Cross-session keyword search (text, tool inputs, tool results)
- Session resume (`rclaude -r`)
- Agent sidechain isolation — sub-agents get their own transcript

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key (required) |
| `CLAUDE_MODEL` | Override default model |
| `ANTHROPIC_BASE_URL` | Custom API endpoint |
| `AWS_PROFILE` / `AWS_REGION` | AWS Bedrock authentication |
| `CLOUD_ML_REGION` | Google Vertex AI region |

### API Providers

Auto-detected from environment:

- **Anthropic** (default) — set `ANTHROPIC_API_KEY`
- **AWS Bedrock** — set `AWS_PROFILE` and `AWS_REGION`
- **Google Vertex AI** — set `CLOUD_ML_REGION`

## Architecture

```
rclaude/                                  Lines
├── src/
│   ├── main.rs                           1,020   CLI entry, REPL, session management
│   └── query_engine.rs                     793   Core agentic loop, streaming, tool dispatch
├── crates/
│   ├── rclaude-core/                     8,554   Messages, config, permissions, hooks, plugins
│   ├── rclaude-tools/                   10,081   45 tool implementations
│   ├── rclaude-services/                 4,898   Compact, session storage, LSP, diagnostics
│   ├── rclaude-commands/                 3,485   73 slash commands
│   ├── rclaude-mcp/                      1,653   MCP client (stdio/SSE/WebSocket/OAuth)
│   ├── rclaude-api/                      1,222   Anthropic/Bedrock/Vertex API clients
│   ├── rclaude-utils/                    1,207   Git, shell, path, formatting utilities
│   └── rclaude-tui/                        689   Terminal UI (ratatui)
└── tests/                                1,014   Integration tests
                                         ──────
                                         34,616   Total
```

## Prerequisites

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version   # 1.75+
```

### System Dependencies

| Platform | Command |
|----------|---------|
| Debian/Ubuntu | `sudo apt install -y pkg-config libssl-dev build-essential gcc` |
| Fedora/RHEL | `sudo dnf install -y pkg-config openssl-devel gcc` |
| macOS | `xcode-select --install` |
| Windows | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "C++ build tools" |

## Development

```bash
cargo build                               # Debug build
cargo build --release                     # Release build
cargo test --workspace                    # Run all 452 tests
cargo clippy --workspace -- -D warnings   # Lint (0 warnings required)
cargo fmt --check                         # Format check
```

## Roadmap

- [ ] Plugin DXT runtime (package install, tool registration, config flow)
- [ ] TUI enhancements (markdown rendering, syntax highlighting, diff view)
- [ ] Multi-agent coordination (Swarm)
- [ ] Voice input mode
- [ ] IDE integration
- [ ] Remote control (Bridge)

See [docs/TODO.md](docs/TODO.md) for the full list.

## Contributing

Contributions are welcome! Please:

1. Fork the repo and create a feature branch
2. Ensure all checks pass: `cargo check && cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --check`
3. Submit a PR with a clear description of the change

## References

Built from publicly available resources:

- [Anthropic Messages API documentation](https://docs.anthropic.com/en/api/messages)
- [Anthropic Claude CLI official documentation](https://docs.anthropic.com/en/docs/claude-code)
- Claude CLI `--help` output and public user guides
- Community discussions, blog posts, and open-source tooling around the Anthropic API

## License

[Apache 2.0](LICENSE)
