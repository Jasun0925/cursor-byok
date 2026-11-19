# PR 审查清单

> 整理：2026-11-04。**上游状态以 2026-10-27 快照为准，当时开放 51 个 PR。** 相比首次 37 项，新增 15 项、#227 关闭且未合并；#483 head 已更新为 `122a4b3071b8`，原 SHA 的评估不能当作新补丁验收。当前完整 head 清单见 [快照](history/pull-requests/2026-10-27.snapshot.json)。原有历史评估仍保留，不能将旧 CI/无冲突判断用于当前 main。
>
> 之前投稿的 #487、#500、#501 因用户要求删除源分支而关闭，均未合入上游；修复提交保留在本 fork main。评论尝试被归档只读的 403 拒绝，见 [回复记录](history/pull-requests/2026-10-27-community-replies.md)。

## 2026-11-13 本 fork PR #44

| PR / head | 解决目标与范围 | 验证边界 |
| --- | --- | --- |
| [#44](https://github.com/haohaomin/cursor-byok/pull/44) / `64104cf` | 为模型设置增加内置模型 JSON 配置备份导出/导入；导入按模型哈希跳过重复项，并补齐排序、并发刷新、导出 URL 和大请求体边界 | 原始提交工作区 Rust 293 项通过、14 项忽略；修复后新增排序/大请求体回归，前端检查、四语言扫描、Clippy 和 Tauri debug 构建通过；Oracle Linux 9.8 ARM64 上 fmt、4 项导入回归和 Clippy 通过；Sourcery 复审通过，但该分支没有 GitHub Actions 运行记录。真实安装包文件选择器和跨电脑导入尚待人工验收。详见 [Issue #471 记录](history/issues/2026-11-13-issue471-model-backup.md)。 |

## 2026-11-06 已合并 PR 的测试状态

本 fork 最近合并的 #12、#14、#15、#16、#17、#18、#19、#20、#21、#22、#23，最终 PR 提交的 Frontend、Rust Linux、Desktop Rust Windows/macOS 检查均通过。合并后 main `b6936e885db61` 的 [CI](https://github.com/haohaomin/cursor-byok/actions/runs/42147293009) 也全部通过。

这里的“通过”是自动化测试与构建结果。真实 Cursor UI、真实 provider 账号、远程 SSH、审批交互仍以各项独立验收记录为准，不能由绿色 CI 推断完成。远程审批 [PR #13](https://github.com/haohaomin/cursor-byok/pull/13) 仍未合并。

## 2026-11-06 上游 PR #237 的独立实现

本地标题生成与元数据保存已通过 [PR #24](https://github.com/haohaomin/cursor-byok/pull/24) 合入 main（`4e90dce`）。新增 9 项回归、工作区全量 563 项测试、Clippy 和 PR 四组 CI 通过；本地跳过 13 项需 Deno 的插件测试与 1 项需缓存 Semble 模型的测试。真实 Cursor 自动改名尚未验收。来源、数据库升级和请求隔离边界见[验证记录](history/pull-requests/2026-11-06-pr237-conversation-title.md)。

## 2026-11-06 上游 PR #211 的选择性实现

当前 Rust 已保留 Grep 正文；本轮只修复仍丢失的分页、总量与工作区信息。新测试修复前 4 项失败，修复后 5 项通过；完整服务端 503 项测试、Clippy 与格式检查通过，14 项环境测试按原配置忽略。当前 head 与历史快照不同；来源、执行路径及 Ls 未移植的边界见[验证记录](history/pull-requests/2026-11-06-pr211-grep-result-context.md)。

## 2026-11-04 本 fork PR #10

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#10](https://github.com/haohaomin/cursor-byok/pull/10) / `042bd8c2cc51` | Statsig 禁用不支持的本地 Agent 循环；解码失败诊断 | 选择性采用门控与测试；日志只保留元数据。未采用 README 和原始 payload 日志。原 PR 已关闭、未合并；本地整合及隔离验证见[记录](history/pull-requests/2026-11-04-pr10-check.md)，真实 Cursor 待验。 |

## 2026-11-06 本 fork PR #18

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#18](https://github.com/haohaomin/cursor-byok/pull/18) / `08bf3ba` | 增加俄语桌面本地化 | 按当前 fork 的 source 词条重新生成并补译 `ru-RU`，接入系统语言解析、设置选择器和 i18n 校验；本地检查与四组 CI 通过，已合入 main。真实安装包 UI 切换待发布前复核。 |

## 2026-11-06 本 fork PR #23

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#23](https://github.com/haohaomin/cursor-byok/pull/23) / `dfea8ef` | 自动压缩摘要模型永久挂起后无法继续 | 保留现有压缩屏障、重复压缩和回退摘要，只加入 30 秒 deadline 与超时回退；定向、完整服务端测试、Clippy、格式检查和四平台 CI 通过。真实 Cursor UI/账号/远程 SSH 未验收；详见[移植记录](history/pull-requests/2026-11-06-pr378-compaction-timeout.md)。 |

## 2026-11-06 本 fork PR #17

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#17](https://github.com/haohaomin/cursor-byok/pull/17) / `b0a8e50` | 运行时覆盖手动和自动压缩提示词 | 选择性实现当前 Rust 架构的服务端路径；本地 `cursor-server` 全部测试 255✓、14 个忽略，Clippy/格式检查通过；PR CI 的 Frontend、Linux、Windows、macOS 全部通过。未实现上游正文声称的桌面 Config 页面；详细边界见[验证记录](history/pull-requests/2026-11-06-pr343-runtime-compaction.md)。 |

## 2026-11-06 本 fork PR #21

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#21](https://github.com/haohaomin/cursor-byok/pull/21) / `3b5ce52` | 提示词正文中的结构标签注入防护 | 按当前 Rust `cursor/compile` 路径重实现；库测试、全部目标测试、定向回归、Clippy、格式检查和四平台 CI 通过。真实 Cursor UI/账号未验收；详见[验证记录](history/pull-requests/2026-11-06-pr229-prompt-injection.md)。 |

## 2026-11-06 本 fork PR #22

| PR / head | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| [#22](https://github.com/haohaomin/cursor-byok/pull/22) / `7dda5f1` | 全局 Cursor 规则的本地相关规则协议入口 | 复用现有规则存储、同步和上下文合并，只补 `FetchRelevantKnowledgeForConversation` 路由与代理分流；定向、完整服务端测试、Clippy、格式检查和四平台 CI 通过。真实 Cursor UI/账号/远程 SSH 未验收；详见[移植记录](history/pull-requests/2026-11-06-pr345-global-rules-relevance.md)。 |

## 2026-11-06 上游 PR #375 的选择性实现

| 上游 PR / 本 fork 分支 | 解决目标与范围 | 采用方式与验证边界 |
| --- | --- | --- |
| 上游 [#375](https://github.com/leookun/cursor-byok/pull/375) / 本 fork [PR #20](https://github.com/haohaomin/cursor-byok/pull/20) / `codex/issue375-resume-progress` | ResumeAction 上下文和无进展恢复 | 不直接采用旧 `request/` 路径；迁移到当前 `compile/` 路径，服务端全量目标测试、Clippy、格式检查通过。PR #20 已创建，真实 Cursor UI 未验收。详见[验证记录](history/pull-requests/2026-11-06-pr375-resume-progress.md)。 |

## 2026-10-27 新增候选

下表以每项最新记录为准；仅标为初筛的条目不表示已逐行审计或验证作者的测试声明。#514 读取了固定 head 的完整 diff、讨论、reviews 和 checks；后三者均为空，无可用上游 CI 结论。#513、#498 的后续处理分别链接到独立证据。

| PR | head | 文件数 | 本轮结论与下一步 |
| --- | --- | ---: | --- |
| [#493](https://github.com/leookun/cursor-byok/pull/493) Keep HTTP/2 when takeover relays Cursor streaming calls | `7ae7b030a07f` | 93 | 待专项验证：93 文件，包含 hudsucker vendoring；需要双向 HTTP/2 与本地代理回归。 |
| [#484](https://github.com/leookun/cursor-byok/pull/484) fix: accept incomplete Cursor subagent run protocol (parent headers, message id) | `ea5864a20e4d` | 5 | 对应入口修复已有 #477 覆盖；不重复合入，保留现有父子关系与幂等语义。 |
| [#510](https://github.com/leookun/cursor-byok/pull/510) feat(plugins): add GitHub Copilot (copilot-auth) built-in provider plugin | `2d882ea88e8e` | 14 | 新 Copilot 插件；需账号授权、token 生命周期、请求路由、错误重试及额度验证。 |
| [#515](https://github.com/leookun/cursor-byok/pull/515) fix: accept Cursor subagent runs with empty message_id | `ef0bd94b8681` | 6 | 不采用。主线准入 8 项通过；移植内容身份/丢弃单父头后 3 项回归，重复轮次历史缩短且父任务进度丢失，见 [复核](history/pull-requests/2026-10-28-pr515-check.md)。 |
| [#514](https://github.com/leookun/cursor-byok/pull/514) fix: 修复提示缓存键固定值，并在模型测试中省略缓存键 | `bf4fab373aae` | 10 | 已复现并移植，补真实连通性入口测试；源码修复，已随 v1.1.0 发布。 |
| [#513](https://github.com/leookun/cursor-byok/pull/513) fix: keep sessions running after failed writes and incomplete subagent requests | `87ee81dced16` | 17 | 已独立修复可复现的连接池污染；使用 SQLx 归还检查，保留正常回滚连接，未导入 WriteSession 或重复子代理补丁。7 项定向与 446 项工作区回归通过，[证据](history/pull-requests/2026-10-27-pr513-recovery.md)；已随 v1.1.0 发布。 |
| [#512](https://github.com/leookun/cursor-byok/pull/512) feat(auto-review): review Shell, WebFetch and MCP calls when Auto-review is on | `08f5c335c6a0` | 12 | Auto-review 新协议与审批行为；需真实客户端验收，未合入。 |
| [#485](https://github.com/leookun/cursor-byok/pull/485) [codex] Fix BYOK Agent tools, subagents and native todos | `49e29e3756e1` | 12 | 工具/子代理/todo 综合补丁；先拆分需求，避免覆盖现有完整生命周期修复。 |
| [#498](https://github.com/leookun/cursor-byok/pull/498) fix(remote-ssh): restore BYOK for Cursor 3.21+ SSH (#469) | `c7114878c330` | 35 | 选择性采用，未整体合入。配置指南与 WebFetch 分页已有 [真实 SSH 验证](history/issues/2026-10-27-issue469-webfetch-fix.md)；早期快照候选的 [启动/忽略/身份缺陷](history/issues/2026-10-27-issue469-semble-check.md)有独立失败证据。重写的采集及分页实现已通过 fork PR #8 合入 main，最新审批候选未提交、真实批准/拒绝待验收，见 [工作状态](work-status.md)。已随 v1.1.0 发布。 |
| [#497](https://github.com/leookun/cursor-byok/pull/497) CODEX ACCOUNTS: choose one Codex account and show remaining quota | `a4199525ab82` | 40 | 账号选择、切换和额度 UI；需并发选择与首输出前后切换回归，未合入。 |
| [#496](https://github.com/leookun/cursor-byok/pull/496) ALIAS MODELS: create model groups that fail over by priority | `fda59c5157a0` | 100 | 模型别名与故障转移共 100 文件；需数据库、会话粘性、失败重试语义专项设计。 |
| [#495](https://github.com/leookun/cursor-byok/pull/495) ADD CUSTOM PLUGINS: install a plugin from a chosen directory | `0a8cece13535` | 18 | 自定义插件目录安装；需路径穿越、符号链接和账号保留验证，未合入。 |
| [#491](https://github.com/leookun/cursor-byok/pull/491) AGENT MANAGEMENT: feat(control): let a local agent manage the app | `c6373fcc30a6` | 16 | 本地管理 API；需默认关闭、凭据保护和授权边界验证，未合入。 |
| [#494](https://github.com/leookun/cursor-byok/pull/494) LANGUAGE: add Russian (ru-RU) localization | `d44e5fe92451` | 9 | 俄语本地化；需迁移到本 fork 当前词条并检查占位符，未合入。 |
| [#492](https://github.com/leookun/cursor-byok/pull/492) PLUGIN: Claude OAuth — complete subscription connection | `5fff04775a17` | 17 | Claude OAuth 插件及辅助网络日志隔离；需账号、刷新、worker 日志与原生 thinking 回放专项验证。 |

## 已完成与开发中

#424、#423、#421 的本地修复和既有发布记录见 [维护状态](maintenance-status.md)；它们的旧“待 UI / 待合入”过程描述不再作为待办。上游 #420 的完整 schema 修复已进 main，但 v1.0.17 不包含后续补修。

远程 Semble（fork PR #8）、Grok（#4）和 GPT/Google OAuth（#6）均已于 2026-11-04 合入 main；已随 v1.1.0 发布。Semble 本地审批补丁未提交，真实账号与客户端审批仍待验收，见 [工作状态](work-status.md)。

## 2026-09-28 初筛的基线与方法

- 上游：[leookun/cursor-byok](https://github.com/leookun/cursor-byok)，`main=2068ab20513288e6a0289d33febe872a30608610`。
- 当时的改版：[renhao12356578/cursor-byok](https://github.com/renhao12356578/cursor-byok)，`main=973fca3efe3c0aae11b84faf57b01405138a430b`。`git log HEAD..upstream/main` 为空：当时上游主分支没有该改版未包含的提交，未合并 PR 是另一回事。
- 读取所有 open PR 的描述、文件清单、可见 CI、review 和行内评论；原 36 项另读取了讨论区评论。最终复查新增 #483，补充了其文件和关键宿主改动分析。
- 对每个 PR head 执行 `git merge-tree --write-tree --name-only <fork-head> <pr-head>`；**10 项无文本冲突，27 项有冲突**。这只验证 Git 可合并性，不代表编译、运行或功能正确。
- 深入核对近期小修复及关键风险路径；旧 Go/Vue 和超大 PR 以架构、范围及冲突筛查为主，**不声称完成所有变更的逐行审计**。API 省略的 patch 不视为已审完。
- CI 栏“4✓”表示查询时 GitHub 显示 Linux Rust、Frontend、Windows Rust、macOS Rust 全部成功；“未见”表示没有可见检查结果，**不等于失败，也不等于通过**。作者在正文自报的测试不当作本次验证。
- 判定针对表中固定 head SHA。合入其他 PR、上游继续提交或 PR 更新后，需要重新计算冲突并验证。

## 完整清单

“可合并性”均针对上节注明的历史基线；“冲突 N”表示 N 个冲突文件。规模为 GitHub 返回的文件数和增删行数，包含测试、生成文件及空行。

| PR | 内容 | 分级 | 规模（文件；+/-） | CI | 可合并性 | 判断依据与下一步 |
| --- | --- | --- | --- | --- | --- | --- |
| [#424](https://github.com/leookun/cursor-byok/pull/424) | ✅ 已解决：消息中断后保留 Task 输出 | A（已完成） | 1；+58/-1（原 PR） | 4✓（原 PR） | 已以完整修复合入个人 fork | **2026-09-30：已修复、实机验证、合入并发布 v1.0.4。** 完整方案补齐迟到结果交付和界面完成状态；独立提交上游 [#487](https://github.com/leookun/cursor-byok/pull/487)。无需再单独合入原 #424。 |
| [#423](https://github.com/leookun/cursor-byok/pull/423) | ✅ 已完成：Anthropic 拒绝不执行工具、不重试 | A（已完成，v1.0.5） | 原 PR 1；+78/-15；完整修复 7 个生产文件 | 原 PR 4✓；本地 270 测试✓ | 修复 `89d90c6`，已合入 main（分支末端 `6402ef7`） | 明确拒绝类型贯穿运行层和 Cursor 协议；请求 1 次、执行消息 0 条、客户端不可重试。详见 [修复记录](history/pull-requests/2026-09-30-pr423-fix-result.md)。 |
| [#421](https://github.com/leookun/cursor-byok/pull/421) | ✅ 已完成：Responses 按输出项补全文本 | A（已完成，v1.0.6） | 原 PR 1；+79/-0 | 原 PR 4✓；组合 287 测试✓；CI 全✓ | 已合入个人 fork main（`e08cab5`） | 原 PR 的 3 个失败场景已补齐；真实 UI 验证通过，Skipped 独立修复一并发布。详见 [发布记录](history/releases.md#v106)。 |
| [#479](https://github.com/leookun/cursor-byok/pull/479) | OpenCode 免费模型插件 | B | 12；+1211/-121 | 未见 | 无文本冲突 | 中：新外部服务及插件管理 UI；旧审查提出的端点、会话 ID、断言问题在当前 SHA 已有修正。 **下一步：**验证目录同步、Chat/Responses、限额、工具调用及原有插件 UI。 |
| [#466](https://github.com/leookun/cursor-byok/pull/466) | Antigravity 额度与账号启停 | B | 15；+649/-119 | 未见 | 冲突 1 | 中：宿主 Disabled 状态、冷却选择及新接口；冲突仅在生成翻译目录，但运行语义仍需验证。 **下一步：**重新生成翻译；验证禁用账号不被回写启用、重启保留和所有账号冷却。 |
| [#460](https://github.com/leookun/cursor-byok/pull/460) | 缺失 usage 时估算上下文 | B | 5；+306/-2 | 4✓ | 无文本冲突 | 本 fork 已在 `origin/main` 上选择性重实现并补集成回归；未知模型窗口默认 200k，真实 usage 保留，失败/取消不生成成功用量。实现与边界见[验证记录](history/pull-requests/2026-11-06-pr460-context-usage.md)。 |
| [#459](https://github.com/leookun/cursor-byok/pull/459) | Grok OAuth 自动续期与思考强度 | B | 17；+1073/-59 | 未见 | 现版冲突已适配并补全 | [现版验证](history/issues/2026-10-27-issue445-check.md) 已复现并修复 #445；排除独立 reasoning 改动，补齐重置、模型取消与手动刷新。Grok 23 项脚本、最新 Linux/macOS 宿主各 37 项与 Windows 38 项通过；四组 CI 全通过，真实账号仍待验收。个人 fork PR #4 已合入 main，已随 v1.1.0 发布。 |
| [#420](https://github.com/leookun/cursor-byok/pull/420) | MCP schema 整数约束规范化 | B（本地已合入） | 原 PR：1；+182/-31 | 本轮服务端 387 测试及 Clippy 通过 | 已合入并推送 main（`e2f17e0`） | 已补齐原补丁的大整数精度缺陷；合并保留空 schema、properties 和 required 修复，新增组合回归。v1.1.0 已包含该合入，未新增真实 UI 验证；上游仍 OPEN、仓库已归档。见 [合并记录](history/pull-requests/2026-10-19-branch-integration.md)。 |
| [#483](https://github.com/leookun/cursor-byok/pull/483) | CodeBuddy CN 与后台自动签到 | C | 38；+4888/-211 | 未见 | 冲突 1 | 高：38 文件，新增宿主调度、锁、状态持久化、凭据刷新及导出策略；作者未测真实流式聊天。 **下一步：**单独验证调度取消/重启/跨日、账号并发与 token 日志保护，再测真实聊天。 |
| [#474](https://github.com/leookun/cursor-byok/pull/474) | 代理缓存刷新，夹带规则黑名单 | C | 6；+242/-1 | 4✓ | 无文本冲突 | 高且有具体问题：与标题无关的规则正文日志、匹配规则删除、返回成功但未保存。 **2026-10-27：**网络部分已独立实现并补验证；规则删除/正文日志仍不采用。见[#508 修复](history/issues/2026-10-27-issue508-fix.md)。 |
| [#458](https://github.com/leookun/cursor-byok/pull/458) | Windows cmd 转义及 CA 信任范围 | C | 3；+7/-8 | 未见 | 无文本冲突 | cmd 参数修复已由 fork PR #14 合入 main；证书改为 CurrentUser 的部分暂不采用，会让既有 LocalMachine 证书被误报为未信任。上游补丁已完成交叉编译和工作区回归，但没有 Windows 实机证书验收；除非先定义迁移策略，否则不再重复发布。 |
| [#447](https://github.com/leookun/cursor-byok/pull/447) | Agent 路由、模型别名与 CLI 启动器 | C | 19；+1120/-2 | 未见 | 冲突 1 | 高：19 文件多功能；与 fork 的控制路由冲突；上游作者不愿维护新 launcher 栈。 **下一步：**拆出 bidi/MCP/路由修复；逐项验证。CLI 优先评估官方 agent -e 入口。 |
| [#437](https://github.com/leookun/cursor-byok/pull/437) | Antigravity 模型和额度管理大改 | C | 60；+2847/-1037 | 未见 | 冲突 1 | 高：60 文件，包含数据库 0010、账号/模型管理及非必要 release workflow 改写。 **下一步：**拆出功能和发版流程；用数据库副本测升级，保护 fork 的更新源与签名。 |
| [#433](https://github.com/leookun/cursor-byok/pull/433) | 所有 Cursor 请求经本地代理 | C | 2；+72/-4（上游） | 上游无可用 checks | 选择性重实现 | 本 fork 只采用非 Tab Cursor 请求进入本地路由、CONNECT 不重写两项；明确舍弃硬编码 400/404 短路和 `custom-protocol` feature。4 项代理单元测试、Clippy、桌面检查和全工作区回归通过；真实代理网络与 Cursor UI 待验，见[验证记录](history/pull-requests/2026-11-13-pr433-proxy-routing.md)。 |
| [#378](https://github.com/leookun/cursor-byok/pull/378) | 自动压缩持久化和恢复 | C | 8；+1082/-58 | 未见 | 冲突 3 | 高：checkpoint 发布屏障、重试/恢复和超时共同改变；当前引擎及输出模块冲突。 **下一步：**移植到现有生命周期；验证压缩后立即失败、重启恢复、前缀稳定性。 |
| [#375](https://github.com/leookun/cursor-byok/pull/375) | ResumeAction 上下文与继续执行 | C | 5；+295/-18 | 未见 | 冲突 3 | 高：actor/请求编译路径已变化，涉及重复执行和无进展重试。 **下一步：**按现有 compile 模块移植；验证取消、重复 Resume、工具完成后的继续执行。 |
| [#364](https://github.com/leookun/cursor-byok/pull/364) | 工具调用 ID 上限 64→256 | C（选择性重实现） | 1 个生产文件；新增 4 项回归 | 本 fork 本地全目标通过；[PR #19](https://github.com/haohaomin/cursor-byok/pull/19) CI 全平台通过 | 冲突 1 | 不采用全局 256；保留 provider 的 64 字节边界，长 ID 用完整 SHA-256 十六进制值稳定映射，避免截断碰撞。实现与测试见[记录](history/pull-requests/2026-11-06-pr364-tool-call-id.md)。 |
| [#354](https://github.com/leookun/cursor-byok/pull/354) | 重复自动压缩与工具输出截断 | C | 6；+505/-77 | 未见 | 冲突 5 | 高：同时改自动压缩、事件 ID 和 provider 历史投影；与 #378 重叠并有多处冲突。 **下一步：**合并需求后统一设计；验证多轮压缩、完整结果持久化及历史前缀稳定。 |
| [#353](https://github.com/leookun/cursor-byok/pull/353) | 压缩保留空间可配置 | C | 11；+433/-64 | 未见 | 冲突 10 | 高：旧设置 UI/共享 API/存储与引擎冲突；会影响上下文阈值。 **下一步：**先明确与 #378/#460 的阈值关系，再移植并测试边界。 |
| [#350](https://github.com/leookun/cursor-byok/pull/350) | ✅ 已选择性实现：自定义上下文窗口优先 | C | 7；+69/-38（上游）；本 fork 9 个文件 | 上游未见；本地 255 单元测试及集成测试✓，桌面检查✓；个人 fork PR #16 CI 全✓ | 已提交个人 fork [PR #16](https://github.com/haohaomin/cursor-byok/pull/16) | 已按当前 `cursor/services/model_catalog` 和 `ModelConfig` 路径重实现；配置上下文排在第一项并成为默认变体，真实上下文选择不会覆盖保存值。未导入旧 protobuf 字段和无关刷新改动。详见[验证记录](history/pull-requests/2026-11-06-pr350-context-priority.md)。 |
| [#345](https://github.com/leookun/cursor-byok/pull/345) | 全局 Cursor 规则持久化和注入 | C | 7；+527/-8 | 未见 | 冲突 5 | 高：旧 handlers/prompting 路径；当前已有 knowledge 服务和 local rules 合并。 **下一步：**先比较现有覆盖，避免重复注入或改变会话内前缀；只移植缺失行为。 |
| [#343](https://github.com/leookun/cursor-byok/pull/343) | 运行时覆盖压缩提示词 | C | 4；+63/-9 | 未见 | 冲突 4 | 中偏高：当前 head 已是作者建议的精简实现，非描述中的完整配置 UI，但仍与新编译路径冲突。 **下一步：**按实际 4 文件 diff 移植 prompts/compaction.md 支持，验证手动和自动压缩。 |
| [#360](https://github.com/leookun/cursor-byok/pull/360) | Grok/Codex OAuth 与账号池旧实现 | D | 41；+7565/-369 | 未见 | 冲突 28 | 高：旧订阅模块、前端及两项迁移；当前已有内置 Grok/Codex 插件。 **下一步：**不直接合并；需求映射到当前插件体系。 |
| [#312](https://github.com/leookun/cursor-byok/pull/312) | Windows/macOS 发布签名旧构建 | D | 5；+127/-7 | 未见 | 冲突 5 | 高：Wails/Taskfile 方案与现有 Tauri Actions 发布流程不匹配。 **下一步：**不直接合并；沿用现有独立签名与更新源。 |
| [#293](https://github.com/leookun/cursor-byok/pull/293) | Shell 审批恢复等待 | D | 4；+484/-1 | 未见 | 冲突 3 | 旧 Go internal/backend 路径已退出当前架构。 **下一步：**仅提取测试场景，检查现有 Rust 审批恢复。 |
| [#245](https://github.com/leookun/cursor-byok/pull/245) | 编排工具、Projects 与协议同步 | D | 26；+2872/-212 | 未见 | 冲突 18 | 旧 Go 后端及旧 prompt/proto 路径；变更面较广。 **下一步：**按当前 protocols/cursor 和 Rust 功能逐项比较。 |
| [#237](https://github.com/leookun/cursor-byok/pull/237) | 会话标题生成和持久化 | D（Rust 独立实现） | 原 PR：9；+331/-9 | 原 PR 未见；本地 563 项测试通过 | 不直接移植旧 Go 路径 | 补齐本地 NameTab 和早到元数据保存；新增 9 项回归，数据库升级和公共路由已验证，真实 Cursor UI 待验收。见[记录](history/pull-requests/2026-11-06-pr237-conversation-title.md)。 |
| [#229](https://github.com/leookun/cursor-byok/pull/229) | 提示词结构标签注入防护 | D | 3；+166/-8 | 未见 | 冲突 2 | 安全意图值得保留，但补丁落在旧 Go prompt 编译器。 **下一步：**把攻击样例迁入当前提示词测试后评估，勿复活旧模块。 |
| [#227](https://github.com/leookun/cursor-byok/pull/227) | Shell 前台超时和错误报告 | D | 5；+14/-10 | 未见 | 冲突 5 | 旧 Go forwarder/model/mitm 路径。 **下一步：**按当前工具运行时验证超时和错误传播。 |
| [#216](https://github.com/leookun/cursor-byok/pull/216) | 思考链、AOS、代理和 UI 综合增强 | D | 423；+61346/-14030 | 未见 | 冲突 109 | 极高：423 文件；上游作者已要求拆分，含旧构建、后端、前端与证书路径。 **下一步：**拒绝整体合入；按单一功能重做，不导入证书材料。 |
| [#211](https://github.com/leookun/cursor-byok/pull/211)（草稿） | 保留 grep/ls 结果 | D（Rust 选择性实现） | 原 PR：2；+304/-3 | 原 PR 未见；本地服务端 503 项通过 | 不直接移植旧 Go 路径 | 当前正文与大小限制已有；补齐分页、计数和范围标签，旧 Ls 不移植。最新 head `d280adae51f69`，详见[复现与验证](history/pull-requests/2026-11-06-pr211-grep-result-context.md)。 |
| [#210](https://github.com/leookun/cursor-byok/pull/210)（草稿） | 临时过载流式重试 | D | 7；+311/-16 | 未见 | 冲突 4 | 草稿，旧 Go provider/actor 路径；重试可能重复输出或执行。 **下一步：**如需功能，在当前 provider 层验证首字节前后重试语义。 |
| [#208](https://github.com/leookun/cursor-byok/pull/208) | 逐模型系统提示词 | D | 15；+486/-9 | 未见 | 冲突 13 | 旧 Vue/Go 配置及编译链；与当前提示词和前缀稳定要求需重新对齐。 **下一步：**不直接合并；先定义当前会话内何时允许改变提示词。 |
| [#198](https://github.com/leookun/cursor-byok/pull/198) | 工程审计加桌宠 | D | 154；+16830/-4072 | 未见 | 冲突 72 | 极高：154 文件、多功能混合；作者明确无法评估副作用；触及旧广告组件。 **下一步：**拒绝整体合入，按独立问题提取。 |
| [#196](https://github.com/leookun/cursor-byok/pull/196) | CmdK 和终端生成 | D | 21；+1908/-52 | 未见 | 冲突 15 | 旧 Vue/Go 请求和配置链。 **下一步：**有需求再按现有架构单独实现。 |
| [#194](https://github.com/leookun/cursor-byok/pull/194) | 渠道分组和模型发现 | D | 34；+3507/-606 | 未见 | 冲突 18 | 旧 Vue/Go；触及旧 AdModelProvider 和 AGENTS，不能覆盖当前无广告定制及规则。 **下一步：**只比较功能缺口，不整体合入。 |
| [#193](https://github.com/leookun/cursor-byok/pull/193) | WebSocket Fast、MCP hub、统计修复 | D | 29；+2008/-11 | 未见 | 冲突 23 | 旧 Go/Vue，多个网络与持久化功能捆绑。 **下一步：**拆分需求，分别映射到当前 provider/plugin/store。 |

## 需要特别注意的证据

### #474：CI 通过，但实际不只是网络修复

[规则入口](https://github.com/leookun/cursor-byok/blob/24316e9642a8e6c3e048bb70a255c8200fe4f130/server/src/cursor/services/knowledge/mod.rs#L135) 在普通 add/update 路径以 info 级别记录完整规则正文；命中 `.blocklist` 时，add 返回成功和 `id="blocked"`，update 删除旧规则且忽略删除结果。[规则存储](https://github.com/leookun/cursor-byok/blob/24316e9642a8e6c3e048bb70a255c8200fe4f130/server/src/cursor/services/knowledge/store.rs#L145) 也会清理已落盘内容。这会造成规则内容进入日志，以及用户看到成功但数据未保存/被删除。**不建议整项合并。2026-10-27 已独立实现代理缓存修复并补齐隔离 HTTP 验证；源码已随 v1.1.0 发布，系统 UI 切换待验，见[处理记录](history/issues/2026-10-27-issue508-fix.md)。** 没有 `.blocklist` 时不触发黑名单删除，但正文日志仍存在。

### #433：硬编码拒绝请求早于正常分流

[本地代理](https://github.com/leookun/cursor-byok/blob/a5b1ce085d6a48e9fa83866f49dbb198ae753c70/server/src/local_app/proxy.rs#L134) 在检查 Cursor host、TAB Direct 分流之前匹配 `local_stub_status`，对 FileSync、BackgroundComposer 等接口直接返回 400/404；未检查账号是否真的没有这些功能。对依赖这些接口的用户会改变行为，不能视为纯粹的出站代理修复。Cargo 同时强制开启 `custom-protocol`，需要独立验证开发运行模式。

### #458：cmd 修复已采用，证书范围改动暂缓

[证书安装](https://github.com/leookun/cursor-byok/blob/5b63b8586a22975114e955933022b1f3c64a9de4/server/src/local_app/ca/mod.rs#L91) 与 Windows 检测一起转到 CurrentUser，而既有 LocalMachine 证书不再被这段检测识别。cmd 参数部分已经由 fork PR #14 合入 main；证书范围部分即使交叉编译和工作区回归通过，也不能替代 Windows 实机证书验收，因此暂不采用，避免把现有安装标记为未信任。

### #437 / #483：插件标题下还有共享基础设施改动

#437 新增 `0010_add_model_enabled.sql`，当前已有 0001–0009；新增迁移本身并非已证实缺陷，但不能省略旧库升级验证。其 [release workflow](https://github.com/leookun/cursor-byok/blob/c5d5de08ff922977ecf023d81ee3534d1454157b/.github/workflows/release.yml#L20) 加入贡献者仓库/分支发布逻辑，以及 Actions 版本、构建和更新包生成方式变化，不能直接套用到个人 fork。

#483 的 [App 生命周期](https://github.com/leookun/cursor-byok/blob/c0f89f8be3f79ffed57141a0c68d5fdb523d3a12/server/src/app.rs#L140) 启动后台调度并在退出时等待；同时更改共享插件锁、资源数据更新、网络敏感标记和导出政策。这些是需要专项验证的风险面，**并非已证明的故障**。作者说明未做真实流式聊天验证。

### 旧评论需与当前代码核对

#479 旧 review 中的三项意见，当前 head 已分别加入 Responses 路由、稳定 session 派生和阻止付费模型联网的断言，不能继续当作当前未修复缺陷。仍需验证占位工具、完整工具调用、多轮请求及原有插件 UI。

#343 正文仍描述自动创建配置和配置 UI，但当前 head 实际只有 4 文件的可选 `prompts/compaction.md` 实现；本报告按实际 diff 评价。

#364 上游作者明确指出“某些 provider 校验了 tools 的最长为 64”；#447 作者建议官方 `agent -e` 入口并拒绝维护另一套 launcher；#216/#198 作者要求拆分或表示无法评估副作用。原讨论链接保存在 snapshot 中。这些意见是评估依据，不等于 GitHub 已关闭或正式拒绝 PR。

## 三个优先候选的验证记录

用当前 fork 的 `git archive HEAD` 建立 `/tmp` 隔离副本，分别取 #424、#423、#421 与 fork 的无冲突合并树中变动的单个文件；核对每项只改变其声明的那个文件。组合副本包含三项修复，未切换主工作区分支。

执行：

```bash
CARGO_TARGET_DIR=/Users/renhao/works/cursor-byok/target \
  cargo test --locked \
  --manifest-path /tmp/cursor-byok-pr-review-20260928/candidate/Cargo.toml \
  -p cursor-server --lib
```

结果：退出码 101，停在 `sqlx` 编译时加载 `libsqlx_macros-*.dylib`，错误为 **`mis-aligned LINKEDIT string pool`**。清理并重建该依赖包缓存后重试仍相同。**尚无本地测试通过结论，不能把此环境阻塞算成这三个 PR 的回归失败或成功。** 未启动真实应用、读取真实账号凭据或修改用户数据库；Deno、Windows/macOS 桌面端到端及真实 provider 测试未运行。

原建议按 #424 → #423 → #421 验证和整合。截至 9 月 30 日，#424 和 #423 均已修复、验证、合入并发布，本次 #421 已在独立分支补齐并通过服务端验证（`a5de72d`），尚未真实 UI 测试、合入或发布，完成数保持 2 项。#423 原补丁的重试及客户端错误展示问题已补齐；无需再单独合入原 #423。此处的完成标记指个人 fork 中的问题已解决，不代表上游 PR 已合并。

## 重叠改动与定制保护

| PR 组 | 共同影响 | 整合建议 |
| --- | --- | --- |
| #420 / #447 | MCP schema 规范化 | 选择一份实现，避免重复修改 |
| #437 / #466 | Antigravity 额度、账号及插件 UI | 先比较能力，再处理互相覆盖的状态与翻译 |
| #459 / #483 | 插件凭据生命周期与共享 SDK、registry、state | 联合检查刷新、锁和失败恢复，不能只逐个看是否有文本冲突 |
| #479 / #483 / #437 / #466 | 插件管理 UI | 合入后一律回归原有插件；翻译冲突通过扫描生成解决 |
| #378 / #354 / #353 / #460 | 自动压缩与使用量阈值 | 统一 token 依据、重复压缩和 checkpoint 发布时序 |
| #345 / #474 | 本地规则存储及注入 | 保留现有规则，不引入重复注入或无提示删除 |
| #447 / #433 | Cursor 本地代理路由 | 分开验证 bidi/CLI 与通用转发、TAB、云端请求 |

本次没有修改下面这些目录；后续实际合入时必须保护：

```text
apps/desktop/
├── src/shell/                 # 保留无广告布局，不恢复已删除的 ads 模块
└── src-tauri/
    ├── tauri.conf.json        # 保留个人 fork 更新地址和已有公钥
    └── src/update/mod.rs      # 保留 Windows portable 更新源
server/
├── src/control/              # 不恢复广告接口
├── src/cursor/services/       # 保留规则与公益 TAB 服务行为
├── src/plugin/               # 账号和凭据生命周期，验证并发与持久化
└── migrations/               # 仅按迁移规则升级，以数据库副本验证
.github/workflows/release.yml  # 保留个人发布、签名和更新清单流程
.tauri/                       # 忽略的签名密钥目录，不读取或提交私钥
```

公益 TAB 地址 `https://tab.leokun.cn` 与广告分离，不因“无广告”而删除。此清单中的目录说明是后续核对范围，不表示每个 PR 都改到了它们。

## 后续维护

1. 同步最新 open PR 清单和各 head SHA；新增/变更 PR 重新审查，关闭/已合并项注明状态，不沿用旧结论。
2. 从当时的 fork HEAD 重新运行 merge-tree；此表的冲突结果不能永久沿用。
3. 真正合入前在隔离分支解决冲突、完成对应测试，确认无广告、更新源、签名和 TAB 定制。
4. 实际合入、发版与安装分别记录。该评估文档本身不表示已执行这些操作。

## 固定版本索引

完整文件清单、CI 原始状态/链接、冲突文件和评论链接见 [机器可读快照](history/pull-requests/2026-09-28.snapshot.json)。报告只保留审查元数据，不保存原始规则、凭据或私钥内容。

| PR | 本次评估 head SHA |
| --- | --- |
| [#483](https://github.com/leookun/cursor-byok/pull/483) | `c0f89f8be3f79ffed57141a0c68d5fdb523d3a12` |
| [#479](https://github.com/leookun/cursor-byok/pull/479) | `fa3fe67a3ddfa4619d7ee6bc6af1f0e78c702153` |
| [#474](https://github.com/leookun/cursor-byok/pull/474) | `24316e9642a8e6c3e048bb70a255c8200fe4f130` |
| [#466](https://github.com/leookun/cursor-byok/pull/466) | `1cfcf4a994f357f27426387344efc0c795dcc6dd` |
| [#460](https://github.com/leookun/cursor-byok/pull/460) | `98d57a0a605843a9f7d1a8af441772935520d831` |
| [#459](https://github.com/leookun/cursor-byok/pull/459) | `c751da4cf087af3de4cd53a2464d545035961be4` |
| [#458](https://github.com/leookun/cursor-byok/pull/458) | `5b63b8586a22975114e955933022b1f3c64a9de4` |
| [#447](https://github.com/leookun/cursor-byok/pull/447) | `4dfabeeadd0c4e01fa6cb7761cbcb8d74b7dc6ae` |
| [#437](https://github.com/leookun/cursor-byok/pull/437) | `c5d5de08ff922977ecf023d81ee3534d1454157b` |
| [#433](https://github.com/leookun/cursor-byok/pull/433) | `a5b1ce085d6a48e9fa83866f49dbb198ae753c70` |
| [#424](https://github.com/leookun/cursor-byok/pull/424) | `9def2906ee87baa3c127662c560044bb40e480d7` |
| [#423](https://github.com/leookun/cursor-byok/pull/423) | `ba7bf466b3855c26b7c0afa84a5030f97c005a6d` |
| [#421](https://github.com/leookun/cursor-byok/pull/421) | `8c3cedff1a615b16ac963601b8bb964090056104` |
| [#420](https://github.com/leookun/cursor-byok/pull/420) | `4d51f9714057f83c347877e436ebc5c7190f4da6` |
| [#378](https://github.com/leookun/cursor-byok/pull/378) | `1b29c79f1baac8295141f7e96e429b803980d2a1` |
| [#375](https://github.com/leookun/cursor-byok/pull/375) | `50620d9ac5f31b8d57c4e56a05e4f7bfe88c0df8` |
| [#364](https://github.com/leookun/cursor-byok/pull/364) | `386547647f31c16e48f09cbd887898f0294c0a8e` |
| [#360](https://github.com/leookun/cursor-byok/pull/360) | `8ac072994ba9db21407c313335f231d113c6ba49` |
| [#354](https://github.com/leookun/cursor-byok/pull/354) | `b8c7696c7514286dce17cf0eb5a1457a4aecce94` |
| [#353](https://github.com/leookun/cursor-byok/pull/353) | `1583059aa177dcf1ce15e674c0b8791bb288afd3` |
| [#350](https://github.com/leookun/cursor-byok/pull/350) | `c3b2cc54e9705594fff917e755a5eb7d7caa766b` |
| [#345](https://github.com/leookun/cursor-byok/pull/345) | `76884f15e9c9e9c3ced0039761fa17fd0d8bdac2` |
| [#343](https://github.com/leookun/cursor-byok/pull/343) | `cfc333dde4643d1285a402dfbc42cb6e9ddcd13c` |
| [#312](https://github.com/leookun/cursor-byok/pull/312) | `0185d15a93b95b1113d155116bca4fd4b413dbe6` |
| [#293](https://github.com/leookun/cursor-byok/pull/293) | `e3f26f8fec05dbd40153494783aa261d9016d823` |
| [#245](https://github.com/leookun/cursor-byok/pull/245) | `a29f7190b2cdc724515cfe55c3ee481b41cc321c` |
| [#237](https://github.com/leookun/cursor-byok/pull/237) | `ef060ea576d798d8de1cb6d5ab2776853ba1aa36` |
| [#229](https://github.com/leookun/cursor-byok/pull/229) | `9e1a27fe8d8de804bc7c4a48baefa3dde96117e7` |
| [#227](https://github.com/leookun/cursor-byok/pull/227) | `312cc559dd40ff5a64f3d5afbb5dba5eb9a93e06` |
| [#216](https://github.com/leookun/cursor-byok/pull/216) | `d763b179563ec1fe603c650e8c9291dcaaf8e1f2` |
| [#211](https://github.com/leookun/cursor-byok/pull/211) | `d280adae51f691aaa1eb44240416f55d90084d05` |
| [#210](https://github.com/leookun/cursor-byok/pull/210) | `f9792dc1fb88b50c2557a89d84a62cbe2c03a5a1` |
| [#208](https://github.com/leookun/cursor-byok/pull/208) | `0a3c5ffd21c9a3ff12bfeb334f6e576773ccaaa4` |
| [#198](https://github.com/leookun/cursor-byok/pull/198) | `6b1bca7efc308b28276af0ecfab8cd4524f141f7` |
| [#196](https://github.com/leookun/cursor-byok/pull/196) | `f5c76d330068d5907c04eb0b9efc63a587009592` |
| [#194](https://github.com/leookun/cursor-byok/pull/194) | `4dff94b95b7132311119cc0e1b0fe7fd0766e2b3` |
| [#193](https://github.com/leookun/cursor-byok/pull/193) | `3e5e035043bb8b04393c10a8f93ad82e687214cf` |
