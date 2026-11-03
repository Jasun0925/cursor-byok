# #421 验证与补齐修复

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

原 PR 不能直接照搬：它修复后续输出项漏字，但在交错事件、迟到终止事件及首次编号晚于文本出现时，会重复输出。已在隔离分支 `codex/verify-pr421` 补齐；尚未合入主分支或发布；10 月 1 日用户手动 UI 验证已有三项通过，工具执行及文本往返通过，但工具卡片误显示 Skipped 的原因未解决。

- 基线：个人 fork `3f1b202`（已发布 v1.0.5）。
- 上游：[PR #421](https://github.com/leookun/cursor-byok/pull/421)，作者 kevin9327，验证 head `8c3cedff1a615b16ac963601b8bb964090056104`。
- 查询时上游 PR 为 OPEN、MERGEABLE，4 项 CI 通过；不能以此替代本地行为验证。

## 实测对比

同一套 9 项测试经过回环 HTTP/SSE → 正式 OpenAiResponsesProvider → consume_model_cycle，检查最终文本、工具参数、结束原因和用量。使用本地模拟响应，没有请求真实供应商或改动已安装应用。

| 实现 | 通过 | 失败 | 主要问题 |
| --- | ---: | ---: | --- |
| 当前主分支 | 5 | 4 | 第二个文本项无法补全；交错/迟到项补全失败 |
| 原 PR | 6 | 3 | 切换输出项时清空累计状态，回到旧项时再次输出全文；首次显式编号也会丢失已有前缀 |
| 补齐修复 | 9 | 0 | 按输出项保存累计文本，首次显式编号认领已收到的无编号前缀 |

原 PR 的实际失败值：

- 迟到终止事件：预期 `firstsecond`，实际 `firstsecondfirst`。
- 交错事件：预期 `甲乙尾甲尾乙`，实际 `甲乙甲尾甲乙尾乙甲尾甲乙尾乙`。
- 首个 delta 无编号、后续编号为 2：预期 `hello world`，实际 `hellohello world`；这是原 PR 相对基线引入的回归。

交错和迟到事件属于合成边界测试，不表示已在真实 OpenAI 服务中观测到相同事件序列。

## 改动职责

```text
server/
├── src/provider/openai_responses.rs  # 607 行；仅修改 SSE 文本累计状态，生产差异 +26/-3
└── tests/responses_text.rs           # 211 行；9 项真实 HTTP/SSE 回归测试
```

行数包含注释和空行；生产文件与测试文件分别统计，不含生成代码。

单次响应内，文本累计值以 output_index 分开保存，直到响应结束才释放。流式 delta 仍按到达顺序立即发送，终止快照只补发该项尚缺的后缀。缺少 output_index 的事件沿用当前项；首次编号出现时，之前无编号的前缀归入该项。

不改变请求内容、历史序列化、checkpoint、数据库、工具协议或发布配置。没有重排已经发送的文本；多内容片段用 item.done 完整快照补齐的既有行为保留。原 PR 的三个辅助函数测试已由 HTTP/SSE 行为测试覆盖，没有保留重复的实现层测试。

## 测试覆盖

1. 仅终止快照、文本 → 工具 → 文本；含正常 completed 和完整 item.done 后 EOF 两种结束。
2. 后续项丢失 Unicode 后缀，重复快照不重放。
3. 同一项重复 text.done / item.done 不重放。
4. 多项交错到达，分别补齐自己的后缀。
5. 旧项迟到的 item.done 不重放。
6. 同一项的多个内容片段由 item.done 补齐。
7. 缺少 output_index 的后续事件保持当前作用域。
8. 空消息不影响后面的文本。
9. 首次显式编号认领无编号前缀。

## 最终验证

- `cargo test -p cursor-server --all-targets`：279 通过、0 失败，其中本次专项测试 9 项。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。
- `git diff --check`：通过。

本机编译使用命令级 `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`，并通过 `RUSTFLAGS=-C link-arg=-fuse-ld=/Users/renhao/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/gcc-ld/ld64.lld` 指定链接器；复用主目录 target 缓存，没有修改全局工具链设置。

结论：候选修复已通过服务端验证，仍待真实 Cursor UI 测试及后续合入、发布。清单完成数保持 2 项。

[结构化证据](2026-09-30-pr421-evidence.json) 保存对比结果及日志摘要；原始日志在 `/tmp/cursor-byok-pr421-20260930/`。


## 真实 UI 尝试（受阻，未通过）

2026-09-30 已构建候选服务端，并准备隔离 SQLite、临时 Cursor 工作区和本地 Responses SSE 模拟服务。通过 Cursor 界面打开了 `/tmp/cursor-byok-pr421-20260930/ui/cursor-workspace`，测试代理接通且模型目录请求到达隔离服务。

macOS 界面控制反复出现 ScreenCaptureKit `-3811`、`noWindowsAvailable` 和菜单 `elementHasNoFrame`。之后截图停留在旧画面，无法稳定选择测试模型或提交对话。模拟模型请求数为 0，真实供应商请求数为 0；四个 UI 场景均未取得通过结果。自动测试通过结论不受影响，但不能替代本轮 UI 验证。

已禁用并停止测试服务（53021/53022）及模拟服务（52821），移除临时真实模型配置，恢复正式服务（49588/49642）和 4 个原有模型。Cursor 设置文件与测试前备份逐字节一致。未合入、未发布。

[本次尝试及恢复证据](2026-09-30-pr421-ui-attempt.json)。恢复可靠界面控制后，从选择测试模型继续。


## 真实供应商连通性补测

第二次尝试真实 Cursor UI 时，菜单输入仍被 `elementHasNoFrame`、`noWindowsAvailable` 和 ScreenCaptureKit `-3812` 阻塞。通过窗口菜单切换到独立 Cursor Agents、恢复窗口尺寸后仍无法提交对话。

为推进独立验证，通过隔离服务的正式 Control API 模型测试入口，用既有 `gpt-5.6-sol` 供应商配置访问 `/v1/responses`（只在临时模型配置中切换协议）。实际返回数字 1～120，与预期字符串完全一致，无缺失或重复。耗时 16044 ms，首个有效响应 2557 ms，真实计费用量输入 30 / 输出 243 Token；SQLite 记录 1 次 `openai-responses` 调用、HTTP 200、completed / stop、无错误、140 个流事件。

该补测是实际供应商连通性测试，未经过 Cursor UI，也没有多轮追问；不能代替四个界面场景。详见 [真实供应商证据](2026-09-30-pr421-live-connectivity.json)。

补测结束已移除临时真实模型配置、停止测试服务并恢复正式服务（4 个模型）；Cursor 设置与本轮开始前逐字节一致，测试端口均关闭。


## 2026-10-01 用户手动 UI 验证

用户提供真实 Cursor 截图，后台调用记录与测试文件已核对。四个场景实际在同一对话中切换模型执行。

| 场景 | 结论 | 证据 |
| --- | --- | --- |
| 后续输出项补全文字 | 通过 | 第一段完整，第二段缺失后缀正确补齐，各出现一次；单次模型调用 completed |
| 迟到及重复结束通知 | 通过 | OLD_ONCE、NEW_ONCE 各一次；单次模型调用 completed |
| 真实供应商对话及追问 | 通过 | 先回复“已记住”，后正确答 42；两次真实 Responses 调用 completed |
| 工具往返 | 部分通过，待补测 | Shell 实际退出 0，标记文件仅一行；但界面显示 Skipped，完成文字出现两次 |

工具命令耗时 2174 ms，超过测试脚本的 `block_until_ms=1000`，Cursor 先回传 background-running，后发送 `background-completed` 通知，开启另一轮 Run。后台共 3 次模型请求：首次工具调用、工具结果后回答、后台完成通知后回答。后两次独立响应都由模拟服务返回相同完成文字，因此不能将截图重复归因于 #421 单次响应内的补全文本逻辑。界面 Skipped 与实际成功执行不一致，原因尚未确认。

仅调整临时模拟测试脚本：等待时间改为 10000 ms，标记文件改用 `tool-retest.txt`，保留第一次执行证据。待用户用新对话复测第三项；测试服务继续运行。生产修复代码未修改。

[结构化记录](2026-10-01-pr421-manual-ui.json) · [用户截图](assets/2026-10-01-pr421-manual-ui.png)


## 2026-10-01 工具补测结果

用户在新对话 `929fc0bc-a3e8-4df3-ba7e-a26d7e45a935` 补测：工具前后文本完整，完成文字只出现一次。后端恰好 2 次模型调用（tool_use → stop），均 completed / HTTP 200；Shell 结果 `shell completed without output`、is_error=false，`tool-retest.txt` 仅一行 `PR421_EXECUTED`。文本补全与工具实际执行、结果回传通过。

界面仍显示 `Skipped printf`，与成功执行的后台结果不一致。此问题应保留为待调查项，尚无证据判断是否由本次 #421 改动引入，不能把完整工具界面验证标记为全部通过。未合入或发布。

已停止测试服务和模拟服务，移除临时真实模型配置，恢复正式服务及 4 个原有模型；恢复校验见 [补测证据](2026-10-01-pr421-tool-retest.json)。

[补测截图](assets/2026-10-01-pr421-tool-retest.png)


## 2026-10-01 合入及发布准备

工具卡片 Skipped 已定位为 Cursor 展示事件顺序问题，由独立修复 `2616014` 解决；真实 Cursor 复测确认 Ran、客户端 success、命令仅执行一次，详见 [工具卡片修复记录](2026-10-01-tool-card-skipped-fix.md)。以上调查过程中的待确认结论保留为历史记录。

#421 文本补全修复已通过合并提交 `e08cab5` 合入个人 fork 的 main，与工具卡片修复共同发布 v1.0.6。组合后的服务端 287 项测试、前端检查及 Tauri debug no-bundle 构建通过。GitHub CI、四平台发布及 7 个更新包签名验证均通过。此状态不表示上游 PR #421 已合并。最终发布结果见 [v1.0.6 发布记录](../releases.md#v106)。
