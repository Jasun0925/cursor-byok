# #155：后台通知成功流缺少 TurnEnded

## 结论

2026-10-18 确认并修复一个服务端协议缺口：纯进度通知、重复完成通知，以及已转交活动 Run 的完成通知，原先仅以 Connect 成功结束帧关闭请求，没有发送 `InteractionUpdate.TurnEnded`。Cursor 3.25.6 会把这种响应识别为可重试的连接中断。

此前 2026-10-16 的“11 次 HTTP 200、无 Error resuming chat”不足以认定客户端接收成功。那 11 次请求都返回 7 字节 `02000000027B7D`，正好缺少客户端要求的结束事件；它们与客户端在同一次恢复中反复重试的路径吻合。没有当时完整客户端重试日志，因此不把这一关联写成每次重试原因均已实机确认。

本轮已完成代码修复和协议验证。随后在自动更新后的 Cursor 3.25.9 上完成两组实测，后台命令均结束但仍未自动续答，且没有通知请求到达隔离服务；因此本补丁尚未通过完整实机验收。既有“约 20 分钟命令被终止”仍未复现，不把本修复等同于 #155 原报告全部解决。

## 明确的协议边界

```text
Cursor 后台输出匹配 → TaskProgress 请求
  → BYOK 判定无需调用模型
  → 修复前：Connect EOF {}
  → Cursor：未见 turnEnded → 报可重试连接中断
  → 原恢复请求可能持续占用客户端唤醒流程

修复后：BYOK 判定无需调用模型
  → InteractionUpdate.TurnEnded（无用量）
  → Connect EOF {}
  → 客户端流结束检查接受

稍后 TaskFinished → 原有完成投影 → 持久化事件 → 模型继续
```

`TurnEnded` 表示本次通知请求处理结束，不表示后台进程退出。修复保留 `TaskProgress` 与 `TaskFinished` 的区别，不把进度通知投影成任务完成，也不主动执行或轮询 Shell。

## 代码范围

```text
server/
├── src/cursor/conversation/
│   ├── command.rs                 # 区分已有模型轮次结束与仅确认通知
│   └── runtime.rs                 # 仅确认通知时先发 TurnEnded，再关闭流
└── tests/conversation_delivery.rs # 验证结束事件、去重、历史与后续完成恢复
docs/upstream-issue-reviews/
├── 2026-10-18-issue155-stream-probe.mjs       # 运行安装包原有流检查函数
└── 2026-10-18-issue155-stream-end-evidence.json # 脱敏验证结果
```

没有移动、合并或删除模块。通知确认在原有 generation 检查及排空流程之后发送，避免过时 generation 关闭新请求；不发布请求携带的旧 checkpoint，也不新增模型 Run、模拟用户消息或供应商历史。

## 复现与验证

先加强已有传输测试：成功关闭通知请求之前，必须恰好有一个 `TurnEnded`。修复前纯进度测试失败，实际计数为 0；修复后后台通知套件 11 项通过。另补充“进度确认后，同一任务完成仍能调用模型”以及“仅确认通知不能覆盖 checkpoint”的断言。

直接从本机安装包提取并执行原有流检查函数，只替换外部日志、队列和错误类依赖，不编辑 Cursor 安装包：

| 输入 | 实际流检查结果 |
| --- | --- |
| 无消息，成功结束 | 拒绝：`Stream ended without turnEnded`，`isTransport=true` |
| 只有 checkpoint，成功结束 | 同样拒绝 |
| `turnEnded` 后成功结束 | 接受 |

这项验证使用真实安装包中的检查逻辑和模拟 I/O，不能当作真实 UI 自动完成验收。

全部 24 个集成套件、176 项测试通过；`cargo clippy -p cursor-server --locked --lib -- -D warnings`、格式检查与 `git diff --check` 通过。完整结果见配套 JSON。本轮单元测试仍有此前已记录的 `provider::attempt::tests::request_transport_failure_is_one_failed_attempt` 失败：184/185 通过，失败断言为 `matches!(error, Error::Http(_))`。该模块未修改，另行运行所有集成测试，避免单元测试失败提前跳过集成套件。

## 环境与待验收项

- Cursor 仍为 3.25.6。当前启动日志明确使用 legacy 实现；缓存中的 `cursor_agent_host=true` 不能视作启用证据。
- 当前缓存（2026-10-18 01:39:12 +08:00）中 `long_running_jobs=true`、`nal_agent_retries=true`，但本轮没有取得当前窗口的实际功能诊断。
- CUA 读取窗口连续返回 macOS ScreenCaptureKit `-3811`，重置连接后仍失败，因此没有发送新的 UI 测试提示，没有抓到新的自动通知请求。
- 后续在隔离数据库与端口中确认实际功能开关、后台通知请求与响应，验证进度请求不再重试、完成通知自动续答、Shell 只执行一次。
- 本轮未修改 Cursor 设置、安装包、正式数据库或运行中的服务；未提交、合并、发布或评论上游。


## 同日实机接续

用户解锁后已从 IDE 执行 `Developer: Dump Feature Gates`，2026-10-18 13:59:04 +08:00 的实际诊断中 `long_running_jobs`、`nal_agent_retries`、`cursor_agent_host` 均为 enabled。该结果来自同一安装实例的 IDE 窗口，测试 Agent 窗口尚未发起任务，不能据此宣称新 Agent Host 拓扑已实测。

隔离服务已构建、独立数据库已通过 SQLite backup 创建，并从启动前开启详细记录；实验 Bootstrap 身份匹配成功。切换代理后 Cursor 重启出现登录页，现有令牌过期、账号接口返回 401，因此没有产生新的模型 Run 或通知追踪。已恢复正式服务与六个代理键，正式数据库在恢复时与基线一致。

