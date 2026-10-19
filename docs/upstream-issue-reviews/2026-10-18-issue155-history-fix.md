# #155：后台续答丢失已导入历史

**本轮新增修复一个已实机复现的 BYOK 历史选择错误；Cursor 3.25.9 的 25 秒 `notify_on_output` 后台任务已自动续答成功。** 原报告的约 20 分钟卡住、人工审批超时仍未完成验收，因此 #155 保持部分修复。补丁尚未合入或发布。

## 实机失败与根因

17:18 的新测试成功运行一次 Shell，25.015 秒退出，终端记录退出码 0。Cursor 自动发送了 TaskProgress 和 TaskFinished：前者由 BYOK 返回一次 TurnEnded 和成功 EOF，未调用模型；后者失败于 `Cursor stable history shrank from 6 to 2 roots`。此次已有完整请求证据，断点在 BYOK 恢复阶段，不再是此前“尚未收到通知”的状态。

初始请求 `5c5b8be8-443b-4867-a8c2-0cc4e1276f37` 使用官方路由；完成请求 `9c20aa4b-5e03-44ce-ace6-72ae8016b083` 使用本地 BYOK 路由。这是 BYOK 第一次接管该会话。完成请求带来 6 个历史根节点，服务端确实成功解码 6 个，并导入了除系统提示外的 5 条历史；但准备恢复时，从待投递队列取出完成事件的逻辑无条件选择了本地 current checkpoint。新接入会话的 current 仍为空，覆盖了刚导入的历史。

```text
修复前
Cursor 的 6 个历史根节点
  → 解码、导入 5 条会话消息（系统提示单独编译）
  → 完成事件进入待投递队列
  → 取出事件时无条件切到本地空 current
  → 只剩系统提示 + 完成事件，共 2 个根节点
  → 历史缩短校验报错，未调用供应商模型

修复后
导入的历史 + 待投递完成事件
  → 比较本地历史是否完整包含导入历史的前缀
  → 包含：保留本地 canonical 消息身份及已追加内容
  → 不包含：保留请求导入的完整历史
  → 追加完成事件 → 发布 checkpoint → 调用模型 → TurnEnded → EOF
```

首次失败已持久化完成事件，客户端随后的同事件重试被现有去重逻辑确认，没有重新调用模型。本轮没有放宽历史缩短检查，也没有通过重跑 Shell 或重复消费事件掩盖错误。

## 修改范围

```text
server/
├── src/cursor/conversation/
│   ├── background.rs             # 为待投递消息选择完整的历史基点
│   └── runtime.rs                # 取出待投递消息时使用该选择结果
└── tests/conversation_delivery.rs # 新接入历史、本地历史落后与重复通知回归
```

比较使用现有 provider-independent 消息投影，逐条核对角色和内容；解码导入消息时内部消息 ID 可能变化，因此不以 ID 不同误判内容发生变化。仅选择基点，不改写已有消息或供应商序列化。没有新增数据库结构、修改 Cursor 安装文件或移动模块。

## 回归和实机验收

- 先添加 `first_background_completion_preserves_imported_history`，修复前失败：`stable history shrank from 3 to 2 roots`。修复后通过，并断言原始用户消息、STARTED 回复、原有非系统根节点保留，重复完成通知不再次调用模型。
- 添加 `background_completion_preserves_history_newer_than_the_local_head`，覆盖已有本地对话之后 Cursor 又带来更新历史，防止退回较旧的本地 head。
- 后台通知 13 项、前缀稳定性 13 项、checkpoint 恢复 4 项、压缩 6 项通过。随后全部 24 个集成套件、178 项测试通过。
- `cargo clippy -p cursor-server --locked --lib -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 通过。
- 完整单元测试 191/192 通过；仍失败的是此前已记录、未修改的 `provider::attempt::tests::request_transport_failure_is_one_failed_attempt`，断言预期 `Error::Http(_)`。

17:29–17:30 的修复版真实测试使用新的会话 `f0fde35a-20b4-4a8f-82d5-291178dbbed0`，初始请求与通知均走 BYOK：

| 环节 | 实际结果 |
| --- | --- |
| Shell | 只执行一次；等待参数 1000 ms；MATCH / debounce 5000 |
| 后台进程 | 25.020 秒，退出码 0，Shell ID 326156 |
| TaskProgress | 1 次请求；1 个 TurnEnded + EOF；0 次模型调用 |
| TaskFinished | 1 次请求；只触发 1 次模型调用；无协议错误 |
| 自动续答 | 响应文本与 UI 均显示 `HISTORY_FIXED_COMPLETED` |
| 人工推动 | STARTED 到 COMPLETED 之间没有用户追问或重复命令 |
| 时间 | 进程退出约 247 ms 后收到完成请求；请求约 4.088 秒后结束 |

本次实机证明 BYOK 启动的短后台任务可自动继续；官方模型启动后交给 BYOK 的精确转换场景由新增回归覆盖，没有在修复后再次进行同场景 UI 对照。另打开了普通后台命令对照的预填入口，但没有新的 plain 请求或进程，因此不计为测试通过。未取得新的运行时功能 dump；自动通知本身已证明本次实际通知链路工作。

## 环境恢复

两个实机测试保持同一个 Cursor 主进程 PID 53439。隔离服务使用独立数据库和 57573 端口，复用原代理端口 49642；实验入口仅额外跳过代理启用时终止 Cursor 的动作，生产文件没有这个改动。

测试完成已停止隔离服务，恢复正式 Cursor BYOK，确认 `integration=enabled`、`settings_applied=true`。正式 harness 启动时重启了 Cursor，随后已重新打开验证工作区。正式数据库与 Cursor 设置文件哈希均与测试前一致，原 Bootstrap 缓存在重开 Cursor 前恢复。未替换正式安装包，修复仍在工作区；测试文件保留供复核。

完整脱敏请求、进程事件、二进制哈希与恢复结果见 [实机结构化证据](2026-10-18-issue155-bootstrap-live-evidence.json)。前置修复见 [Bootstrap](2026-10-18-issue155-bootstrap-fix.md) 和 [通知流结束](2026-10-18-issue155-stream-end-fix.md)。

## 同日追加检测

用户要求继续检测后，新增隔离探针复现了旧完成通知撤销已提交压缩、跨批次完成通知重复唤醒两项缺口；均未修复。此前单元测试失败本轮未复现，完整 192 项通过。上述短任务实机成功结论保留，整项仍未完成。详见 [最新检测](2026-10-18-issue155-audit.md)。
