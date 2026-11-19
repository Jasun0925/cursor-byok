# Issue 清单

整理：2026-11-04；本轮合入与 v1.1.0 发布状态已核对。上游元数据以 [2026-10-27 快照](history/issues/2026-10-27.snapshot.json) 为准，当时开放 104 项；保留首次收集的 100 项和后续 6 项，关闭或无法访问不等于本地已解决。

- **先看进度**：[已做好与未完成工作](work-status.md)。
- **看安装包包含什么**：[维护状态](maintenance-status.md)。
- **看调查依据**：[历史资料](history/README.md)，旧报告中的“当前”仅指其记录日期。
- `☑` 只用于已有验证证据的本地完成项；`◐` 表示部分完成；`☐` 包含尚未复现、已写代码待验收和暂跳过，须结合“本地状态”阅读。
- 优先级 A/B/C/D 分别为优先复测、常规排查、功能需求、信息不足。自动化、真实 Cursor、真实供应商、合入与发布分别记录。

## 当前待办

| 项目 | 已完成 | 剩余 |
| --- | --- | --- |
| #469 远程 Semble | 主机采集/分页等已随 v1.1.0 发布；本地审批候选回归通过 | 审批候选未提交；真实批准/拒绝及 Windows Cursor UI 待验收 |
| #445 / #481 OAuth | 实现与 API 2 已随 v1.1.0 发布，有隔离测试 | 真实账号验收 |
| #155 后台任务 | v1.0.17 普通短任务自动续答和去重通过 | 仍为完成一半；审批、压缩等边界未完整覆盖，不测 20 分钟 |
| #505 多 reasoning signature | 源码修复完成；checkpoint、模型流和 provider 回归通过 | 真实 Cursor/原网关待验收，见 [修复记录](history/issues/2026-11-06-issue505-fix.md) |

## 2026-10-27 新增问题

