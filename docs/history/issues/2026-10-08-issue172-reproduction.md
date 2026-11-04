# #172：中断后上下文丢失复现

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

基线：main `78e8074` / v1.0.12。复现分支：`codex/issue172-reproduction`。本次仅增加测试与记录，没有修改生产代码、正式数据库或发布新版本。

## 方法

使用真实服务端 TransportRegistry、Cursor protobuf 消息、checkpoint 发布/确认、SQLite 持久化及 provider 请求投影。provider 为可控事件流；工具返回由测试客户端模拟，不执行真实文件读写，不访问真实中转站。不将此结果称为真实 Cursor UI 或真实网络断流复测。

每项先输出 COMPLETED_TEXT_172 并完成 Read，结果为 READ_RESULT_172；随后输出 VISIBLE_BEFORE_STOP_172，在不同阶段中断。恢复时只发送继续指令和客户端最后收到的 checkpoint，不在提示里泄露这三个标记。

| 场景 | 已显示的当前阶段文字 | 更早已完成文字 | 已完成 Read 结果 |
| --- | --- | --- | --- |
| TextDelta 已到达客户端后 CancelAction，再发后续用户消息 | 丢失 | 保留 | 保留 |
| 当前文字 TextEnd 后发起下一次 Read，等待结果时取消，再 Resume | 丢失 | 保留 | 保留 |
| TextDelta 后模拟 provider 连接重置，耗尽 8 次自动重试，再 Resume | 丢失 | 保留 | 保留 |

三项均确认标记先出现在发往 Cursor 的 TextDelta，恢复后的 provider history 却不含该标记。恢复时没有重复发起工具（replayed_tools=0）。等待工具场景使用可控 Read 挂起以建立确定的中断时点，没有运行真实长时间 Shell。

新增 3 项“应保留可见文字”的断言全部失败，复现成功。现有 checkpoint_recovery 4 项、prefix_stability 13 项全部通过（共 17 项），说明既有测试未覆盖这一保留缺口。最初异常断流探针因 12 秒等待不足提前结束；最终将等待上限设为 55 秒，确认耗尽现有 8 次、每次间隔 5 秒的重试后完成复现。最终执行耗时约 42 秒，无超时替代结果。

## 结论与边界

确认当前版本仍存在与 #172 相关的可复现缺口：**中断的模型调用已经展示出来的文字，未进入恢复后的模型上下文**。其中等待工具时，文字本身已经结束，仍因同一轮工具未完成而丢失。

本次没有复现“整轮所有内容都丢失”；更早完成的模型调用文字和工具结果保留。不能将未完成工具结果视为本应存在的内容，也不能用本次模拟代替原作者环境验证。

代码线索：`server/src/run/engine.rs` 的取消/最终失败分支直接结束 run；`ModelCycleFailure` 虽带有 partial_text，但这些分支未持久化它。`server/src/cursor/conversation/output.rs` 的 CycleInterrupted 分支清空展示状态。等待工具取消另需核对 pending assistant 与 settled checkpoint 的边界。以上仅用于下一步定位，尚未实施修复。

测试文件：[issue172_repro.rs](../../../server/tests/issue172_repro.rs)。完整结果：[证据 JSON](2026-10-08-issue172-evidence.json)。测试 provider 只增加“输出指定事件后保持挂起”的能力，用于精确发送取消动作。
