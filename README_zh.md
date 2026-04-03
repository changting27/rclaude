# 🦀 rclaude

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-452%20passing-green.svg)]()

快速、原生的 Anthropic Claude 命令行工具 — **~50ms 启动**，单文件二进制，无需 Node.js。

[English](README.md)

## 为什么选择 rclaude？

- ⚡ **快速** — 冷启动 ~50ms，二进制 ~15MB，零运行时开销
- 🔧 **45 个内置工具** — 文件操作、Bash（AST 安全分析）、Grep、Glob、LSP、MCP、Agent 等
- 🔄 **智能 agentic 循环** — 限流自动重试、上下文溢出自动 compact、截断自动续写
- 🔌 **可扩展** — 插件市场、MCP 服务器（stdio/SSE/WebSocket）、自定义 hooks 和 skills
- 🔒 **4 种权限模式** — 从完全交互到完全自主，规则持久化
- 🏗️ **多提供商** — Anthropic、AWS Bedrock、Google Vertex AI，环境变量自动检测

> 34,600+ 行 Rust · 8 crates · 452 测试 · 0 clippy warnings

## 快速开始

```bash
# 从源码构建（需要 Rust 1.75+，见下方"环境准备"）
git clone <repo-url> rclaude
cd rclaude
cargo install --path .

# 设置 API Key
export ANTHROPIC_API_KEY="sk-ant-..."

# 开始
rclaude                        # 交互式 REPL
rclaude -p "explain this code" # 单次提问
rclaude --tui                  # 终端 UI 模式
```

## 使用

```bash
rclaude "重构这个函数"                     # 直接提问
rclaude -p "main.rs 做了什么"              # Print 模式（无后续对话）
rclaude -m claude-sonnet-4-20250514       # 指定模型
rclaude -r                                # 恢复上次会话
rclaude --permission-mode auto            # 自动批准安全工具
rclaude --max-budget-usd 1.0             # 设置费用上限
```

### 权限模式

| 模式 | 行为 |
|------|------|
| `default` | 危险工具交互式 Allow/Always/Deny 提示 |
| `auto` | 安全工具（只读）自动批准，危险工具仍需确认 |
| `bypass` | 所有工具自动批准（谨慎使用） |
| `plan` | 仅允许只读工具，禁止写操作 |

### REPL 命令

73 个斜杠命令，按类别组织：

| 分类 | 命令 |
|------|------|
| 会话 | `/compact` `/resume` `/clear` `/export` `/session` `/rewind` |
| 代码 | `/commit` `/diff` `/review` `/pr-comments` `/release-notes` |
| 模型 | `/model` `/cost` `/stats` `/usage` `/effort` |
| 配置 | `/config` `/permissions` `/memory` `/hooks` `/theme` |
| 工具 | `/mcp` `/plugin` `/skills` `/agents` `/tasks` |
| 信息 | `/help` `/doctor` `/version` `/context` `/files` |

REPL 中输入 `/help` 查看完整列表。

## 功能详情

### 工具（45 个）

| 分类 | 工具 |
|------|------|
| 文件系统 | Read、Write、Edit、Glob、Grep |
| 执行 | Bash（tree-sitter AST 安全分析）、PowerShell |
| 代码智能 | LSP 集成、Notebook 支持 |
| Agent | 子 agent 派生、内存快照、分叉 agent |
| MCP | 工具代理、资源读取、OAuth 认证 |
| Web | Search、Fetch |
| 工作流 | 任务管理、计划模式、TODO 追踪 |

### Agentic 循环

- **流式输出**，实时 token 显示
- **自动重试** — 429（限流）倒计时重试，529 指数退避
- **自动 compact** — 上下文窗口满时自动压缩，恢复最近文件、活跃计划、skills 和会话记忆
- **输出续写** — 截断时自动续写最多 3 次
- **并行工具执行** — 安全（只读）工具并行调用

### 插件与 MCP 生态

