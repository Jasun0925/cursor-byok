# #155：legacy Agent Exec 完成通知复测

## 结论

2026-10-17 在 Cursor 3.25.6 的真实 legacy Agent Exec 下完成两组后台 Shell 复测。两组命令都按预期退出，`notify_on_output` 都在匹配标记出现时被客户端记录；但 Cursor 没有在命令结束后自动创建新的模型恢复 Run，界面只保留“命令正在后台运行”的原始回复。

2026-10-18 证据复核更正：上述结果只证明“命令结束后没有自动续答”。本轮没有抓取完整后台通知请求，且未重新核实当前窗口 `long_running_jobs` 的运行值，因此不能据此确定是客户端没有发送 `TaskFinished`，也不能确定根因是 `trackedWork` 生命周期。2026-10-16 的配置对照已经证明功能开关会影响自动恢复，下一步应先排除这个变量。本轮没有修改生产代码。

## 复测输入

两组测试都在同一个空白 `No Repo` 对话中执行，参数为：

```text
block_until_ms=1000
notify_on_output.pattern=PROBE_DONE
notify_on_output.debounce_ms=5000
```

| 组别 | Shell | 结果 | Cursor 行为 | BYOK 数据库 |
| --- | --- | --- | --- | --- |
| A：命中即退出 | `658689` | `PROBE_START` → 等待 15 秒 → `PROBE_DONE`，退出码 0，耗时 18.068 秒 | 日志在 15.324 秒记录匹配；没有自动恢复回复 | 仅有初始 Root Run `63a2d922-0d0c-49be-bb14-3c7803d082e2:1a07ec00`，没有新的完成 Run |
| B：命中后继续 | `658690` | `PROBE_START` → 5 秒后 `PROBE_DONE` → 再运行 20 秒 → `PROBE_EXIT`，退出码 0，耗时 29.231 秒 | 日志在 5.141 秒记录匹配；命令结束后仍没有自动恢复回复 | 仅有初始 Root Run `bf023410-1bba-4816-a236-23bdb8166455:ae43e935`，没有新的完成 Run |

Shell 终端文件：

- `658689`：`~/.cursor/projects/var-folders-94-7x3w4znd6fl1k54t41jbw46m0000gn-T-12034b2e-b456-43e5-addb-563ed11bc244/terminals/658689.txt`
- `658690`：`~/.cursor/projects/var-folders-94-7x3w4znd6fl1k54t41jbw46m0000gn-T-12034b2e-b456-43e5-addb-563ed11bc244/terminals/658690.txt`

## 客户端证据

Cursor Agent Exec 日志只出现两条匹配记录：

```text
2026-10-17 21:36:50.190 agent.shell_output_notification_match
  shell_id=658689 ... first_match_ms=15324
2026-10-17 22:43:53.584 agent.shell_output_notification_match
  shell_id=658690 ... first_match_ms=5141
```

窗口启动日志仍明确为：

```text
[CursorAgentHostEnablementService] cursor_agent_host gate is disabled
[AgentHostService] using legacy implementation
```

两组 UI 对话在命令完成后都没有新增模型文本或新的工具轮次；只显示初始助手回复“已按指定参数执行一次 Shell，命令正在后台运行，已设置 `PROBE_DONE` 输出通知”。

## 服务端证据

对 `~/.cursor-byok-v3/cursor-byok.db` 的只读核对显示：

- A、B 各只有一个 `root` Run，状态为 `completed`；没有由后台完成通知创建的第二个 Run。
- B 开始时出现的三条 `cursor-root:*` 消息分别是 A 的原始 Shell 调用、返回“running in background”的工具结果和原始助手回复，属于旧历史重新导入。它们不是完成通知；此前将其写成“Shell 完成投影”有误，已于 2026-10-18 更正。
- 本会话没有 `runtime:background-completed:*` 消息，`cursor_run_traces` 中也没有该会话的记录。没有新 Run 或完成消息不能证明网络上从未收到通知，仍需补抓真实请求。
- B 的进程已经写出 `PROBE_EXIT` 并以退出码 0 结束，但仍没有新的完成 Run。

完整结构化记录见 [2026-10-17-issue155-legacy-completion-evidence.json](2026-10-17-issue155-legacy-completion-evidence.json)。

## 状态

- 服务端纯 `TaskProgress` 协议错误：已修复并通过真实 Cursor 验收。
- 服务端 20 分钟主动截断本地 Shell：当前代码未发现，未复现。
- 后台自动续答：两组均未通过；原因待区分功能配置、客户端转发和服务端接收处理。
- 下一步：在隔离环境确认 Bootstrap 身份匹配和 `long_running_jobs` 实际运行值，抓取完整 `TaskProgress` / `TaskFinished` 请求及响应；再依断点决定修复 BYOK 配置、协议处理或向 Cursor 官方报告客户端问题。Agent Host 仅是后续对照候选，尚未证明可解决本问题。

本轮只补充实机证据，没有提交、合并、发布或评论上游；测试会话和终端文件暂保留用于复核，没有做破坏性清理。