| Issue | 优先级 | 当前结论 | 下一步 |
| --- | --- | --- | --- |
| [#511](https://github.com/leookun/cursor-byok/issues/511) 本地SQLite(~/.cursor-byok-v3/cursor-byok.db)占用过大空间 | B | 清理后不缩文件已复现并修复；原库增长来源待取证 | 两种清理的物理空间回收及并发恢复已在临时库验证，见[修复与边界](history/issues/2026-10-27-issue511-storage-fix.md)。原截图主库 19.09 GB，内容构成未知；未清理正式库；修复已随 v1.1.0 发布。 |
| [#509](https://github.com/leookun/cursor-byok/issues/509) [bug]工具 cursor-app-control-move_agent_to_cloned_root 的 JSON Schema 被序列化为1.0 | A | 源码已修复，v1.1.0 已发布/原环境待验 | 现 main 已合入 #420 完整整数约束修复；v1.1.0 已包含，见 PR 合并记录。 |
| [#508](https://github.com/leookun/cursor-byok/issues/508) [Bug] 系统代理切换后所有出站请求瞬间失败（HTTP client 缓存了过期的系统代理配置） | A | 源码修复，v1.1.0 已发布/系统 UI 待验 | 旧路由已复现；三种客户端检测代理变化并重建，隔离 HTTP 路由通过。[修复及边界](history/issues/2026-10-27-issue508-fix.md)。 |
| [#505](https://github.com/leookun/cursor-byok/issues/505) Agent 一轮里有多段 reasoning signature 时失败：Cursor assistant has multiple reasoning signatures | A | 源码已修复，checkpoint、模型流和 provider 回归通过；真实环境待验收 | 多个 opaque/envelope signature 按顺序恢复，`ProviderReplayState` 不再因重复而失败；未取得原网关样本，见[修复记录](history/issues/2026-11-06-issue505-fix.md)。 |
| [#504](https://github.com/leookun/cursor-byok/issues/504) earlier user messages resent as part of new turn; agent repeats completed steps. | A | 重复交付中断工具已独立复现并修复；原报告整体待取证 | 同一 ID 重传在排队前去重，新 ID 的重复文字仍执行；历史恢复和 Write 对照通过，见[修复及边界](history/issues/2026-10-27-issue504-input-redelivery-fix.md)。不等同于原作者的重复执行、参数遗漏和错误记忆都已解决；已随 v1.1.0 发布。 |
| [#503](https://github.com/leookun/cursor-byok/issues/503) 这个是不是需要代理 | D | 待补信息 | 只有是否需要代理的提问；需环境、错误阶段和供应商入口。 |


## 完整清单

以下保留首次采集的 100 项调查记录。GitHub 状态以页首 2026-10-27 快照为准；本地修复状态与上游关闭状态分别维护。表中测试数量、版本和“当前代码”均对应所链接记录的验证日期，不代表今天重跑验收。

### 模型协议与供应商（16 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#482](https://github.com/leookun/cursor-byok/issues/482) cursor调用lm studio openai 本地模型出错 | A | MCP schema 缺口已修复并合入 main；实机待复测 | 缺省 properties 与空 schema 的编译/Responses HTTP 回归通过；当前 main 全量 281 项通过。模拟 LM Studio 相关校验，未证明原请求 tools.18 的唯一根因；真实 LM Studio 待复测，见 [修复记录](history/issues/2026-10-03-issue482-fix.md)。 |
| ☐ | [#318](https://github.com/leookun/cursor-byok/issues/318) bug: input_image 缺少必填的 detail 字段，严格校验后端（vLLM）返回 400，且会导致整个会话永久报废 | A | 当前代码已覆盖；回归通过 | `input_image` 及历史重放均保留 `detail:auto`；全量 309 项通过。未使用原 vLLM 现场，未合并发布，见 [复核记录](history/issues/2026-10-03-issue318-fix.md)。 |
| ☐ | [#441](https://github.com/leookun/cursor-byok/issues/441) 新版本cursor无法使用kimi k3 | B | 配置预设已补充；未使用真实 Moonshot API Key，原作者环境未现场复测 | Kimi Code 使用 `api.kimi.com/coding + k3`；Moonshot 公共 API 使用 `api.moonshot.cn/v1 + kimi-k3`。点击独立预设后检查生成配置，再用真实 Key 验证请求是否到达 BYOK；见 [修复记录](history/issues/2026-10-03-issue441-fix.md)。 |
| ☐ | [#429](https://github.com/leookun/cursor-byok/issues/429) 插件配置里面的谷歌模型没有最新的gemini 3.8 flash模型，只有3.7flash | B | 当前插件已包含 Gemini 3.8 四档；真实账号待复测 | `server/plugins/build-in/antigravity-auth/models.ts` 已声明 `gemini-3.8-flash-{high,medium,low,tiered}`，provider 也保留显式档位；无新增代码，见 [复核记录](history/issues/2026-10-04-issue429-check.md)。 |
| ☐ | [#310](https://github.com/leookun/cursor-byok/issues/310) deepseek-r1 模型中，会出现 openai adapter status=400 body={"error":{"message":"think value \"medium\" is not supported for this model","type":"api_error","param":null,"code":null}}， think这个字段不兼容，可以提供一个设置不传入的配置吗 | B | 已新增显式“不传入”设置；真实 DeepSeek API 待复测 | 保存 `reasoning_effort=none` 后清除 Cursor 请求的 effort，OpenAI Chat 请求不再写入 `reasoning_effort`；283 项服务端测试、桌面检查和 Clippy 通过，见 [修复记录](history/issues/2026-10-04-issue310-fix.md)。 |
| ☐ | [#294](https://github.com/leookun/cursor-byok/issues/294) 粘贴图片后，历史回放每轮重发 image 块，deepseek-v4-flash 文本模型，anthropic协议，每轮回复都提"图片依旧无法读取（Unsupported Image）" | B | 已新增模型级“移除图片输入”配置；真实 Cursor + Anthropic 文本模型待复测 | 开启后 provider-visible 历史会移除图片块，原始会话保留；服务端与桌面回归通过，见 [修复记录](history/issues/2026-10-04-issue294-fix.md)。 |
| ☐ | [#290](https://github.com/leookun/cursor-byok/issues/290) claude报错signature | B | 当前签名回放路径验证通过；原网关待复测 | 新增 6 项 HTTP/SSE 与 checkpoint 回归，相关 197 项通过；没有生产代码改动，未确认原 Bedrock 网关根因，见 [复核记录](history/issues/2026-10-04-issue290-check.md)。 |
| ☐ | [#199](https://github.com/leookun/cursor-byok/issues/199) 部分公益站grok4.5访问报错 | B | 本地 MCP schema 缺口已修复；原公益站待实测 | 顶层 `required:null` 规范化为空数组，Chat/Responses HTTP 回归通过；未确认原 Grok 网关根因，见 [修复记录](history/issues/2026-10-04-issue199-fix.md)。 |
| ☑（PR #36 已合入 main） | [#184](https://github.com/leookun/cursor-byok/issues/184) gemini模型不支持吗？ | B | 已增加 Google Gemini OpenAI 兼容预设，见 [PR #36](https://github.com/haohaomin/cursor-byok/pull/36) 和 [验证记录](history/issues/2026-11-13-issue463-gemini-openai-compat.md)；405 的原生协议路径仍不支持 | 使用 `generativelanguage.googleapis.com/v1beta/openai/chat/completions`；真实 Gemini Key 和原作者环境待验收。 |
| ☐ | [#88](https://github.com/leookun/cursor-byok/issues/88) 配置的OpenAi模型出现Instructions are required | B | 本地连通性测试缺口已修复；原中转站待实测 | 测试请求补充非空系统指令，严格/宽松 Responses HTTP 回归通过；见 [修复记录](history/issues/2026-10-05-issue88-fix.md)。 |
| ☐ | [#86](https://github.com/leookun/cursor-byok/issues/86) 响应出现&lt;think&gt;标签时出现异常中断 | B | 当前正文标签路径验证通过；原官方 API 待实测 | 新增 6 项回归确认完整/跨分片/未闭合标签与后续正文保留，断流明确报错；无生产代码改动，见 [复核记录](history/issues/2026-10-05-issue86-check.md)。 |
| ☑（PR #36 已合入 main） | [#463](https://github.com/leookun/cursor-byok/issues/463) Gemini Provider Support | C | 已增加 Gemini OpenAI 兼容预设，见 [PR #36](https://github.com/haohaomin/cursor-byok/pull/36) 和 [验证记录](history/issues/2026-11-13-issue463-gemini-openai-compat.md)；原生 `generateContent` 协议仍未实现 | 使用 `generativelanguage.googleapis.com/v1beta/openai/chat/completions` 和 Bearer API Key；真实 Gemini Key 待验收。 |
| ☐ | [#470](https://github.com/leookun/cursor-byok/issues/470) 谷歌模型的gemini 3.1pro模型也不能用 | D | 待补信息 | 主要证据为图片；补协议、请求地址类型和服务端错误。 |
| ☐ | [#440](https://github.com/leookun/cursor-byok/issues/440) 谷歌里面的claude模型用不了 | D | 待补信息 | 补 Google 插件下具体 Claude 入口、授权状态和错误正文。 |
| ☐ | [#379](https://github.com/leookun/cursor-byok/issues/379) Grok Responses API 的后台工具任务完成回调时，BYOK 代理组装回包缺少必填的 `tool_call_id` 字段，xAI 服务端直接参数校验失败 | D | 待补信息 | 正文为空；补缺失 tool_call_id 的请求和后台完成通知，勿只凭标题改协议。 |
| ☐ | [#110](https://github.com/leookun/cursor-byok/issues/110) minixmax3 适配问题 | D | 待补信息 | 补模型准确名称、协议、finish_reason 和错误正文。 |

### 子代理与模型继承（9 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#477](https://github.com/leookun/cursor-byok/issues/477) 子代理执行的时候卡住，一直卡在Planning next moves 子代理改成官方原版 模型 就可以正常执行子代理 | A | 已修复；自动化与 Mac 单子代理 UI 通过 | 单父头 400 与缺少消息 ID 的协议回归通过；302 项测试通过，Mac 单子代理 UI 通过。Windows 缺省 ID 未现场复测；已合入 main，v1.0.8 发布已触发，完成状态未查询，见 [修复记录](history/issues/2026-10-02-issue477-fix.md)。 |
| ☑ | [#449](https://github.com/leookun/cursor-byok/issues/449) [bug] Cursor tools 中的 subagent 模型列表写死而不是从用户的 Client Message 中读取 | A | 已解决；用户确认，已合入并触发 v1.0.12 发布 | 本次显式模型 → Cursor 默认 → 父模型，保留禁用限制。344 项测试通过；用户本轮确认“这个解决了”，未另行采集新优先级的真实调用证据；见 [修复记录](history/issues/2026-10-08-issue449-fix.md)。 |
| ☐ | [#340](https://github.com/leookun/cursor-byok/issues/340) (V0.1.1)Cursor Multitask模式下，子任务完成直接Stopped中断停止了，父任务也没有继续 | A | 已合入；v1.0.9 发布已触发 | 两轮后台 Task 自动唤醒父代理；6 个 Run、10 次模型请求成功。实际携带工具 ID，缺省分支由自动化验证；全量 305 项通过。正式环境已恢复。已合入 main（d950c39），用户要求不监视发布；未证明原报告唯一根因，见 [修复记录](history/issues/2026-10-03-issue340-fix.md)。 |
| ☐（协议未复现） | [#337](https://github.com/leookun/cursor-byok/issues/337) beta10还存在的一些问题 bug | B | 当前队列/Task 协议与思考参数 HTTP 对照通过；客户端待验 | 新增四项回归；23 项中断/队列测试及两协议共 10 次实际 loopback HTTP 请求通过。未改生产逻辑，未证明 Cursor 标签或父子参数继承正确，见[复查记录](history/issues/2026-10-27-issue337-check.md)。 |
| ☐ | [#311](https://github.com/leookun/cursor-byok/issues/311) 不会自动切换 便宜的模型来搜索文件吗 | C | 需求待评估 | 先明确用户选择与自动选择便宜子模型的规则，再评估策略。 |
| ☐ | [#251](https://github.com/leookun/cursor-byok/issues/251) 支持在 Cursor-byok 中配置自定义系统提示词面板（prompt override）以及自定义subagent无法使用 | C | 需求待评估 | 拆分自定义子代理角色丢失与提示编辑面板两项需求。 |
| ☐ | [#244](https://github.com/leookun/cursor-byok/issues/244) [Feature] Orchestration 工具（create-agent/AWAIT）、Projects 模式、按 Cursor 3.14.7 重同步 prompt/proto | C | 需求待评估 | 旧 0.0.42 的多功能需求；按当前协议逐项核对，避免整体重做。 |
| ☐ | [#346](https://github.com/leookun/cursor-byok/issues/346) The thinking Effort of the child proxy should comply with the configuration. | D | 待补信息 | 与 #449/#118 归组；补主/子模型配置及实际请求参数。 |
| ☐ | [#118](https://github.com/leookun/cursor-byok/issues/118) 子代理的模型未遵守用户配置，经常选的是fast | D | 待补信息 | 与 #449/#346 归组，补子模型配置及实际 fast 参数。 |

### 对话、工具与上下文（28 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☑ | [#410](https://github.com/leookun/cursor-byok/issues/410) 让agent对一个文件进行review，过程看起来貌似正常，但是cursor里没有最终的文字输出 | A | 本地缺口已解决、合入并发布 v1.0.7 | 7037431 已合入 main；294 项测试及四项真实 UI 通过。上游原场景根因仍未确认。见 [修复记录](history/issues/2026-10-02-issue410-fix.md)。 |
| ☐ | [#472](https://github.com/leookun/cursor-byok/issues/472) 中断运行 | B | 评论报恢复，待现版复测 | 评论称更新 Cursor 后恢复；先核对当前版本，再复现复杂命令/长上下文中断。 |
| ☐ | [#444](https://github.com/leookun/cursor-byok/issues/444) 使用grok free api接入，压缩后 基本完全忘记之前的事了 | B | 待现版复测 | 记录压缩前后摘要与 provider 历史，区分模型能力和上下文丢失。 |
| ☐ | [#341](https://github.com/leookun/cursor-byok/issues/341) 回滚失效 | B | 已检查：两个入口手动实测通过，当前简单场景未复现 | 用户确认 Undo、Restore Checkpoint 均正常；2026-10-11 首次磁盘核对两个文件均为 value=1。后续额外编辑单独记录，未将其误判为回滚失败。未修改生产代码，原失败条件与隔离探针缺口仍保留；见 [新版补充验证](history/issues/2026-10-11-issue341-check.md)及[实机证据](history/issues/2026-10-11-issue341-live-evidence.json)。 |
| ☐ | [#302](https://github.com/leookun/cursor-byok/issues/302) 模型读 terminal 输出漏 .txt 后缀：Read 找不到文件 / Model drops .txt suffix when reading shell terminal files | B | 2026-10-10 协议检查通过；实际漏后缀待复现 | 当前提示词已有 $id.txt 说明，完成通知保留完整 output_path；初始后台结果返回目录和编号。5 项定向回归通过，Cursor 空白窗口及 -3812 阻断实测，未改生产代码；见 [复核记录](history/issues/2026-10-10-issue302-check.md)。 |
| ☐（已检查，暂跳过） | [#301](https://github.com/leookun/cursor-byok/issues/301) Shell 命令输出被吞 — 客户端显示空白气泡 / Shell output swallowed in local mode | B | 2026-10-10 已标记检查完成；协议未复现，UI 未验证 | 11 KB / 66 KB 单行中文 JSON 实时增量完整，最终卡片、历史回放与模型结果非空；超长结果保留首尾和截断提示。9 项展示回归通过，UI 被锁屏阻断；见 [复核记录](history/issues/2026-10-09-issue301-check.md)。 |
| ☐ | [#300](https://github.com/leookun/cursor-byok/issues/300) AwaitShell 后台 Shell 完成后未立即返回 / AwaitShell waits after background shell completes | B | v1.0.13 协议测试未复现；真实 UI 待测 | AwaitShell 已移除，数字/字符串 ID 均立即失败；成功 Shell 通知在 5–35 ms 调用模型，另有约 2 秒连接收尾，5 项定向回归通过。macOS -3811 阻断 UI，未测进程退出至客户端通知的延迟；见 [复核记录](history/issues/2026-10-09-issue300-check.md)。 |
| ☐（暂后置） | [#297](https://github.com/leookun/cursor-byok/issues/297) 工具在长对话环境中的性能严重下降 | B | 用户选择以后日常使用时观察；长对话性能未复现 | 构建阻断已解除，6 项压缩回归通过；以后出现卡顿再分离模型、压缩、检查点与 Blob ACK 耗时，不记为修复完成；见 [调查记录](history/issues/2026-10-12-issue297-check.md)。 |
| ☑（检查完成） | [#288](https://github.com/leookun/cursor-byok/issues/288) 切模型的问题 | B | 用户验收：当前模型组合未复现 | gpt-6-astra → gpt-5.6-sol 两轮真实 MCP 调用通过；补齐浏览器依赖后均返回 about:blank。用户接受并要求下一项，切回 A 未测；未改生产代码、未关闭上游。见 [调查记录](history/issues/2026-10-11-issue288-check.md)。 |
| ☐（暂跳过） | [#282](https://github.com/leookun/cursor-byok/issues/282) CLI --endpoint 本地模式丢失 skills/rules｜CLI --endpoint local mode omits request_context skills/rules | B | 2026-10-14 用户确认不用 CLI，暂不处理 | 尚未真实复现；以后需要 CLI 时再对比非交互模式与 GUI 的 request_context。前次只确认 PATH 中未找到 agent，不能据此断言本机未安装。 |
| ☐（当前模型未复现） | [#258](https://github.com/leookun/cursor-byok/issues/258) bug : getting response in chinese | B | 用户确认无需继续；当前模型四轮保持英文 | Cursor 3.24.18 + gpt-6-astra，无工具、内置浏览器、读取中文页面及 Playwright 路径均未切换中文；4 个 run、8 次模型调用正常完成。2026-10-15 停止推进，未改代码，原 GPT-5.6 Luna 未配置，见 [复测记录](history/issues/2026-10-14-issue258-check.md)。 |
| ☐（暂跳过） | [#223](https://github.com/leookun/cursor-byok/issues/223) [Bug]: 新会话标题不会由 AI 自动生成，NameTab RPC 返回 404 | B | 2026-10-14 用户认为不影响使用，暂不处理 | 当前标题请求走官方服务；静态初查不能证明现版标题失败，尚未 UI 复现，见 [初查记录](history/issues/2026-10-14-issue223-check.md)。 |
| ☐（当前模型未复现） | [#214](https://github.com/leookun/cursor-byok/issues/214) opencode-mimo-v2.5 中途容易没执行完所有todo就停下 | B | 2026-10-14 真实 Cursor + gpt-6 High 完成全部待办；原模型待验证 | 一次输入自行执行 13 轮，4/4 todo 完成，18 项 unittest 通过；4 项协议边界探针通过。未改生产代码，未标记修复，上游仍 OPEN，见 [复测记录](history/issues/2026-10-14-issue214-check.md)。 |
| ☐ | [#192](https://github.com/leookun/cursor-byok/issues/192) 配置的文件似乎调用工具有问题 | B | 待现版复测 | 补工具定义、调用参数和响应，先确认模型是否具备对应工具能力。 |
| ☐（协议未复现） | [#174](https://github.com/leookun/cursor-byok/issues/174) 上游异常导致重试时会重复附加system_reminder块 | B | 2026-10-14 HTTP 重试及恢复共 5 项通过；UI 待验 | 两次 503 后三份请求正文逐字节一致，模式提示保持 2 段；4 项 #172 恢复回归通过。未改生产代码，未标记已解决，见 [复测记录](history/issues/2026-10-14-issue174-check.md)。 |
| ☐ | [#172](https://github.com/leookun/cursor-byok/issues/172) 中途打断对话后，继续对话会丢失模型该轮对话模型已经输出及toolcall获取到的内容 | B | 代码已修复；服务端 348 项通过，真实 UI 待验收 | 中断文字持久化并发布检查点；保留已完成结果，仅取消未完成工具；连续失败重试保持历史前缀。尚未合并或发布，保持未勾选，见 [修复记录](history/issues/2026-10-08-issue172-fix.md)。 |
| ◐（完成一半） | [#155](https://github.com/leookun/cursor-byok/issues/155) shell 超时 | B | 普通后台自动续答及去重重放通过；审批、压缩实机边界待覆盖，20 分钟按用户要求跳过 | 2026-10-19 当前补丁普通后台 25 秒实机自动续答通过；请求沙箱权限的第二组也完成，但没有审批弹窗，审批等待未覆盖。两条真实完成通知经单条、合批、倒序、拆批重放，模型调用总数保持 6、历史不变。界面输入故障导致压缩测试未发出；最终补丁 notify_on_output 与官方→BYOK 转入未重测。既有工作区 426 项回归通过。正式代理已恢复，正式库保持版本 10；未替换正式应用；修复已提交 main（2df03e3），v1.0.17 已发布，资产及更新清单核验通过。见 [验收报告](history/issues/2026-10-19-issue155-acceptance.md)、[验收证据](history/issues/2026-10-19-issue155-acceptance-evidence.json)、[补修报告](history/issues/2026-10-18-issue155-audit-fix.md)。 |
| ☐ | [#102](https://github.com/leookun/cursor-byok/issues/102) 之前的那个issue关闭了，日志输出如下 | B | 待现版复测 | 正文报告 Responses id/call_id 混用；在当前历史回放代码复现，不归入文本补全修复。 |
| ☐（协议缺口已修复 / UI 未复现） | [#55](https://github.com/leookun/cursor-byok/issues/55) AGENTS.md作用域问题 | B | 2026-10-14 已保留规则来源和范围；原 UI 行为未复现 | 原 2 项失败转通过，新增共 7 项回归；相关 211 项、Clippy 与格式检查通过。此前真实前后端三轮规则正确，修改后未重跑 UI；已合入 main，v1.0.15 发布已触发，未监视结果。见 [修复记录](history/issues/2026-10-14-issue55-fix.md)。 |
| ☐ | [#327](https://github.com/leookun/cursor-byok/issues/327) 最新版beta5 版本缺少CreateGoal 工具，无法使用目标模式 | C | 需求待评估 | 核对当前 CreateGoal 工具声明与执行能力，旧 beta 的缺失需重新确认。 |
| ☐ | [#139](https://github.com/leookun/cursor-byok/issues/139) cursorplan模式可以直接改之前的计划 | C | 需求待评估 | 核对 Plan 编辑工具与当前协议能力，补修改已有计划的步骤。 |
| ☐ | [#123](https://github.com/leookun/cursor-byok/issues/123) debug模式稳定性提升，如服务启动，日志追踪链路 | C | 需求待评估 | 拆分 Debug 的服务启动和日志追踪需求，先给可验证的验收场景。 |
| ☐ | [#473](https://github.com/leookun/cursor-byok/issues/473) 内置读取文件工具不可用？ | D | 待补信息 | 主要证据为截图；补 Read 参数、返回结果及当前版本后复测。 |
| ☐ | [#373](https://github.com/leookun/cursor-byok/issues/373) Compressed requests that encounter errors may not be able to continue | D | 受控服务端未复现；原 UI 待复测 | 手动压缩失败重试、自动压缩后错误及新运行层恢复均通过；2 项新增回归、相关 260 项通过。原截图没有错误正文；仅补测试记录，未改生产代码或标记已解决，见 [复查记录](history/issues/2026-10-27-issue373-check.md)。 |
| ☐ | [#348](https://github.com/leookun/cursor-byok/issues/348) Official model did not record thought Effort and Fast | D | 待补信息 | 需补官方模型的 effort/fast 选择值与调用记录，判断是统计还是路由问题。 |
| ☐ | [#296](https://github.com/leookun/cursor-byok/issues/296) 可能看不到历史的编辑 | D | 待补信息 | 仅截图提示编辑历史不可见；需补具体编辑工具和重放步骤。 |
| ☐ | [#126](https://github.com/leookun/cursor-byok/issues/126) 空壳mcp调用或者不积极调用 | D | 待补信息 | 正文为空；补 MCP 列表、工具声明及实际调用日志。 |
| ☐ | [#111](https://github.com/leookun/cursor-byok/issues/111) search web fail | D | 待补信息 | 主要证据为截图；补 WebSearch 工具参数和失败返回。 |

### 连接、代理与 Cursor 集成（24 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#469](https://github.com/leookun/cursor-byok/issues/469) v1.0.0 版本在更新 cursor 后无法在服务器上使用 | A | 基础路由/检索已实测；审批候选未提交、待实机验收 | v1.0.17 可用 [always 配置](remote-ssh.md)。已合入 main 的 WebFetch、采集、忽略规则、多批及大目录分页已有 [SSH 验收](history/issues/2026-10-27-issue469-paging-live.md)，[Python 导入隔离](history/issues/2026-10-28-issue469-python-isolation-fix.md)已修复。后续 [真实审批自动取消](history/issues/2026-10-30-issue469-approval-evidence.json)已复现；独立终端卡片候选自动化通过，批准/拒绝及 Windows Cursor UI 未验收，审批补丁未合入，采集实现已随 v1.1.0 发布。当前分支与测试边界见 [工作状态](work-status.md)。 |
| ☐ | [#465](https://github.com/leookun/cursor-byok/issues/465) The model you chose is not available | A | 作者报恢复，待独立复测 | 作者称合并 #433 后恢复；需复测无真实调用记录的问题，不能直接照搬该 PR 的广泛拦截。 |
| ☐ | [#478](https://github.com/leookun/cursor-byok/issues/478) 使用 multi model review 时候，无法选择官方模型+自定义模型 | B | 待现版复测 | 先复现 multi-model review 的模型列表和调用路由；区分官方/自定义模型限制。 |
| ☐ | [#467](https://github.com/leookun/cursor-byok/issues/467) 插件代理不可用 | B | 待现版复测 | 区分 Cursor 扩展网络、系统代理与 BYOK 代理，记录实际出口。 |
| ☐ | [#461](https://github.com/leookun/cursor-byok/issues/461) 功能无法使用 | B | 待现版复测 | 按模型列表→Agent 请求→代理入口→供应商调用逐段定位，与 #465 对照。 |
| ☐ | [#442](https://github.com/leookun/cursor-byok/issues/442) 突然不能用了 | B | 待现版复测 | 补最近一次正常/失败版本和代理日志，优先区分全模型故障与供应商故障。 |
| ☐ | [#415](https://github.com/leookun/cursor-byok/issues/415) skills显示为空的问题 | B | 待现版复测 | Remote SSH 的文件可读但 UI 空白；先区分 Cursor 展示与 BYOK 提示注入。 |
| ☐ | [#352](https://github.com/leookun/cursor-byok/issues/352) 新版本必须要安装证书吗 | B | 待现版复测 | 属于部署约束咨询；核对证书依赖和支持模式，不能直接禁用 TLS 校验。 |
| ☐ | [#317](https://github.com/leookun/cursor-byok/issues/317) 新版本怎么使用呢？配置了上游deepseek但是打开cursor没有一点反应 | B | 待现版复测 | 与 #461/#465 归组；确认请求是否进入代理及服务端。 |
| ☐ | [#203](https://github.com/leookun/cursor-byok/issues/203) 生成的代码块索引无法点击跳转到代码文件 | B | 待现版复测 | 复测代码引用协议和文件跳转，区分渲染链接与路径生成。 |
| ☐ | [#175](https://github.com/leookun/cursor-byok/issues/175) StreamableHttp Mcp 工具在ssh 远程服务器上不工作 | B | 待现版复测 | 需 Remote SSH 实测 Streamable HTTP MCP 的请求出口与代理透传。 |
| ☐ | [#480](https://github.com/leookun/cursor-byok/issues/480) BYOK 会话无法使用 GenerateImage；同一条 Agent 对话里「生图 + 写代码」需要混合路由 | C | 需求待评估 | 核对 GenerateImage 的服务端 executor 边界；混合路由涉及官方服务，先明确支持范围。 |
| ☐ | [#476](https://github.com/leookun/cursor-byok/issues/476) 各位大佬， 能不能支持 remote-control 中使用 api | C | 需求待评估 | 需补 remote-control 的使用入口和失败阶段，再评估 API 接入。 |
| ☐ | [#389](https://github.com/leookun/cursor-byok/issues/389) Auto review Support | C | 需求待评估 | 补 Auto review 的准确入口、预期流程和当前失败现象。 |
| ☐ | [#122](https://github.com/leookun/cursor-byok/issues/122) 支持同时生成多张图片 | C | 需求待评估 | 先明确 GenerateImage 的执行能力及多图返回协议，与 #480 归组。 |
| ☐ | [#121](https://github.com/leookun/cursor-byok/issues/121) 能否支持和cursor本身的模型共存 | C | 需求待评估 | 核对当前官方/自定义模型共存能力，进一步区分 Multi-model review 的 #478。 |
| ☐ | [#104](https://github.com/leookun/cursor-byok/issues/104) cursor3的Multi Root的Set up workspace能支持吗 | C | 需求待评估 | 明确 Multi Root workspace 设置与代理文件访问的边界。 |
| ☐ | [#45](https://github.com/leookun/cursor-byok/issues/45) Codebase Indexing功能似乎无法使用 | C | 需求待评估 | 评估官方云端索引与本地检索边界，先明确功能需求。 |
| ☐ | [#36](https://github.com/leookun/cursor-byok/issues/36) 希望能支持cursor原生的 ctrl k 功能 | C | 需求待评估 | 先核对 Ctrl+K 的 RPC 与当前支持范围，再复现终端/编辑器入口。 |
| ☐ | [#499](https://github.com/leookun/cursor-byok/issues/499) Connection failed. Please try again, orcontact support if the issue persists | D | 待补信息 | 仅通用连接报错及图片；补 Cursor/BYOK 版本、代理日志和复现步骤。 |
| ☐ | [#468](https://github.com/leookun/cursor-byok/issues/468) v1.0.0版本用不了 | D | 待补信息 | 主要证据为图片；先补日志和版本，不能仅凭“用不了”判断根因。 |
| ☐ | [#464](https://github.com/leookun/cursor-byok/issues/464) revuew之后,修改的文件没发查看 | D | 待补信息 | 先区分文件已改但 diff 不展示，还是编辑事件未到达；需补日志。 |
| ☐ | [#133](https://github.com/leookun/cursor-byok/issues/133) 3.6版本中此功能无法使用 | D | 待补信息 | 旧 Cursor 3.6，主要证据为图片；先补具体功能名与现版复现。 |
| ☐ | [#53](https://github.com/leookun/cursor-byok/issues/53) 为啥总是reconnect呢，是RPM不够吗 | D | 待补信息 | 补 RPM、HTTP 状态与重连时间，不能仅凭截图判断限流。 |

### 账号授权与额度（3 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#445](https://github.com/leookun/cursor-byok/issues/445) [Bug] Grok 登录令牌到期后不会自动续期，已保存的 refresh_token 未使用 | A | 源码已合入；真实账号待验收 | 基线续期探针、模型准备取消与不透明 token 手动刷新均先复现再修复。Grok 23 项脚本、最新 Linux/macOS 宿主各 37 项及 Windows 38 项通过；四组 CI 全通过，含详细日志认证重试回归。插件 API 2 同步适配三个内置插件；真实账号未验收，已通过 fork PR #4 合入 main，已随 v1.1.0 发布。见 [记录](history/issues/2026-10-27-issue445-check.md)。 |
| ☐ | [#481](https://github.com/leookun/cursor-byok/issues/481) GPT和Gemini账号令牌过期 | B | 源码已合入；真实账号待验收 | Codex/Gemini 改用 API 2 prepare；补齐 Google 手动刷新失效状态阻断、401 被吞及失败误报成功。最新 Linux/macOS 宿主各 41 项、Windows 42 项及 Google 16 项脚本通过，见 [复查记录](history/issues/2026-10-28-issue481-check.md)。PR #4 与 #6 均已合入 main，真实账号待验收，已随 v1.1.0 发布。 |
| ☐ | [#191](https://github.com/leookun/cursor-byok/issues/191) 额度显示 | C | 需求待评估 | 核对现版账号额度面板已有覆盖，按具体供应商补需求。 |

### 安装、桌面界面与数据（11 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ◐ | [#486](https://github.com/leookun/cursor-byok/issues/486) Linux RPM 无法在 Rocky Linux 9.5 安装：缺少 libwebkit2gtk-4.1.so.0 | B | 已补充 [Linux 安装与依赖说明](linux.md)；RPM 仍要求宿主机提供 WebKitGTK 4.1 | Rocky/RHEL/Fedora 尚未现场安装、启动和托盘回归；未通过删除 RPM 依赖来制造假修复。 |
| ☐ | [#436](https://github.com/leookun/cursor-byok/issues/436) Linux(fedora) 托盘右键菜单没有显示出来 | B | 待现版复测 | 需 Fedora 桌面环境实测托盘菜单，记录桌面协议及依赖。 |
| ☐（局部已修） | [#339](https://github.com/leookun/cursor-byok/issues/339) 亮色模式样式问题 | B | 分组布局 PR #30 与截断按钮 PR #31 均已合入，最终 CI 通过 | 分组布局的 12 组 60 次状态检查通过，见 [布局修复](history/issues/2026-11-06-issue339-model-test-layout.md)；截断按钮首次点击/键盘激活已先复现再修复，8 组页面回归及提示边界通过，见 [交互修复](history/issues/2026-11-06-truncated-button-interaction.md)。表格悬停穿透/数字箭头此前 Chromium 未复现，原生 GUI 待验。 |
| ☐ | [#323](https://github.com/leookun/cursor-byok/issues/323) beta版更新所有模型配置全部丢失 | B | 待现版复测 | 旧 beta 升级后配置丢失的报告；先识别数据目录/版本，仅用数据副本验证。 |
| ☐ | [#314](https://github.com/leookun/cursor-byok/issues/314) [Bug] macOS v0.0.49 首次启动后，通过菜单栏退出会导致应用卡死无响应 | B | 待现版复测 | 报告版本 0.0.49；先确认当前架构仍可复现首次启动退出卡死。 |
| ☐ | [#283](https://github.com/leookun/cursor-byok/issues/283) 与原账号切换问题 | B | 待现版复测 | 旧版账号/设置损坏报告；仅用数据副本测试接管与恢复，禁止用删除数据库作为修复。 |
| ☐ | [#256](https://github.com/leookun/cursor-byok/issues/256) windows有一定概率闪退 | B | 待现版复测 | 旧版 Windows 闪退日志；需现版崩溃堆栈和 Windows 环境。 |
| ☐ | [#371](https://github.com/leookun/cursor-byok/issues/371) follow the system theme and set it as default. | C | PR #26 已合入；浏览器、构建及四组 CI 通过，原生应用待验 | 默认跟随系统、实时切换、显式选择保存及首屏主题已补齐。7 组首屏检查、10 次运行时核对、四语言检查及带本机 SQLx 工具链参数的 Tauri debug 构建通过；未发布，见 [修复与边界](history/issues/2026-11-06-issue371-system-theme.md)。 |
| ☐（实现与局部实测完成） | [#134](https://github.com/leookun/cursor-byok/issues/134) 添加一个打开cursor 的快捷键按钮 | C | PR #27 已合入；macOS 原生分发、四语言布局及四组 CI 通过 | 复用 opener，仅允许 `cursor://`；普通浏览器隐藏。820×558 复测修复俄语工具栏挤出，原生权限失败及其他 URI 拒绝已验。冷启动、缺少 handler、Windows/Linux GUI 未验，见 [实现与验收](history/issues/2026-11-06-issue134-open-cursor.md)。 |
| ☐（已修复待原生验收） | [#119](https://github.com/leookun/cursor-byok/issues/119) 有个小建议，更新提示关闭之后就不要弹出来了，可以用户手动点击 版本更新 | C | 已复现刷新后重复提醒并修复；浏览器与构建通过 | 记住最后自动提醒的版本，新版本仍提醒，设置更新入口保留；刷新、定时检查、存储读写失败及最小窗口通过。真实安装包重启未验，跨 origin 不保留，见 [修复与边界](history/issues/2026-11-06-issue119-update-notification.md)。 |
| ☐ | [#370](https://github.com/leookun/cursor-byok/issues/370) Import modal scrollbar is not working | D | PR #29 已合入；浏览器、构建及最终提交四组 CI 通过 | 120 个模型时内部视口被撑高导致滚动失效；修正两层高度收缩，滚轮/拖拽、短内容、表格/详情回归通过。原报告正文为空，原生 GUI 待验；异步导入焦点已另行 [复现修复并测试](history/issues/2026-11-06-import-dialog-focus.md)，见 [滚动修复记录](history/issues/2026-11-06-issue370-modal-scroll.md)。 |

### 配置与统计需求（8 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#324](https://github.com/leookun/cursor-byok/issues/324) 使用上的一些小痛点。 | B | 待现版复测 | 拆分配置复制、密钥显示和模型哈希冲突；先复现冲突，避免混合改动。 |
| ◐ | [#471](https://github.com/leookun/cursor-byok/issues/471) 需求：增加一个配置备份功能 | C | 已实现内置模型 JSON 导出/导入；已补排序、并发刷新和大请求体回归；macOS 与 Oracle Linux 9.8 ARM64 定向验证通过 | 插件模型由插件配置独立管理，不进入本备份；导出包含 API Key，界面已提示妥善保管；真实打包桌面 UI 尚待人工验收。实现与验证见 [记录](history/issues/2026-11-13-issue471-model-backup.md)。 |
| ☑（PR #39 已合入 main） | [#417](https://github.com/leookun/cursor-byok/issues/417) 增加自定义定价 | C | 已实现全局及逐模型 Token 价格；[PR #39](https://github.com/haohaomin/cursor-byok/pull/39) 已在当前 `main` 基线上通过本地验证并合入 | 以 `model_hash` 保存输入/输出/缓存读写单价；未设置覆盖时使用全局价格。插件模型暂沿用全局价格，见 [实现与验证](history/issues/2026-11-13-issue150-model-pricing.md)。 |
| ☐（现版已支持） | [#254](https://github.com/leookun/cursor-byok/issues/254) 支持配置tab-server的baseurl，而不是内置一个 | C | 既有功能无需重写；路由、持久化及设置页受控验证通过 | 16 个 TAB RPC 的前缀/query/二进制转发、SQLite 重连、地址切换、流式交付与非法配置保护共 4 项测试通过；设置页接线已测。未修改生产代码；真实 Cursor TAB、HTTPS 服务及控制面账号未验，见 [核查与边界](history/issues/2026-11-06-issue254-tab-baseurl-check.md)。 |
| ☑（PR #34、#39 已合入 main） | [#150](https://github.com/leookun/cursor-byok/issues/150) 提几个意见看看需求能否实现。 | C | #107 按模型分项统计见 [PR #34](https://github.com/haohaomin/cursor-byok/pull/34)，逐模型 Token 定价见 [PR #39](https://github.com/haohaomin/cursor-byok/pull/39)；两者本地验证通过 | 首页总价和模型明细按 `model_hash` 选择覆盖价，其他模型使用全局价格；插件模型暂不单独定价。真实供应商、插件独立定价和发布版本仍待验收。见 [统计记录](history/issues/2026-11-12-issue107-model-usage-breakdown.md) 与 [定价验证](history/issues/2026-11-13-issue150-model-pricing.md)。 |
| ☐ | [#146](https://github.com/leookun/cursor-byok/issues/146) Docker支持 | C | 需求待评估 | 明确桌面/纯服务部署目标后评估容器支持。 |
| ☑（PR #34 已合入 main） | [#107](https://github.com/leookun/cursor-byok/issues/107) 请问能不能在用量统计时候加个模型的分类 | C | 概览四项统计可按模型展开；Store、工作区和浏览器回归通过 | Git 自动提交的本地模型/直连路径已核对，真实 Cursor Git UI 未验。 |
| ☐ | [#99](https://github.com/leookun/cursor-byok/issues/99) 关于批量添加模型和token用量统计的一点建议 | C | 需求待评估 | 先检查现版批量模型导入和统计；统计清零单独作为数据问题验证。 |

### 社区讨论（1 项）

| 完成 | Issue / 原标题 | 分级 | 本地状态 | 下一步 / 证据线索 |
| --- | --- | --- | --- | --- |
| ☐ | [#292](https://github.com/leookun/cursor-byok/issues/292) 这个工具好像被之前不少卖cursor破解的拿去二开了😢 | D | 讨论，不计修复完成 | 社区分发讨论，单独保留，不进入代码修复队列。 |


## 维护约定

更新本表和对应证据，不再新增按日期命名的主清单。历史数量、旧 CI、旧冲突判断不作为当前版本验收。PR 风险和处理状态见 [PR 清单](pull-requests.md)。
