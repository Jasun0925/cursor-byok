# 后台完成通知重复触发模型续写

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

在 #172 真实 Cursor 复测中发现：用户没有追加指令，复述口令结束后却自动生成先前要求的 50 段文字。

## 实测证据与根因

Cursor 对 Shell 任务 `128961`（生成随机口令，同一个 tool_call_id）发送了三条不同 request_id 的 `BackgroundTaskCompletionAction`。最后一次在口令复述完成后到达。BYOK 去重了持久化消息，却仍创建新 run；历史末尾是 assistant，因此既有逻辑追加 `Continue from where you left off.`，再次调用模型。

原先 `conversation_delivery` 测试只断言通知在数据库中出现一次，甚至允许重复通知增加 provider 调用。新增“模型调用只能有一次”的断言在修复前失败（实际 2，期望 1）。

## 修复结构

```text
server/
├── src/cursor/compile/run.rs              # 复用通知编译器的稳定、排序后的事件身份
├── src/cursor/conversation/
│   ├── background.rs                     # 校验已保存通知，保留正在接收的通知身份
│   ├── registry.rs                       # 持有会话范围的通知接收状态
│   └── runtime.rs                        # 导入检查点和启动模型前执行去重
└── tests/conversation_delivery.rs         # 模型调用次数、并发、重建运行环境与取消回归
```

```text
完成通知 → 会话 ID + 现有事件 ID
  ├─ 正在接收的相同事件 → 成功结束本次 transport，不创建 run
  ├─ 已持久化的相同事件 → 成功结束本次 transport，不导入旧检查点
  └─ 新事件 → 既有编译/投递流程 → 活跃 run 接收，或唤醒结束的主对话
```

通知身份沿用任务类型、task_id、subagent_id、tool_call_id 的现有编码；request_id 和模型选项不属于通知身份。内存占位阻止首次消息持久化前的并发重入，作用域结束自动释放；持久化历史负责后续重复请求和服务重建后的去重。没有新表、schema 迁移或 provider 专属逻辑，也不改写历史前缀。

通知未持久化前如果编译失败，占位释放，随后允许重送。一旦通知已经写入会话，后续失败、取消均不会被同一自动通知重新启动；用户主动 Resume 或新消息仍可继续。这明确区分同一事件的重复投递与用户发起的新操作。

## 验证与状态

后台完成相关 9 项测试通过，覆盖：

- 相同通知换请求 ID 重发，不增加 provider 调用；重建 registry 并保留数据库后同样有效。
- 两条相同 Shell 通知同时到达只启动一个 run；随后携带空旧检查点重发不覆盖当前历史。
- 已取消的处理不会被自动重送重新启动。
- 显式 Resume 可调用模型。
- 首次 Shell/子代理完成、不同子任务、缺省 tool_call_id、Fast 参数和正在运行的父对话继续按原逻辑处理。

最终服务端全量 **350 项通过、0 失败**，Clippy all-targets（`-D warnings`）、fmt 和 diff 检查通过。隔离测试服务已更新，保留原测试数据库与代理端口 57184，正式数据未修改。详见 [验证证据](2026-10-08-background-completion-evidence.json)。修复后真实 Cursor 手动复测尚待用户进行；本次未合并、未发布。
