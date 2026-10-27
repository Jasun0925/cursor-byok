# 上游修复回复与投递记录 · 2026-10-27

用户已明确授权：对确认解决的 Issue/PR 评论具体结论，并提供维护 fork 链接。没有授权冒充原维护者或宣称未验证问题已解决。

## 实际投递

- 目标：[PR #423](https://github.com/leookun/cursor-byok/pull/423)。发送前读取全部现有评论，没有本账号重复回复。
- 基线：`main=818eeac`；`89d90c6` 为 `v1.0.17` 的祖先；本轮 8 项 `anthropic_refusal` 测试通过，历史 UI 证据仍保留。
- API：`POST /repos/leookun/cursor-byok/issues/423/comments`。
- 结果：HTTP 403，`Repository was archived so is read-only.`；没有评论 URL，没有成功发布。
- 同一仓库的其他候选不再重复尝试写入。未联系其他用户、未在无关仓库推广、未修改原 Issue/PR 状态。
- fork 已开启 Issues，公开入口为 [维护状态](../maintenance-status.md)。

## PR #423：实际提交但被拒绝的正文

感谢这个 PR。我在维护的 fork 中采用了拒绝优先于工具执行的处理，并补齐服务端停止重试与 Cursor 不可重试错误标记；已随 v1.0.5 发布，当前 v1.0.17 也包含这项修复。

验证范围：真实 Cursor 界面连接本地可控 Anthropic SSE，拒绝与工具块同时返回时仅一次模型请求、工具不执行；纯文本拒绝和正常工具调用也通过。2026-10-27 在当前 main 重新运行 8 项 HTTP/SSE、运行层及 Cursor 协议回归全部通过。这里没有把模拟响应等同于真实 Anthropic 供应商实测。

修复与测试记录：https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-pr-reviews/2026-09-30-pr423-fix-result.md
可下载版本：https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.17

我是该 fork 的维护者，原 PR 的贡献已在记录中注明。遇到同一触发场景的用户可据此验证；这不表示本 PR 已合入原仓库。

## 其他候选草稿（未发送）

### Issue #504：仅确认相关的重复交付缺口（未发送）

我维护的 fork 中独立复现了一个相关问题：同一 UserMessage.message_id 使用新传输序号再次送入活动 Run 时，服务端会重新排队并中断已经执行中的工具。补丁把消息 ID 去重放到排队之前；相同文字、不同 ID 的新消息仍正常执行。

六项协议测试覆盖初始与追加消息重传、真正的新输入、checkpoint 恢复、分片 Write 内容及缺失内容报错。后两类原报告症状在受控路径未复现，尚不能认定三种现象具有同一个根因。[修复与验证边界](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-issue504-input-redelivery-fix.md)。源码补丁尚未发布安装包；需要原环境的脱敏请求与工具记录才能进一步确认。原仓库归档只读，本段没有发送。

### Issue #340

我维护的 [fork](https://github.com/haohaomin/cursor-byok) 中已补齐后台子代理完成后的结果交付，v1.0.9 起包含，当前可下载 v1.0.17。此前使用真实 Cursor 连续两轮测试普通与 Fast：父初始 Run 结束后，子代理完成通知创建父会话的新 Run，并回复子代理结果。两次真实通知均携带工具 ID，缺省 ID 仍仅有自动化覆盖；不把此结论扩展到所有后台 Shell 审批和压缩边界。[验证记录](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-03-issue340-fix.md)。

### PR #421

感谢原补丁。我在维护的 [fork](https://github.com/haohaomin/cursor-byok) 中补齐按输出项、Unicode 后缀以及最终 completed 快照的文字恢复，v1.0.6/v1.0.7 起发布。2026-10-27 当前源码的 16 项 Responses HTTP/SSE 回归通过，历史真实 Cursor 验证见[记录](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-pr-reviews/2026-10-01-v1.0.6-release.md)。并未确认所有第三方网关的空白回复都由同一原因导致。

### Issue #449

我维护的 [fork](https://github.com/haohaomin/cursor-byok) 已移除 Task 固定模型名单，改为读取客户端模型信息，并正确解析本地模型显示名；Cursor 明确指定的子代理模型仍具有优先级。修复随 v1.0.12 发布，维护者完成过实际模型选择验收，当前两项模型上下文回归通过。[详细范围](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-08-issue449-fix.md)。

### Issue #509：仅源码补丁，不能宣传安装包已解决

当前 [fork main](https://github.com/haohaomin/cursor-byok) 已合入 #420 的完整整数约束修复，保留大整数精度和空对象/required 修复。自动化覆盖 minLength 等八个整数关键字及嵌套 schema，原 Cursor 工具场景仍待实测。**v1.0.17 安装包不包含 2026-10-19 的这次合入**，不能建议安装该版本来验证此补丁。[合入记录](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-pr-reviews/2026-10-19-branch-integration.md)。

### PR #513：数据库部分的独立修复（未发送）

感谢这个故障报告。我维护的 [fork](https://github.com/haohaomin/cursor-byok) 中复现了 SQLite 自动回滚后 SQLx 保留事务深度、后续 Store 写入报 InvalidSavePointStatement 的问题。修复放在 SQLx 连接池归还检查中：先等待排队回滚，仍有残留事务状态则关闭连接；正常回滚的健康连接继续复用，没有导入整个 WriteSession 或重复改动子代理入口。

7 项故障恢复测试通过，包括临时数据库实际 SQLITE_FULL、提交/回滚失败、取消及健康连接复用；全工作区 446 项测试通过。原机器未实测，源码补丁尚未发布安装包。[完整证据](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-pr-reviews/2026-10-27-pr513-recovery.md)。由于原仓库归档只读，本段仅保存为草稿，没有发送。

### Issue #508 / PR #474：代理缓存修复（未发送）

感谢 GGboya 对代理缓存失效的定位。我维护的 [fork](https://github.com/haohaomin/cursor-byok) 已参考该 PR 的指纹与检查间隔方案，独立补齐三种 HTTP 客户端的缓存更新，并覆盖环境变量、macOS 系统设置和 Windows 注册表读取。没有采用 PR 中无关的规则删除及正文日志。

隔离测试先复现了请求继续走旧代理与旧认证，修复后 A→B→直连→A、现有流不中断、读取失败保留缓存、代理仍启用但已停止时不直连等检查通过；本机全工作区 451 项测试通过。这里是本地 HTTP/进程环境及 macOS 配置读取验证，尚未进行 Windows/macOS 系统 UI 切换或真实供应商 CONNECT 验收。**仅源码修复，v1.0.17 安装包不包含。** [复现、实现与验证边界](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-issue508-fix.md)。原仓库归档只读，因此本段只是草稿，没有发送。

### Issue #469 / PR #498：基础路由及 WebFetch 分页（未发送）

感谢 sunrisegm 的候选方案。我维护的 fork 已用 Cursor 3.27.14、BYOK v1.0.17、macOS → Linux SSH、OpenAI Chat 模型独立复现了 `PROXY 127.0.0.1` 连接错误。按 Cursor 官方提供的设置，在目标主机的 **远程 [SSH: 主机]** 中把 `cursorAgentHost.remoteInferenceRoute` 改成 `always`，再 Reload Window、新建 BYOK 对话后，基础回复、远端 Read/Shell、文件编辑、续聊、取消后继续和窗口重载测试通过。

[配置步骤与证据](https://github.com/haohaomin/cursor-byok/blob/main/docs/remote-ssh.md)。这项配置可用于现有 v1.0.17；新增应用内指南尚未发布。另已选择性移植并补齐该 PR 的 WebFetch 分页：同一真实 SSH 环境中，模型自动读取 3 页、完整还原 34,103 字节，同会话续读和跨会话拒绝也通过；9 次供应商调用全部 completed，源码全工作区 464 项测试通过。[分页修复证据](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-issue469-webfetch-fix.md)。此修复尚未发布，v1.0.17 不包含。远端 Semble 独有路径失败已复现；候选快照存在启动参数、忽略规则及索引来源缺陷，尚未采用，详见[审查记录](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-issue469-semble-check.md)。因此没有把 PR 整体标成已解决。证书错误与其他模型协议仍需独立复测。上游已归档，本段保存为草稿，没有发送。

后续源码进展：远程快照审查中发现的本机 Semble `.cursorignore` 与热缓存问题已独立修复，真实嵌入模型三组对照及 469 项全工作区回归通过。[独立修复记录](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-semble-cursorignore-fix.md)。尚未发布安装包，远程 Semble 采集仍未实现；这不是 #469 的整体完成声明，也未发送评论。

### Issue #511：已确认的清理缺口，原始增长来源待查（未发送）

我维护的 fork 中复现了两种统计清理删除记录却不收缩 SQLite 文件的独立问题。现补丁在删除事务提交后回收主库与 WAL 空间，保留既有清理范围；调用者断开后继续完成，其他读连接阻止截断时明确报告部分完成。

真实临时数据库从约 12.23 MiB 缩至 228 KiB，WAL 归零；历史、模型配置、共享 blob、并发读写、断开和失败重试均有覆盖。本机全工作区 475 项、Linux 473 项通过，Windows/macOS 各 6 项原生回归通过。[PR #1 与源码](https://github.com/haohaomin/cursor-byok/pull/1)、[详细边界](https://github.com/haohaomin/cursor-byok/blob/main/docs/upstream-issue-reviews/2026-10-27-issue511-storage-fix.md)。安装包尚未包含此补丁。

原截图的 19.09 GB 主库中有效会话、日志及空闲页各占多少尚未知，不能据此宣称所有增长原因已解决；没有清理正式数据。上游已归档只读，本段只保存为草稿，没有发送，也没有关闭原 Issue。
