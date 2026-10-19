# #423：Anthropic 拒绝响应工具执行验证

> **后续修复已完成：** `89d90c6` 补齐拒绝终止和 Cursor 不可重试标记，270 项服务端测试通过；保存在隔离分支、尚未合入。详见 [修复记录](/Users/renhao/.codex/worktrees/task-result-delivery/cursor-byok/docs/upstream-pr-reviews/2026-09-30-pr423-fix-result.md)。以下保留原 PR 的历史验证结论。

**结论：防止执行的修复有效，但不建议原样合入。** 当前运行层会把“拒绝 + 工具”产生的协议错误作为可重试错误；持续拒绝时实测共请求 9 次（首次 + 8 次重试），约 40 秒后才失败。应先明确拒绝的终止语义，避免重复请求及额外用量，再合入。

## 基线与范围

- 上游 PR：[leookun/cursor-byok#423](https://github.com/leookun/cursor-byok/pull/423)，仍为 OPEN。
- 已核对 head：`ba7bf466b3855c26b7c0afa84a5030f97c005a6d`；上游该提交的 4 项 CI 通过。
- 个人 fork 基线：`0119cb44699a6657862cb81fd209456ea138055c`。
- 隔离工作树：`/Users/renhao/.codex/worktrees/task-result-delivery/cursor-byok`；分支：`codex/verify-pr423`。
- 原补丁只改 `server/src/provider/anthropic.rs`，+78/-15，无冲突；另补充 `server/tests/anthropic_refusal.rs` 共 7 项集成测试。
- 本次未合入主分支、推送、发版或替换安装包；主工作区仅新增验证文档并更新本地清单。

## 实际执行路径

```text
本地模拟 Anthropic HTTP 服务             # 输出固定的 SSE，不请求真实供应商
    ↓ HTTP /v1/messages
server/src/provider/anthropic.rs         # 解析工具块、refusal、完成原因
    ↓ ModelEvent
server/src/run/model_cycle.rs            # 校验完成原因和工具是否一致
    ↓ 成功则交付工具 / 失败则交付错误
server/src/run/engine.rs                 # 成功后进入执行；错误按重试策略处理
    ├── server/src/run/model_retry.rs    # 当前最多重试 8 次，每次等 5 秒
    └── server/src/cursor/               # 输出 Cursor 协议执行消息及结束帧
            ↓
模拟 Cursor 接收端                      # 应答 checkpoint KV，捕获 ExecServerMessage
```

所有运行数据使用临时 SQLite 数据库。模拟 Cursor 只捕获执行消息，不执行其中的 Read 请求，没有读取用户文件。

## 修复前后证据

| 场景 | 基线 | 应用原 #423 后 |
| --- | --- | --- |
| refusal + 工具，正常 message_stop | 错误允许工具通过 | 协议校验拒绝工具 |
| refusal + 工具，缺少 message_stop、已有 stop_reason | 同样错误允许工具通过 | 拒绝工具；覆盖内容块已关闭/未关闭 |
| refusal 后又出现 end_turn | 拒绝标志丢失 | 拒绝标志保留 |
| 完整 Cursor 协议链路 | 捕获到 `exec_id=refusal-tool` | 0 条执行消息，最终返回协议错误 |
| end_turn/stop_sequence/pause_turn/tool_use + 完整工具 | 正常通过 | 保持正常通过 |
| refusal 无工具 | 保留拒绝文本 | 保持文本，正常 Stop |
| max_tokens/model_context_window_exceeded | 拒绝不完整输出 | 保持拒绝 |
| EOF，既无 stop_reason 也无 message_stop | 报流错误 | 保持报错，不授权工具 |

红灯：7 项新增测试中 4 通过、3 失败，包含真实协议执行消息断言失败。绿灯：同一测试文件不变，应用原补丁后 7 项全部通过。

完整链路额外观测：`refusal transport requests=9, Cursor exec messages=0`，测试用时 40.21 秒。该测试证明拒绝的这一轮工具不执行；不代表系统立即停止请求，也不保证后续重试的模型仍会拒绝。

## 回归检查

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p cursor-server --all-targets -- -D warnings`：通过。
- 单元测试 159、`error_lifecycle` 7、`external_api` 12、`interrupt` 21、`tool_round` 18：共 217 项通过。
- 加上新增测试，本次绿灯合计 224 项，0 失败、0 忽略。
- `git diff --check`：通过。
- 使用命令级 MacOSX26.5 SDK 与 Rust 自带 ld64.lld，未修改全局工具链。

测试源码保存在上述隔离工作树。日志目录：`/tmp/cursor-byok-pr423-20260930/`（`red.log`、`green.log`、`regression.log`）；结构化摘要：[evidence.json](2026-09-30-pr423-evidence.json)。

## 合入建议与验证边界

保留 #423 的拒绝优先级处理；把拒绝明确传递给运行层，并使其作为不可重试的终止结果。不要只对通用协议错误文本做字符串匹配，也不要关闭所有协议/网络错误的重试。后续补充“拒绝只请求一次、工具执行为零”的回归后再合入。

这里验证了真实服务端代码和模拟客户端的协议链路，没有启动真实 Cursor UI、调用真实 Anthropic、重建桌面安装包或运行改动后的云端 CI。新增外部 API 的原生透传路径不经过 `model_cycle`；本 PR 也不负责约束外部客户端是否执行收到的工具块。
