# #155：后台通知与前台等待对照

后续复测：临时对齐配置身份并开启 `long_running_jobs` 后，Cursor 已自动恢复原会话，但被服务端以 `background task notification contains no finished task` 拒绝。见 [配置与恢复请求复测](2026-10-16-issue155-gate-check.md)。下文保留三组初始对照的结果。

2026-10-16，延续 [21 分钟测试](2026-10-15-issue155-check.md)。结论：**45 秒前台命令正常结束并自动续答；两组 45 秒后台命令均未自动续答。显式登记输出通知后，Cursor 确实匹配到结束标记，但仍没有通知请求到达 BYOK。故障范围缩小到客户端通知转发/会话唤醒，具体原因尚未确认。**

本轮只测试和记录，不修改生产代码、不提交、不发版。未重新查询上游状态；上次查询为 2026-10-15 OPEN。

## 环境与方法

- 代码基线：`a7ae78ab1d064206fdbb5aeb0659bd48e3dadb7d`（v1.0.15）。使用与上轮 SHA-256 相同的服务端二进制。
- Cursor 3.24.18，界面 gpt-6 High，真实供应商调用模型 `gpt-6-astra`。
- 独立 SQLite 副本、隔离 Git 工作区，服务端/代理端口 57573 / 57584；未变更 Run Everything 审批模式。
- 两个新会话，各运行一次相同 Python 脚本，只有 case 标签和通知参数不同。脚本打印 START，等待 45 秒，打印 DONE 并退出。
- 两组均使用 `block_until_ms=1000`，要求后台化后结束初始回复，禁止轮询/重启。选择 45 秒是为了让脚本退出前，初始回复和连接有充分时间结束。
- 通过真实 Cursor 点击发送、真实模型选择工具；服务端请求、工具参数、Connect/protobuf 字节、客户端日志和终端退出记录分别核对。没有人工注入完成通知。

## 结果

| 检查项 | A：不设置通知 | B：显式 `notify_on_output` |
| --- | --- | --- |
| 命令 | `python3 -u probe.py default` | `python3 -u probe.py notify` |
| 输出通知参数 | 无 | pattern=`PROBE_DONE`，reason=`probe completion`，debounce_ms=5000 |
| 脚本实测耗时 | 45.006 秒 | 45.005 秒 |
| Cursor 终端总耗时（含启动） | 48.797 秒 | 48.424 秒 |
| 退出码 | 0 | 0 |
| 初始模型回复 | `CASE_A_STARTED` | `CASE_B_STARTED` |
| 输出匹配证据 | 未配置 | 客户端日志 `outcome=matched` |
| 退出后的观察时长 | 427.751 秒 | 374.914 秒 |
| 自动通知请求/模型续轮 | 无 | 无 |

总计 **2 个 run、4 次真实模型调用、2 次 Shell**，没有重复执行、额外 Read 或自动续轮。初始模型回复均发生在脚本结束前；最终两次 run 均为 completed。最后采样时间为 2026-10-16 00:29:52（Asia/Shanghai）。

## C：前台等待补测

解锁后在新的隔离数据库副本和工作区中完成，保持相同二进制、Cursor 版本、模型和 45 秒脚本；只改变工作区和 case 标签，以及 `block_until_ms=60000`，不传 `notify_on_output`。

| 检查项 | 实测结果 |
| --- | --- |
| 命令 | `python3 -u probe.py foreground`，仅执行一次 |
| 脚本耗时 | 45.006 秒 |
| 实际客户端事件 | Start → Stdout START → Stdout DONE → Exit；没有 Backgrounded |
| 退出状态 | 协议 `code=0`、`aborted=false` |
| 执行耗时字段 | Exit 的 `local_execution_time_ms=47111`；外层 ExecClientMessage 为 49096，两者均含不同范围的执行开销，不等同于脚本计时 |
| 工具完成后再次调用模型 | 276 毫秒 |
| 工具完成后保存最终回复 | 6.894 秒 |
| 最终界面 | `CASE_C_COMPLETED`，报告 45.006 秒 |
| 调用数量 | 1 个 run、2 次真实模型调用、1 次 Shell；没有人工追问或额外轮询工具 |

