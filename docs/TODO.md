# rclaude TODO

> 待实现功能，按优先级排序

## P1: 待实现命令

| 优先级 | 命令 | 说明 |
|--------|------|------|
| 中 | extra-usage | 额外用量查看 |
| 中 | rate-limit-options | 限流选项配置 |
| 中 | oauth-refresh | OAuth token 刷新 |
| 中 | reload-plugins | 运行时插件重载 |
| 低 | bridge | 远程控制 (JWT+WebSocket) |
| 低 | voice | 语音输入模式 |
| 低 | teleport | 环境迁移 |
| 低 | remote-setup / remote-env | 远程环境配置 |
| 低 | thinkback / thinkback-play | 思考过程回放 |
| 低 | assistant | 助手模式 |
| 低 | passes | 多轮自动执行 |
| 低 | break-cache | 缓存失效控制 |

## P2: 生态扩展

### 插件 DXT 运行时
- [ ] DXT 包解压 + manifest 验证
- [ ] 插件工具注册到 Tool 系统
- [ ] 插件 commands/agents/skills/hooks 组件加载
- [ ] 插件 userConfig 配置流程

### Git 命令深化
- [ ] AI commit message 生成
- [ ] 代码审查增强
- [ ] PR 创建流程

### TUI 增强
- [ ] Markdown 终端渲染 + 语法高亮
- [ ] Diff 可视化
- [ ] 文件路径自动补全
- [ ] 主题系统

## P3: 高级功能

- [ ] 多 agent 协调 (Swarm)
- [ ] PowerShell 工具完整实现 (Windows)
- [ ] 语音 / IDE 集成
- [ ] 远程控制 (Bridge)
- [ ] Coordinator 模式
