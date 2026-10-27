# 维护状态与上游问题处理

更新：2026-10-27。维护者：haohaomin。

原仓库 [leookun/cursor-byok](https://github.com/leookun/cursor-byok) 已归档。这个 fork 继续收集原仓库的问题和候选补丁，复现后修复，并保留原作者与贡献者署名。它不是 Cursor 官方项目，也不是原作者指定的继任仓库。

- 安装：[当前已发布的 v1.0.17](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.17)。
- 反馈：[本仓库 Issues](https://github.com/haohaomin/cursor-byok/issues)。请附原 Issue/PR 编号、操作系统、Cursor/BYOK 版本、模型协议、复现步骤和脱敏错误。
- 贡献：[本仓库 Pull Requests](https://github.com/haohaomin/cursor-byok/pulls)。

源码与安装包分别标记。源码中已有的修改不代表 v1.0.17 安装包已包含；未复现、部分通过或缺少原环境的条目不会标记为彻底解决。

## 已有修复与验证边界

| 上游问题 / PR | fork 中的处理 | 可用范围与证据 |
| --- | --- | --- |
| [PR #424](https://github.com/leookun/cursor-byok/pull/424)：Steer 后子代理结果丢失 | 已修复，保留执行并交付完成结果 | v1.0.4 起；[真实 Cursor 验证](upstream-pr-reviews/2026-09-29-pr424-fix-result.md) |
| [PR #423](https://github.com/leookun/cursor-byok/pull/423)：Anthropic 拒绝后执行工具或反复重试 | 已修复，拒绝时不执行工具、不重试 | v1.0.5 起；[真实 Cursor + 模拟 SSE 验证](upstream-pr-reviews/2026-09-30-pr423-fix-result.md)，没有冒充真实供应商拒绝测试 |
| [PR #421](https://github.com/leookun/cursor-byok/pull/421)、[Issue #410](https://github.com/leookun/cursor-byok/issues/410)：Responses 文字缺失 | 按输出项补齐文字，并读取最终快照中的缺失后缀 | v1.0.6 / v1.0.7 起；[验证记录](upstream-issue-reviews/2026-10-02-issue410-fix.md)。原作者网关的所有问题并未得到确认 |
| [Issue #477](https://github.com/leookun/cursor-byok/issues/477)、[PR #484](https://github.com/leookun/cursor-byok/pull/484)：子代理卡在 Planning next moves | 已修复不完整父任务头、子代理缺少消息 ID 的入口拒绝 | v1.0.8 起；[自动化及 Mac UI 验证](upstream-issue-reviews/2026-10-02-issue477-fix.md)，未现场覆盖 Windows 原环境 |
| [Issue #340](https://github.com/leookun/cursor-byok/issues/340)：后台子代理完成后父对话不继续 | 已修复完成通知交付，普通/Fast 模式各一轮真实链路通过 | v1.0.9 起；[验证范围](upstream-issue-reviews/2026-10-03-issue340-fix.md)，不等同于所有后台 Shell 边界都已通过 |
| [Issue #449](https://github.com/leookun/cursor-byok/issues/449)：子代理模型名单与设置 | 使用客户端模型信息和实际选择，保留 Cursor 设置的优先级 | v1.0.12 起；[修复及用户验收记录](upstream-issue-reviews/2026-10-08-issue449-fix.md) |
| [Issue #509](https://github.com/leookun/cursor-byok/issues/509)、[PR #420](https://github.com/leookun/cursor-byok/pull/420)：MCP schema 的 minLength 变成 1.0 | 源码已规范化整数约束，并保留大整数精度、空对象及 required 修复 | 已在 main，**v1.0.17 未包含这次合入**；[合入与回归](upstream-pr-reviews/2026-10-19-branch-integration.md)，原 Cursor 工具尚未实测 |
| [PR #514](https://github.com/leookun/cursor-byok/pull/514)：所有会话使用固定缓存键 | 按会话生成稳定键，连通性测试省略缓存键 | 源码修复 [`fe192c8`](https://github.com/haohaomin/cursor-byok/commit/fe192c8)，**未发布**；[复现与验证记录](upstream-pr-reviews/2026-10-27-pr514-fix.md)。不承诺命中率或延迟改善 |
| [PR #513](https://github.com/leookun/cursor-byok/pull/513)：事务失败后后续会话写入失败 | 修复 SQLite 自动回滚后残留 SQLx 事务深度的连接池污染 | 源码修复，**未发布**；[7 项故障恢复及 446 项全工作区回归](upstream-pr-reviews/2026-10-27-pr513-recovery.md)，原作者机器未现场复测 |
| [Issue #508](https://github.com/leookun/cursor-byok/issues/508)、[PR #474](https://github.com/leookun/cursor-byok/pull/474)：切换系统代理后请求继续走旧地址 | 检测代理配置变化，重建三种缓存 HTTP 客户端，保留现有响应流 | 源码修复，**未发布**；[隔离路由和认证回归](upstream-issue-reviews/2026-10-27-issue508-fix.md)，系统设置 UI 切换待实测 |
| Semble 共享索引层：`.cursorignore` 未生效，热缓存绕过新增规则 | 独立 Cursor 过滤；普通与相关查询先重新检查规则，保留未变向量 | 源码修复，**未发布**；[真实嵌入模型三组对照及 469 项回归](upstream-issue-reviews/2026-10-27-semble-cursorignore-fix.md)。不等同于远程 Semble 已解决 |

## 尚未解决的重点

- [#504](https://github.com/leookun/cursor-byok/issues/504)：已独立复现并修复活动 Run 中重复用户消息导致工具被中断的缺口，按消息 ID 在排队前去重。六项协议对照覆盖真实新消息、历史恢复和 Write 内容；原作者的全部症状仍缺请求样本，不能整体标为解决。[源码修复与验证边界](upstream-issue-reviews/2026-10-27-issue504-input-redelivery-fix.md)，尚未发布安装包。
- [#155](https://github.com/leookun/cursor-byok/issues/155)：保持“完成一半”。普通后台短任务自动续答已通过；真实审批等待、压缩期间完成、最终补丁的输出通知等边界仍待验收。20 分钟测试按维护者本轮范围排除。[验收记录](upstream-issue-reviews/2026-10-19-issue155-acceptance.md)
- [#505](https://github.com/leookun/cursor-byok/issues/505)：多个 reasoning signature。两个合成 checkpoint 已复现拒绝第二个签名；原生 Anthropic 多块签名回放及 OpenAI Chat 聚合对照通过。仍未修复，原作者网关的签名数据与历史来源待取证，不能直接删除校验或覆盖签名。[复现与验证边界](upstream-issue-reviews/2026-10-27-issue505-check.md)
- [#469](https://github.com/leookun/cursor-byok/issues/469) / [PR #498](https://github.com/leookun/cursor-byok/pull/498)：Remote SSH。真实 `auto` 代理连接失败已复现；远端设为 `always` 后，基础 Agent、Read/Shell、编辑、续聊、取消与窗口重载通过，v1.0.17 可使用这个[配置办法](remote-ssh.md)。WebFetch 分页已在源码修复，真实 SSH 的全文、续读、跨会话隔离及 464 项回归通过；[安装包尚未包含](upstream-issue-reviews/2026-10-27-issue469-webfetch-fix.md)。远程 Semble 路径失败已复现；候选快照的启动、忽略规则及索引身份存在实测缺陷，[尚未采用，仍待修复](upstream-issue-reviews/2026-10-27-issue469-semble-check.md)。后续 [Ls/元数据实验](upstream-issue-reviews/2026-10-27-issue469-inventory-check.md)确认原生清单不完整；元数据探针 Linux 验证通过，完整过滤与 Cursor 采集仍未接通。
- [#511](https://github.com/leookun/cursor-byok/issues/511)：已独立复现并修复“清理统计后文件不缩小”。两种范围、历史/模型保留、读连接占用、调用者断开及失败恢复均有临时库验证；本机 475 项、Linux 473 项、Windows/macOS 各 6 项回归通过，[PR #1](https://github.com/haohaomin/cursor-byok/pull/1)记录源码合入状态，尚未发布。原作者 19.09 GB 主库的具体组成和增长来源仍待取证，[不能视作全部解决](upstream-issue-reviews/2026-10-27-issue511-storage-fix.md)。

## 清单与公开回复

本次从 GitHub 分页读取全部 Issue/PR 元数据：当前开放 **104 个 Issue、51 个 PR**。沿用原来的 [Issue 主清单](upstream-issue-reviews/2026-10-02.md) 和 [PR 主清单](upstream-pr-reviews/2026-09-28.md)，保留过去的证据及未完成状态。采集不等于每项都已复现或逐行审计。

原仓库归档后不允许新增评论。2026-10-27 对 PR #423 的具体修复回复被 GitHub 以 `403 Repository was archived so is read-only` 拒绝；没有成功发布评论，也不会在其他无关讨论中重复推广。可复核的说明保存在 [回复草稿与投递记录](upstream-pr-reviews/2026-10-27-community-replies.md)。后续反馈请使用本 fork 的 Issues。