服务端与真实界面均确认自动续答成功。此结果证明本次前台工具返回后的模型续轮有效，不能推广为所有命令、时长与审批场景都正常。三组短命令合计 **3 个 run、6 次真实模型调用、3 次 Shell**。

另一个独立观察：虽然客户端协议提供退出码 0，保存给模型的 `result_content` 只有 stdout，因此模型如实回复“Exit status: Not provided in the tool result”。本次通过协议确认成功退出，不能把模型回答当作退出码证据；这一结果展示问题未阻断续答，本轮只记录。

前台补测证据单独保存在 [foreground-evidence.json](2026-10-16-issue155-foreground-evidence.json)，未覆盖之前两组后台证据。

## 已定位的边界

```text
模型生成 Shell 参数
  → BYOK 编码并发送 ShellStreamArgs             已确认
  → Cursor 启动命令并转后台                    已确认
  → 命令打印 PROBE_DONE 并正常退出              两组均确认
  → Cursor 输出通知观察器匹配 PROBE_DONE         B 组确认
  → 客户端通知转发、会话唤醒                   待定位具体阻断条件
  → BYOK 接收新的完成 Action                   未收到
  → 模型继续并显示完成结果                     未发生
```

B 组客户端日志（00:23:37.247）：`agent.shell_output_notification_match`，`outcome=matched`，`shell_id=325158`，`first_match_ms=45365`，tool_call_id 与 BYOK 发出的请求一致。这排除了本次“模型没传参数”或“结束标记没匹配上”的解释。

读取当前安装包的 `cursor-agent-exec/dist/main.js` 发现：

1. 完成通知及输出通知的转发均检查 `long_running_jobs` 功能开关；开关为 false 时不转发。**本轮没有读取该开关的实际运行值，不能据此断言它被关闭。**
2. 转发还要求 `trackedWork` 中能找到对应的 `composerId`。日志中的 `conversation_id=local` 不足以证明错绑：代码有将事件重新绑定到所属会话的适配器。实际绑定值尚未验证。
3. 后续应检查这两个实际条件及通知队列，而非直接调整命令超时或判断供应商卡住。

## 独立发现：通知间隔单位

本轮捕获的 BYOK 协议把 `debounce_ms=5000` 原值写为 protobuf `debounce=5000`。安装包中的官方工具构造器会先将毫秒除以 1000，客户端观察器再将协议值乘以 1000。因此存在独立的时间单位不一致，影响后续匹配通知的合并间隔。

**不把它当成本次不续答的已证实根因**：客户端第一次匹配会立即登记通知，且本次已有 matched 日志。该差异仅记录为待定向验证项，本轮没有修复。

## 限制与收尾

- 补充前台等待对照最初因锁屏未发送；用户解锁后已完成上述 C 组，01:22:36 采样确认 run 完成。
- 真实审批等待、长时间完全无输出、21 分钟显式通知均未覆盖。
- 原 #155 的“命令超时终止”未复现；当前确认的是测试场景下后台结束不自动续答，不能直接等同于完整复现原报告，更不能标记已修复。
- 已保存 [后台脱敏证据](2026-10-16-issue155-notification-evidence.json) 与 [前台脱敏证据](2026-10-16-issue155-foreground-evidence.json)：请求参数、解码后的实际 Shell 协议、脚本事件、客户端日志和最终回复。
- 两轮隔离服务均已退出、端口已释放，恢复六个代理键并保留无关设置，临时数据库及工作区已删除。正式数据库 SHA-256 与测试前一致；详情见两份证据的 `cleanup` 字段。没有修改审批模式，C 组沿用已有 unrestricted 配置，未验证真实人工审批等待。