- 从 marketplace 浏览、搜索、安装插件
- MCP 服务器支持：**stdio**、**SSE**、**WebSocket** 三种传输
- OAuth (PKCE) 认证，支持远程 MCP 服务器
- 自动发现项目树中的 `.mcp.json` 配置
- 自定义 hooks（6 种事件类型），支持条件过滤

### 会话管理

- JSONL 转录持久化，消息链式关联
- 跨会话关键词搜索（文本、工具输入、工具结果）
- 会话恢复（`rclaude -r`）
- Agent 侧链隔离 — 子 agent 拥有独立转录

## 配置

### 环境变量

| 变量 | 说明 |
|------|------|
| `ANTHROPIC_API_KEY` | Anthropic API 密钥（必需） |
| `CLAUDE_MODEL` | 覆盖默认模型 |
| `ANTHROPIC_BASE_URL` | 自定义 API 端点 |
| `AWS_PROFILE` / `AWS_REGION` | AWS Bedrock 认证 |
| `CLOUD_ML_REGION` | Google Vertex AI 区域 |

### API 提供商

根据环境变量自动检测：

- **Anthropic**（默认）— 设置 `ANTHROPIC_API_KEY`
- **AWS Bedrock** — 设置 `AWS_PROFILE` 和 `AWS_REGION`
- **Google Vertex AI** — 设置 `CLOUD_ML_REGION`

## 架构

```
rclaude/                                    行数
├── src/
│   ├── main.rs                           1,020   CLI 入口、REPL、会话管理
│   └── query_engine.rs                     793   核心 agentic 循环、流式输出、工具调度
├── crates/
│   ├── rclaude-core/                     8,554   消息、配置、权限、hooks、插件
│   ├── rclaude-tools/                   10,081   45 个工具实现
│   ├── rclaude-services/                 4,898   Compact、会话存储、LSP、诊断
│   ├── rclaude-commands/                 3,485   73 个斜杠命令
│   ├── rclaude-mcp/                      1,653   MCP 客户端（stdio/SSE/WebSocket/OAuth）
│   ├── rclaude-api/                      1,222   Anthropic/Bedrock/Vertex API 客户端
│   ├── rclaude-utils/                    1,207   Git、Shell、路径、格式化工具
│   └── rclaude-tui/                        689   终端 UI（ratatui）
└── tests/                                1,014   集成测试
                                         ──────
                                         34,616   总计
```

## 环境准备

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version   # 1.75+
```

### 系统依赖

| 平台 | 命令 |
|------|------|
| Debian/Ubuntu | `sudo apt install -y pkg-config libssl-dev build-essential gcc` |
| Fedora/RHEL | `sudo dnf install -y pkg-config openssl-devel gcc` |
| macOS | `xcode-select --install` |
| Windows | [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，勾选 "C++ 生成工具" |

## 开发

```bash
cargo build                               # Debug 构建
cargo build --release                     # Release 构建
cargo test --workspace                    # 运行全部 452 个测试
cargo clippy --workspace -- -D warnings   # Lint（要求 0 warnings）
cargo fmt --check                         # 格式检查
```

## 路线图

- [ ] 插件 DXT 运行时（包安装、工具注册、配置流程）
- [ ] TUI 增强（Markdown 渲染、语法高亮、Diff 视图）
- [ ] 多 agent 协调（Swarm）
- [ ] 语音输入模式
- [ ] IDE 集成
- [ ] 远程控制（Bridge）

完整列表见 [docs/TODO.md](docs/TODO.md)。

## 贡献

欢迎贡献！请：

1. Fork 仓库并创建功能分支
2. 确保所有检查通过：`cargo check && cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --check`
3. 提交 PR 并附上清晰的变更说明

## 参考资料

基于公开可用的资源构建：

- [Anthropic Messages API 文档](https://docs.anthropic.com/en/api/messages)
- [Anthropic Claude CLI 官方文档](https://docs.anthropic.com/en/docs/claude-code)
- Claude CLI `--help` 输出及公开用户指南
- 社区讨论、博客文章及围绕 Anthropic API 的开源工具

## 许可证

[MIT](LICENSE)