随后通过浏览器中的原账号执行官方登录，网页显示 All set，但客户端令牌尚未刷新，不能认定客户端已恢复登录。返回桌面时 CUA 明确报告 Mac 再次锁定。后续先验证官方登录回调是否完成，再继续两组后台任务验收。临时服务已停止，隔离目录以私有权限保留用于接续。完整记录见 [实机接续证据](2026-10-18-issue155-live-check-evidence.json)。

## 3.25.9 实机结果（本次接续）

Cursor 重启后实际版本变为 3.25.9。两组请求均在独立端口 57573/57584、独立 test.db 上处理，日志确认实际使用 Agent Host 的 connect 运行路径，不能继续归因于“只发生在 3.25.6 legacy”。初始轮均成功，客户端记录 retries_attempted=0；这不代表后台通知轮通过。

| 检查 | 普通后台 | 输出监控后台 |
| --- | --- | --- |
| Shell 执行次数 | 1 | 1 |
| 前台等待 | 1000 ms | 1000 ms |
| Cursor 返回 backgrounded | 是 | 是 |
| ShellArgs 包含 conversation_id | 是 | 是 |
| notify_on_output | 未设置 | pattern=MATCH，debounce=5000 ms |
| 进程持续时间 | 25.008 s | 25.009 s |
| 终端退出码 | 0 | 0 |
| 初始回复 | CASE_PLAIN_STARTED | NOTIFYSTARTED |
| 后台通知请求到达 BYOK | 0 | 0 |
| 退出后观察时间 | 至少 1303 s | 至少 995 s |
| 自动续答 | 未发生 | 未发生 |

```text
BYOK 发 Shell（携带会话 ID）
  → Cursor 返回 backgrounded
  → 初始模型轮正常结束（含 TurnEnded）
  → 本地命令完成，终端 exit_code=0
  → [尚未证实：运行时开关 / 客户端通知生成及转发]
  → BYOK 未收到 TaskProgress 或 TaskFinished
  → 未创建自动恢复模型轮
```

当前 Bootstrap 缓存中 long_running_jobs=true，但本次未取得测试窗口的运行时诊断；此前 13:59 的 IDE dump 不能代替新版本新窗口。监控参数已通过协议解码确认送达，但当前日志没有记录匹配通知，不能宣称客户端已生成并转发。两组结果定位了请求到达服务端之前的边界，尚不足以在功能开关与客户端通知逻辑之间确定唯一原因。

已重新运行 3.25.9 安装包原有流检查函数：空成功流和 checkpoint-only 仍被拒绝，TurnEnded 后成功结束被接受。probe 脚本的外部绑定名已适配 3.25.9，旧版验证哈希仍保留在原证据中。由于本轮没有收到通知请求，新增通知确认分支未被真实 UI 流量覆盖；这与初始模型轮含 TurnEnded 是两件事。

测试过程中界面捕获反复出现 -3811/-3812；重启后完成第二组，但 IDE 命令面板未能正常打开，运行时 gate dump 保持未验证。完整结果见 [实机证据](2026-10-18-issue155-live-check-evidence.json)。恢复正式服务、代理键与实验前 Bootstrap；正式数据库哈希仍与基线一致，测试请求不在正式数据库中。未修改 Cursor 安装包、未提交或发布。

用户再次确认置前后，查看菜单可读，但选择命令面板再次返回 ScreenCaptureKit -3811；未得到新的运行时开关证据。下一步需要在可稳定操作的测试窗口中读取 long_running_jobs，并观察 enqueueBackgroundWakeup 到 Agent Run 的转发；不能仅凭安装包中的清理逻辑认定此次丢失通知的原因。

## 运行时开关诊断与收尾

用户随后在 IDE 执行诊断命令。`feature-gates-2026-10-18T08-12-35-172Z.json`（本地 16:12:35）确认当时 `long_running_jobs`、`nal_agent_retries`、`cursor_agent_host`、`background_nudge_2`、`terminal_execution_service_2` 均为 enabled，且没有 override。该诊断确认了这个 IDE 进程的开关值，不能追溯证明前面两组 Agent 测试的运行时状态。

为保持同一 Cursor 生命周期，暂停正式服务后，隔离数据库和服务端口 57573 临时使用原代理端口 49642；但中断期间 Cursor 从 PID 30162 重启为 34380，同进程对照条件已失效。新探针没有执行：没有事件文件，也没有新增请求追踪。界面捕获继续失败，用户暂时无法手动操作，因此这组记录为“尚未执行”，不计为第三次失败或通过。

已停止隔离服务、释放 57573，恢复六个代理设置和实验前持久化 Bootstrap，并恢复正式服务 49588 / 代理 49642。正式状态为 enabled、settings_applied=true，正式数据库哈希与初始基线一致。恢复时发现 Cursor 已退出，随后通过 CUA 重新打开并确认进程存在；没有验证重启后运行时 Bootstrap 内容。测试目录继续保留。未修改 Cursor 安装包、未提交或发布。

剩余结论仍为：前两组后台进程正常完成，但通知未到达隔离 BYOK。新增服务端通知确认补丁尚未被真实通知流量覆盖；下一次验收需要在同一客户端生命周期内关联实际开关、通知生成、转发和服务端接收记录。

同日继续修复了此前只存在于实验版本中的 Bootstrap 身份与后台开关问题，新增回归及独立 HTTP 验证通过；真实自动续答仍待验收。见 [配置修复记录](2026-10-18-issue155-bootstrap-fix.md)。
