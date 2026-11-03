# 工作状态

更新：2026-11-03；功能分支与测试结果的核对日期见各节。本文只记录本地工作交付状态；逐项调查见 [Issue 清单](issues.md)，原始测试见 [历史资料](history/README.md)。

**结论：已有一批修复进入 main；远程 Semble、Grok、GPT/Google OAuth 三条开发分支还没有合入。最新的 Semble 审批修复已写好并通过本机测试，但仍未提交，真实审批尚未验收。**

## 1. 已发布的成果

既有发布与验收证据见 [维护状态](maintenance-status.md)，本次没有发布新版本。

| 工作 | 已完成部分 | 保留的边界 |
| --- | --- | --- |
| Responses 文字缺失、子代理启动及结果交付 | #410、#477、#340 的具体缺口已修复，分别有自动化或真实 Cursor 验证，进入已有版本 | 不代表所有供应商、Windows 原环境和后台 Shell 边界都覆盖 |
| 子代理模型选择与中断历史 | #449 模型名单/选择已获用户验收；#172 中断后的文字与已完成工具保留修复已合入已有版本 | #172 相关实机边界继续以原记录为准 |
| #155 后台命令结束后不续答 | v1.0.17 已包含服务端修复；普通 25 秒后台任务自动续答及完成通知去重重放已通过 | 按用户要求仍为“完成一半”；真实审批等待、压缩期间完成等未完整验收；不做 20 分钟测试 |

## 2. 已合入 main 的近期工作

2026-11-01 通过 GitHub API 只读核对：远端 `main` 为 `a9c737bc815ad1bc82b05f3d7a5c34b6f7870806`，与本地 main 一致。源码合入状态不能替代安装包发布状态。

| 工作 | main 中的结果 | 证据 |
| --- | --- | --- |
| #511 SQLite 清理后文件不缩小 | 临时库复现后修复空间回收、并发及失败恢复；PR #1 已合入 | [修复与测试](history/issues/2026-10-27-issue511-storage-fix.md)；原作者大库的增长来源尚未定位 |
| #504 重复用户消息中断工具 | 同一消息 ID 在排队前去重，新 ID 的相同文字仍作为新输入；PR #2 已合入 | [修复与边界](history/issues/2026-10-27-issue504-input-redelivery-fix.md)；不声称原报告全部症状都解决 |
| #337 排队消息、子代理思考参数 | 补充协议与实际 HTTP 请求回归；PR #3 已合入 | [调查记录](history/issues/2026-10-27-issue337-check.md)；仅测试与文档，受控场景未复现，无新生产修复 |
| #373 压缩失败后继续对话 | 补充压缩失败、重试、恢复和结束帧判断测试；PR #5 已合入 | [调查记录](history/issues/2026-10-27-issue373-check.md)；仅测试与文档，原 UI 待复测 |
| 上游 PR #515 子代理消息身份方案 | 保留候选实验和三项失败证据，未采用原补丁；本 fork PR #7 已合入记录 | [评审记录](history/pull-requests/2026-10-28-pr515-check.md)；不是合入上游 #515 的代码 |

其他此前已进入源码的修复（系统代理缓存、SQLite 事务恢复、缓存键、MCP schema 等）继续见 [维护状态](maintenance-status.md)，本轮不改写它们的验收结论。

## 3. 已提交、已推送，尚未合入的分支

下面的远端 head、PR 状态与 base 均于 2026-11-01 经 GitHub API 核对。两个 OAuth 工作树为干净状态，没有待提交文件。

| 分支 / PR | 当前提交 | 已完成 | 还缺什么 |
| --- | --- | --- | --- |
| `codex/remote-semble` / PR #8，OPEN、Draft，base 为 main | `fd5ca63` | 远程主机采集、请求快照、忽略规则、分页、多批及 Python 导入隔离；此前真实 SSH 的两种查询、大目录和取消通过 | 本地新增审批修复未提交；修复后真实批准/拒绝、Windows Cursor UI 待验收 |
| `codex/issue445-grok-refresh` / PR #4，OPEN、Draft，base 为 main | `e873a23` | Grok 自动续期、手动刷新、同账号串行、凭据轮换保存、取消与 reset；合成网络、实际 Deno 宿主及跨平台 CI 已有记录 | 没有真实 Grok 测试账号；未合入、未发布 |
| `codex/issue481-oauth-refresh` / PR #6，OPEN、非 Draft，base 为 Grok 分支 | `9951089` | GPT/Google 续期、401 有限重试、冷却、旧结果不覆盖新授权、Google 手动配额刷新补修 | 依赖 PR #4 的插件 API 2；没有真实 GPT/Google 测试账号；未合入、未发布 |

