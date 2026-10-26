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
