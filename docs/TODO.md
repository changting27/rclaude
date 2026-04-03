# rclaude TODO

> Pending features, ordered by priority

## P1: Missing Commands

| Priority | Command | Description |
|----------|---------|-------------|
| Medium | extra-usage | Extended usage analytics |
| Medium | rate-limit-options | Rate limit configuration |
| Medium | reload-plugins | Runtime plugin reload |
| Low | bridge | Remote control (JWT+WebSocket) |
| Low | voice | Voice input mode |
| Low | teleport | Environment migration |
| Low | remote-setup / remote-env | Remote environment setup |
| Low | thinkback / thinkback-play | Thinking process replay |
| Low | assistant | Assistant mode |
| Low | passes | Multi-pass auto execution |
| Low | break-cache | Cache invalidation control |

## P2: Ecosystem

### Plugin DXT Runtime
- [x] DXT manifest parsing and discovery
- [ ] Plugin tool registration into Tool system
- [ ] Plugin commands/agents/skills/hooks component loading
- [ ] Plugin userConfig configuration flow

### TUI Enhancements
- [x] Basic markdown rendering (headers, code blocks, bold/italic)
- [ ] Syntax highlighting (tree-sitter based)
- [ ] Diff visualization
- [ ] File path autocomplete

## P3: Advanced Features

- [ ] Multi-agent coordination (Swarm)
- [ ] Voice input mode
- [ ] IDE integration (VS Code / JetBrains plugin protocol)
- [ ] Remote control (Bridge)
- [ ] Coordinator mode
