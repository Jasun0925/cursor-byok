# #155：服务端长任务边界复核

> 2026-10-18 更正：下文不能把剩余边界仅归于客户端。已另行确认服务端成功空流缺少 TurnEnded，见 [流结束修复](2026-10-18-issue155-stream-end-fix.md)。20 分钟 Shell 计时器的代码检查结论不变。

## 结论

2026-10-17 对当前分支的 Shell 编码、后台通知入口和进程级超时做了定向复核。没有发现一个会在约 20 分钟主动终止本地 Shell 的 BYOK 服务端计时器；本轮不修改生产代码。

现版 Shell 请求明确使用后台模式，硬超时为 24 小时。服务端供应商请求默认超时为 1 小时、流空闲超时为 30 分钟，这两个计时器只约束模型供应商连接，不约束已经交给 Cursor Agent Exec 管理的本地 Shell。

因此，当前 #155 的剩余边界仍在 Cursor 3.25.6 的 legacy Agent Exec：长任务命中 `notify_on_output` 后，客户端能够发出 `TaskProgress`，但现有真实记录没有看到随后自动发出的 `TaskFinished`。服务端已能安全忽略单独的进度通知；把进度伪装成完成会错误地启动模型恢复，不能作为修复。

## 代码路径

```text
server/src/cursor/tools/codec/request.rs
  Shell → timeout_behavior=BACKGROUND
         hard_timeout=86_400_000 ms
         conversation_id=<当前会话>

server/src/config.rs
  provider_request_timeout     默认 3_600 s
  provider_stream_idle_timeout 默认 1_800 s

Cursor Agent Exec legacy
  Shell 输出匹配 → TaskProgress → BYOK
  进程退出       → 应由客户端发送 TaskFinished → BYOK
```

本机安装包的 legacy 实现只在 `trackedWork` 仍有 `composerId` 时构造完成通知；未启用的 `cursor-agent-host` 另有 `completionConversationId` 保存。当前日志确认实际拓扑是 legacy：`cursor_agent_host gate is disabled`、`using legacy implementation`。

## 定向验证

以下测试均针对当前工作树运行：

| 测试 | 结果 |
| --- | --- |
| `background_progress_notification_is_ignored_without_model_resume` | 通过 |
| `shell_uses_background_timeout_and_preserves_stream_identity` | 通过 |
| `provider_timeout_defaults_match_runtime_boundaries` | 通过 |

测试证明服务端不会因纯 `TaskProgress` 创建模型 Run，也会把 Shell 请求编码为后台模式并保留流身份；它们不能证明 legacy 客户端会在 20 分钟后发送 `TaskFinished`。

## 状态

- 服务端纯进度协议阻断：已验证修复。
- 服务端 20 分钟 Shell 截断：当前代码未发现，未复现。
- legacy 客户端长任务完成通知：仍待真实 15–20 秒/20 分钟边界或启用 Agent Host 后对照。
- 本轮没有修改生产代码、提交、合并、发布或评论上游。

完整结构化摘要见 [server-boundary-evidence.json](2026-10-17-issue155-server-boundary-evidence.json)。
