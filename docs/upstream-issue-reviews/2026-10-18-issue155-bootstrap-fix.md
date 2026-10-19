# #155：补齐后台任务配置与身份绑定

2026-10-18 接续排查。**新增修复 BYOK Bootstrap 的两个确定缺口：没有保证 `long_running_jobs` 开启；上游配置获取失败时，本地响应始终使用 `local_ultra`，与当前登录身份不一致。代码回归与独立 HTTP 验证通过；同日接续又修复历史选择错误，25 秒 notify_on_output 实机自动续答已通过，详见文末最新记录。**

本轮查询上游 [#155](https://github.com/leookun/cursor-byok/issues/155) 仍为 OPEN；其人工审批超时和约 20 分钟卡住的原报告，没有因此被判定全部修复。

## 证据与改动

[10 月 16 日身份/开关对照](2026-10-16-issue155-gate-check.md) 已观察到配置身份不一致时，缓存中的开关与运行值不同；身份对齐并启用后台任务后，Cursor 发出了恢复请求。这部分当时只修改实验二进制，正式源码尚未修复。

本机 Cursor 3.25.9 的 `ExperimentRefreshService` 从访问令牌读取 `sub`，把它作为 Statsig 查询用户；配置数据则按 Bootstrap 响应中的 `user` 存储。固定的 `local_ultra` 无法满足该身份一致性条件。

```text
server/src/cursor/services/
├── analytics.rs        # 按请求身份生成本地配置，保证所需后台功能开关
└── analytics/tests.rs  # 配置身份、哈希格式、协议编码与配置保留回归

BootstrapStatsig 请求
  → 保留请求令牌中的 sub，仅用于配置身份
  → 上游有效响应：保留其余配置，启用 long_running_jobs / nal_agent_retries
  → 上游不可用：以请求 sub 生成本地配置，启用同样两个开关
  → Cursor 按当前身份读取配置
```

本地响应移除固定的 `userID`、`customIDs.localUserID` 和 `sdkParams.stableID`。没有可读身份时返回空 `user`，不另造账号。只读取 `sub`；不复制邮箱等其他声明，不将未验证声明用于认证或权益判断，不记录令牌或身份原文。上游成功响应保留原身份、其他开关、动态配置和层配置。

本次没有修改会话投影、历史、数据库结构或 Cursor 安装文件，也没有增加其他客户端实现路径。先前的 `TurnEnded` 修复保留。

## 验证

- 先添加 3 个测试：修复前全部失败，分别复现后台开关保持关闭、请求身份被替换为 `local_ultra`、缺失身份时仍制造固定账号。
- 修复后 3 个测试通过，覆盖两个请求身份、本地账号身份、无效/缺失身份，以及 none / djb2 / sha256 三种配置键格式、普通 protobuf / Connect 帧两种编码。
- `cargo test -p cursor-server --locked --lib cursor::services:: -- --nocapture`：30 项通过。
- `cargo test -p cursor-server --locked --test conversation_delivery`：11 项通过，包含进度确认、后续完成恢复和重复通知去重。
- `cargo clippy -p cursor-server --locked --lib -- -D warnings`、改动文件的 rustfmt 检查、`git diff --check` 均通过。
- 独立端口 57673、独立数据库上的 HTTP 探针使用同一份生产 analytics 代码，覆盖两个合成身份和无身份请求，均为 HTTP 200、身份正确、两个开关启用、响应长度正确。通过无效上游 URL 触发已有本地生成路径，不向外部发送合成凭据。

HTTP 探针的入口只挂载 analytics handler，没有启动桌面代理管理逻辑。它证明真实 HTTP 响应正确，不能代替 Cursor 自动通知验收。

## 实机边界与收尾

CUA 能读取 Cursor Agents 窗口，但新建对话操作返回 ScreenCaptureKit `-3812`；此前登录入口操作返回 `-3811`。本轮没有发送新的实机测试请求、没有运行新的后台探针。

**此前 3.25.9 两组失败测试用的实验 Bootstrap 已包含匹配身份和 `long_running_jobs=true`，但缺少同窗口、同生命周期的运行值证据。因此不能声称本次配置补丁已解释或解决那两次通知未送达。** 最新状态仍是：配置缺口和通知响应协议缺口已修复，真实自动续答待验收。

已停止 HTTP 探针并释放端口。正式服务保持 enabled、settings_applied=true，正式数据库及 Cursor 设置文件哈希与本轮开始一致；没有替换正式二进制、没有提交或发布。脱敏结果见 [结构化证据](2026-10-18-issue155-bootstrap-evidence.json)。

## 17:00 后实机验收接续（当时等待发送测试）

用户要求测试后，已构建完整隔离服务，包含相同 analytics 修复及通知流结束修复。服务使用独立数据库和端口 57573，暂停正式服务后复用原代理端口 49642；仅在实验源码中跳过代理启动时自动终止 Cursor 的动作，以保留同一客户端生命周期。Cursor PID 53439 在切换前后保持一致。

已观察到 Cursor 持久化 Bootstrap 更新：djb2 格式下 `long_running_jobs`、`nal_agent_retries` 均为 true / local_enabled，其他上游动态配置仍存在。该结果验证了补丁返回的配置已进入客户端缓存，仍不能替代运行时 dump 或自动续答结果。

界面工具 getAXState/getApp 持续报 ScreenCaptureKit `-3811`，已请求用户手动执行诊断并在新对话发送一次 25 秒后台探针。截至本次记录，没有新增 Run、进程事件或运行时 dump。测试服务暂时保持就绪，正式服务暂停；待测试完成恢复。当前状态见 [本轮实机证据](2026-10-18-issue155-bootstrap-live-evidence.json)。

## 17:30 最新结果

已打开专用验证工作区并完成实机测试：17:18 的 TaskProgress / TaskFinished 均到达服务端，进一步定位了待投递消息切到空历史的错误；修复后 17:30 新 BYOK 会话自动显示 `HISTORY_FIXED_COMPLETED`，命令只执行一次、正常退出。全部 178 项集成测试通过，正式代理和设置已恢复。短任务自动续答已通过，约 20 分钟及审批超时仍未验收；见 [最新修复与完整边界](2026-10-18-issue155-history-fix.md)。
