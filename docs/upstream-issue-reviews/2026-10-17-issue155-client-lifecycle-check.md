# #155：客户端长任务完成通知边界

## 结论

服务端对纯 `TaskProgress` 的协议错误已经修复并通过真实 Cursor 验收；#155 的长任务自动完成链路仍不能宣布彻底解决。当前 Cursor 3.25.6 实际使用 legacy Agent Exec，客户端日志明确记录 `cursor_agent_host gate is disabled` 和 `using legacy implementation`。

安装包同时包含一个未启用的 `cursor-agent-host` 实现。它为每个 Shell 保存独立的 `completionConversationId`，完成时即使 `trackedWork` 已被清理，仍能找到根会话；当前 legacy 实现没有这层保留。这个差异提供了调查线索，尚未证明是本次失败的原因；2026-10-18 复核发现最新实测没有排除功能开关，也没有完整请求追踪，不能将代码差异视为根因证据。

## 证据

### 当前实际拓扑

Cursor 3.25.6 的窗口日志：

```text
[CursorAgentHostEnablementService] cursor_agent_host gate is disabled, ... effective agent-host topology is disabled
[AgentHostService] using legacy implementation
```

因此，静态分析 `cursor-agent-host/dist/657.js` 不能当作本机本轮实际执行路径的 UI 通过证据；它只说明安装包中存在另一条实现。

### Legacy 与未启用 Host 的差异

当前执行文件：

```text
/Applications/Cursor.app/Contents/Resources/app/extensions/cursor-agent-exec/dist/main.js
sha256=8b181310e93d99e800f264a0eebcc26710afec5a5d2a4184070eae894de84b4c
```

Legacy 的完成路径要求 `trackedWork.get(shellId)` 存在且有 `composerId`，然后才调用 `enqueueBackgroundWakeup`；`cleanupShell` 会删除该条 `trackedWork`。`adopt` 路径还把完成通知的会话参数留空，依赖这张运行时表回填。

未启用的 Host 文件：

```text
/Applications/Cursor.app/Contents/Resources/app/extensions/cursor-agent-host/dist/657.js
sha256=7fc73318adbcb8ec0db0342c36c082b60c2a99fc58dd59eb145f96d84af663ba
```

该实现新增了 `completionConversationIds`、`rememberCompletionConversationId` 和 `completionConversationIdFor`：Shell 创建时保存根会话 ID，完成时优先使用保存的 ID，即使 `trackedWork` 已不存在也能构造完成唤醒。

### 与真实测试的关系

| 场景 | 结果 | 能说明什么 |
| --- | --- | --- |
| 2026-10-17 两个短后台 Shell（179169、179170） | 均收到 `TaskFinished`，模型自动续答，数据库各有完成 Run | 正常短任务的客户端完成链路可用 |
| 2026-10-16 20 秒显式通知 | 服务端收到 11 次纯 `TaskProgress`，均 HTTP 200；未收到 `TaskFinished` | 纯进度阻断已消失，但长任务完成边界仍未覆盖 |
| 2026-10-17 命中后退出 / 命中后继续 20 秒 | 两个真实 Shell 均退出码 0；客户端只记录匹配，没有自动恢复 Run | legacy 完成通知生命周期仍未通过；详见 [补充复测](2026-10-17-issue155-legacy-completion-check.md) |
| 2026-10-16 45 秒后台对照 | 进程正常退出；后台组未自动续答 | 原 #155 的长任务自动完成仍未完全复现/解释 |

纯进度通知现在返回成功空结束帧，不启动模型 Run，也不再显示 `Error resuming chat`。重复的相同 `TaskProgress` 说明客户端仍在重放未完成的进度唤醒；这不是供应商连通性问题，也不是新的模型 Run 重试。

## 当前状态与下一步

- **已验证**：服务端忽略纯 `TaskProgress`，不会报协议错误、不会重复执行 Shell、不会唤醒模型。
- **未完成**：legacy Cursor 在长任务、接管或 `trackedWork` 生命周期跨越场景下，是否能及时发出 `TaskFinished` 并触发模型续答；本轮真实 Shell 已复现“命中通知但没有自动恢复 Run”。
- **不应做**：为了掩盖客户端缺少完成通知而把进度伪装成完成，或把空响应误报成自动续答成功。
- **下一步（2026-10-18 更正）**：先核实当前窗口 Bootstrap 身份和 `long_running_jobs` 的实际运行值，再抓取真实通知请求及响应。只有排除配置、协议等因素后，才进一步验证 legacy 生命周期；Agent Host 对照尚未完成。

本轮没有修改生产代码、没有提交、合并、发布或评论上游；仅补充证据并修正本地清单状态。

## 环境收尾

已删除本轮正式数据库中的测试会话 `7adba622-8ca1-4e59-a9b5-a35c674f1eaa`：20 条消息、4 个 Run、6 次模型调用、2 个工具轮次及 15 个检查点；SQLite 外键检查为 0。临时数据库备份已删除。服务已恢复为 `49588`，Cursor 代理为 `49642`，集成状态为 enabled。