OAuth 详细记录保存在尚未合入的分支：[Grok 固定提交记录](https://github.com/haohaomin/cursor-byok/blob/e873a23f146497a2a68ebe25d89c7d26d85875ab/docs/upstream-issue-reviews/2026-10-27-issue445-check.md)、[GPT/Google 固定提交记录](https://github.com/haohaomin/cursor-byok/blob/995108926cde18d4ba31523ea6c59dad16c3df76/docs/upstream-issue-reviews/2026-10-28-issue481-check.md)。链接保留这些提交当时的目录；本次只整理当前工作树，没有修改两个 OAuth 工作树。

## 4. 上轮新增、尚未提交的审批修复

真实 Cursor **3.27.22** 的 SSH 测试已发出一次 SembleSearch。BYOK 收到的唯一 Exec 事件是 `rejected: Review cancelled or failed`，没有 Start/Exit。用户没有点击拒绝，因此这次不能算“拒绝验收通过”。请求与两次模型调用均正常结束，见 [脱敏证据](history/issues/2026-10-30-issue469-approval-evidence.json)。

代码检查发现：Semble 显示为 MCP 卡片，内部采集 Shell 却复用其调用 ID；安装客户端的终端审查路径只为终端卡片创建 review model。新增回归在基线确实失败，候选修复后通过。静态分析支持这一原因，但修复后真实 Cursor 是否完整展示审批仍须实测。

```text
server/src/cursor/
├── tools/semble/collection.rs       # 新增：每次采集的独立终端卡片与执行请求
├── tools/semble.rs                  # 采集阶段推进，只生成一次父搜索结果
├── tools/runtime.rs                 # 执行所有权；取消时终止采集并关闭卡片
├── tools/codec/response.rs          # 多条客户端呈现事件与断流处理
├── tools/tool_call_dispatch/search.rs # 启动时先发终端卡片，再发 Exec
├── tools/mod.rs                     # 传递取消产生的呈现事件
├── conversation/runtime.rs          # 向 Cursor 发送执行和卡片事件
├── conversation/output.rs           # 输出与中断路径的收尾
└── tools/semble/tests.rs            # 审批身份、拒绝、取消、重复事件回归
server/tests/tool_round.rs            # 断流错误结果的回归适配
```

这部分包含 **8 个生产代码文件、2 个测试文件**，保留在 `codex/remote-semble` 的本地工作区；文档整理单独提交，不包含这些代码修改。终端卡片先于 Shell 请求，ID 与 MCP 卡片不同；采集分页、完成、拒绝、取消和断流都有收尾；原始源码采集正文不额外进入模型工具结果。没有设置跳过审批，没有修改 Cursor 安装文件。

## 5. 已有测试结果

以下结果核对于 2026-11-01，取自当时保留的实际执行日志。2026-11-02 仅整理文档，没有重新运行程序测试。

| 检查 | 实际结果 | 能证明的范围 |
| --- | --- | --- |
| 新增审批身份回归，修复前 | 1 项失败，原因是 Shell 与 MCP 调用 ID 相同 | 已有代码缺口，不是凭推测加改动 |
| `cargo test --workspace` | **522 通过、0 失败、1 忽略** | 当前候选的工作区自动化 |
| 单独执行忽略的真实嵌入模型测试 | **1 通过**；普通及相关搜索两阶段采集、过滤均通过 | 真正运行主机命令和缓存嵌入模型，未经过 Cursor 审批 UI |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 | Rust 静态检查 |
| `cargo fmt --all -- --check`、`git diff --check` | 2026-11-01 重新执行，通过 | 当时工作区的格式、差异有效性 |
| 修复后 Cursor 点击批准 / 拒绝 | **未完成** | 后续界面工具报 ScreenCaptureKit `-3812`；不能用自动化代替 |

522 项结果属于 remote-semble 当前工作区，不能移作两个 OAuth 分支的全量通过数。此前跨平台 CI 只证明各自已提交版本，不覆盖这份未提交审批补丁。

## 6. 环境收尾与剩余事项

以下环境状态核对于 2026-11-01；2026-11-02 文档整理未再次操作 Cursor 或远端测试环境。

- 本机 Cursor 设置已按原始字节恢复，当时重新核对 SHA-256 与原备份记录一致。
- 远端临时 `cursorAgentHost.remoteInferenceRoute=always` 已撤销，Machine 设置恢复为原本不存在；当时 SSH 只读核对通过。
- 隔离服务已经停止，50745/50746 两个端口关闭；正式后端健康检查为 204。
- 已清理测试期间含凭据的临时数据库副本、设置备份、服务日志和重复 example；脱敏证据、测试日志及合成远端工作区保留用于复核。
- 测试时临时移除的 Command Allowlist `python3` 项：**尚无恢复确认**。它不保存在上述 settings.json 中，不能用设置文件摘要宣称已恢复。
- 当前审批候选仍保留在 `codex/remote-semble` 的本地工作区，未提交。README 与文档整理独立提交；文档进入 main 不代表 Semble 或 OAuth 的实现已合入，也没有发布新安装包。

后续顺序：确认允许列表恢复情况 → 在可操作的 Cursor 中验收候选审批（批准后只执行一次、拒绝不执行、等待不超时）→ 提交/推送并核对该提交的 CI → 决定合入。OAuth 分支等待可用测试账号；#155 的未覆盖边界独立保留，不以 #469 的搜索测试代替。
