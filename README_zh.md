# 🦀 rclaude

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-452%20passing-green.svg)]()

基于 Anthropic Claude API 公开文档和社区资源构建的 Rust 命令行工具。

[English](README.md)

> 33,600+ 行 Rust · 8 crates · 452 测试 · 0 clippy warnings

## 快速开始

```bash
# 安装
cargo install --path .

# 设置 API Key
export ANTHROPIC_API_KEY="sk-ant-..."

# 开始使用
rclaude                        # 交互式 REPL
rclaude -p "explain this code" # 单次提问
rclaude --tui                  # TUI 模式
```

环境要求：Rust 1.75+、Linux 需 `pkg-config` + `openssl-dev`。

同时支持 **AWS Bedrock**（`AWS_PROFILE`）和 **Google Vertex AI**（`CLOUD_ML_REGION`），自动检测。

## 核心亮点

**快速启动** — 冷启动 ~50ms。单文件 ~15MB，无需 Node.js。

**48 工具 · 73 命令** — 文件读写编辑、Bash（tree-sitter AST 安全分析）、Grep、Glob、LSP、MCP、Agent 内存快照、6 个任务管理工具等。

**智能 agentic 循环** — 流式输出、自动重试（429/529）、上下文满时自动 compact、截断时自动续写、安全工具并行执行。

**插件生态** — 从 marketplace 浏览/搜索/安装插件。通过 settings 启用/禁用。插件可提供 MCP 服务器和 hooks。

**4 种权限模式** — Default / Auto / Bypass / Plan。Auto 模式使用安全工具白名单。交互式 Allow/Always/Deny 提示，规则持久化。

## 使用示例

```bash
rclaude "重构这个函数"                     # 直接提问
rclaude -p "main.rs 做了什么"              # Print 模式
rclaude -m claude-sonnet-4-20250514       # 指定模型
rclaude -r                                # 恢复上次会话
rclaude --permission-mode auto            # 自动批准安全工具
rclaude --max-budget-usd 1.0             # 设置费用上限
```

REPL 内置命令：`/help` `/compact` `/model` `/cost` `/commit` `/diff` `/review` `/plugin` `/mcp` `/tasks` 等 60+ 个。

完整参数：`rclaude --help`

## 架构

```
rclaude/
├── src/main.rs + query_engine.rs    1,813  # CLI + 核心 agentic 循环
├── crates/
│   ├── rclaude-core/                8,568  # 消息、配置、权限、hooks、插件
│   ├── rclaude-tools/              10,095  # 48 个工具模块
│   ├── rclaude-services/            4,903  # compact、session、LSP、setup
│   ├── rclaude-commands/            3,486  # 73 个斜杠命令
│   ├── rclaude-api/                 1,222  # Anthropic/Bedrock/Vertex 客户端
│   ├── rclaude-utils/               1,207  # git、shell、路径工具
│   ├── rclaude-mcp/                 1,654  # MCP stdio/SSE/OAuth 客户端
│   └── rclaude-tui/                   689  # ratatui TUI
└── tests/                           1,015  # 集成测试
```

## 开发

```bash
cargo check --workspace                   # 0 errors, 0 warnings
cargo test --workspace                    # 452 tests
cargo clippy --workspace -- -D warnings   # 0 warnings
cargo fmt --check                         # 格式一致
```

## 参考资料

本项目完全基于公开可用的资源构建：

- [Anthropic Messages API 文档](https://docs.anthropic.com/en/api/messages)
- [Anthropic Claude CLI 官方文档](https://docs.anthropic.com/en/docs/claude-code)
- Claude CLI `--help` 输出及公开用户指南
- 社区讨论、博客文章及围绕 Anthropic API 的开源工具

## 许可证

[MIT](LICENSE)
