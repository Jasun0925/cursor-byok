# Issue #504：重复用户输入中断正在执行的工具

**已独立复现并修复一个相关缺口：同一条用户消息使用新的传输序号再次到达活动 Run 时，服务端会重新排队并 Abort 正在执行的工具。尚无证据证明这是原报告全部症状的根因。** 独立源码提交为 [`9846647`](https://github.com/haohaomin/cursor-byok/commit/9846647)。补丁没有改动 Write、历史恢复或模型供应商适配器；安装包尚未发布。

## 原报告与范围

- [上游 #504](https://github.com/leookun/cursor-byok/issues/504) 在 2026-10-27 重新读取：OPEN、0 条评论，最后更新时间为 2026-10-07。正文分别提到旧用户请求再次出现、Write 内容遗漏、以及模型声称做过未发生的工具调用，没有版本、请求、checkpoint 或供应商响应样本。
- 基线：`main=1343b8af0ba85c1002d90f38767300c4225b1312`。隔离分支 `codex/issue504-history`，不混入待验收的远程 Semble 修改。
- 测试使用临时 SQLite、真实传输协议和运行层、FakeProvider 事件流及合成 Exec 回包。没有操作正式数据库、改变 Cursor 设置或执行真实文件写入。

## 失败基线

两个场景都保持 Read 未完成，然后用新的 `Append.seqno` 发送相同 `UserMessage.message_id`：

| 消息来源 | 修复前 | 修复后 |
| --- | --- | --- |
| 初始 AgentRunRequest 中的用户消息，经运行时 UserMessageAction 重传 | 再次 Queued，向 Read 发出 Abort | 不重新排队，原 Read 正常完成 |
| 活动 Run 已接收的追加用户消息，再次通过 UserMessageAction 到达 | 再次 Queued，向 Read 发出 Abort | 不重新排队，原 Read 正常完成 |

基线两项测试均在 `duplicate user input aborted the already-running Read (exec 1)` 断言失败；`/tmp/cursor-byok-issue504-abort-baseline.log` 保存输出。更早的排队断言也失败，见 `/tmp/cursor-byok-issue504-baseline.log`。初次夹具编译因 SetBlobResult 字段不完整而失败，已修正；该编译错误不计作产品复现。

运行层后来才执行的持久化去重无法撤回已经发出的 Abort。这解释了本次捕获到的工具中断，但尚未证明 Cursor 在原作者环境实际重传了这些消息，也没有复现原作者所说的所有“重新执行”。

## 修复与生命周期

```text
server/
├── src/cursor/conversation/
│   └── output.rs              # 活动 Run 的输入、输出与工具中断；1260 行，本次增加 13 行
└── tests/
    └── issue504_repro.rs      # 消息交付、历史恢复、Write 参数的六组对照；523 行
```

行数含注释和空行；测试单列，没有生成代码。没有移动生产模块或增加持久化表。

```text
Cursor Append（新传输序号）
  → ConversationAction::UserMessageAction
  → 当前 Run 的 ConversationOutput
      ├─ message_id 等于初始输入或已接受的追加输入 → 结束本次交付，不中断工具
      └─ 新 message_id → 编译 → Queued → 中断当前工具 → 追加持久化消息 → 模型继续
```

去重发生在编译、排队和中断之前，复用当前 Run 的初始用户消息与已接受 injection ID 集合。按消息身份去重，不按文字去重；相同文字而不同 ID 的第二条指令仍正常进入历史并中断旧工具。空 ID 仍进入既有校验，没有新增容错路径。

此状态属于当前活动输出生命周期；没有新增全会话或跨进程的去重缓存。新的 AgentRunRequest、完成后创建的新 generation、InjectContextAction 的既有处理均未改变。不能把它解释为所有重连、重新提交或编辑消息的去重保证。

## 六项定向验证

1. 重传初始输入不重新排队、不 Abort Read；成功工具结果恰好一份。
2. 重传运行中追加输入同样不 Abort、不重复入史；原 Read 结果继续交给模型。
3. 相同文字、不同消息 ID 保留两份输入，第二条正常中断 Read 并继续。
4. 第一轮完成后关闭 Registry，再用同一临时库新建 Store/Registry，通过输出 checkpoint 开启第二轮：旧输入在历史中恰好一次，新用户消息不混入旧输入；系统指令相同，旧请求历史是新请求的精确结构前缀；没有凭空生成工具调用。
5. 把含中文、换行、引号、emoji 的 Write JSON 拆成逐字符参数增量：真实解析与调度路径传给原生 WriteArgs.file_text 的内容完整，模型历史保留原始参数和一个成功结果。
6. Write 缺少 contents：允许既有预读，之后不发出原生 Write，模型收到 `missing contents` 错误；没有伪造写入成功。

六项通过，日志 `/tmp/cursor-byok-issue504-boundaries.log`。基线与早期两项/三项日志中的测试目标名为 `user_message_delivery`，补齐另外三类对照后统一命名为 `issue504_repro`。

本机 `cargo test --workspace --all-targets` **481 通过、0 失败、0 忽略**，包括既有中断、后台交付、前缀稳定性、checkpoint 恢复、压缩和工具回归，日志 `/tmp/cursor-byok-issue504-workspace.log`。`cargo clippy --workspace --all-targets -- -D warnings`、格式及差异检查通过，Clippy 日志 `/tmp/cursor-byok-issue504-clippy.log`。本段记录本机结果，远端 CI 不以本机结果代替。

复跑入口：

```sh
cargo test -p cursor-server --test issue504_repro -- --nocapture
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## 未证明的部分

- 六项测试不是 Cursor UI、真实网关或真实供应商端到端验收；逐字符 ModelEvent 也不覆盖 HTTP/SSE 适配器丢字段的所有可能。
- Write 和恢复历史的两个原始症状在本次受控路径未复现，因此没有为它们添加生产补丁。模型自然语言中的“记错”也不能凭合成工具账本正确就判定不存在。
- 下一步需要原报告的脱敏 AgentRunRequest、运行时 ConversationAction、模型工具参数和实际 Exec 请求，按消息 ID、工具 ID 和时间对应，才能确定真实断点。
- 原仓库再次确认已归档只读。回复只保存为草稿，没有发送、没有关闭原 Issue，也不推荐用户安装尚未包含此补丁的 v1.0.17 来验证修复。
