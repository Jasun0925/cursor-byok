# #155：真实通知载荷与 Resume 复测

2026-10-16，接续 [功能配置实验](2026-10-16-issue155-gate-check.md)。**已确认自动恢复失败的实际通知是 `TaskProgress`；点击一次 Resume 后，Cursor 把进度与完成通知一起发送，BYOK 接受完成项并续答成功。问题已复现、原因已定位，生产代码尚未修复。**

## 范围与版本

- BYOK 基线 `a7ae78ab1d064206fdbb5aeb0659bd48e3dadb7d`，独立数据库、工作区和端口 57573 / 57584。从第一条请求前就开启详细协议记录。
- 只在临时归档源码中使用上一轮的 Analytics Bootstrap 实验补丁：对齐本机配置身份，开启 `long_running_jobs`。没有放宽通知处理逻辑。
- Cursor 在接管环境重启时应用了已下载的更新，由上轮 3.24.18 变为 **3.25.6**。本轮 `main.log` 的启动更新地址与安装包版本均为 3.25.6；不能写成与上轮完全相同的客户端版本。
- 界面 gpt-6 High，实际供应商模型 `gpt-6-astra`。本轮没有导出开启状态的功能诊断，真实自动通知请求证明转发链路已工作。
- 用户授权的真实界面测试；不是构造通知的协议模拟。原 #155 所述审批等待与约 20 分钟卡住的场景仍未完整覆盖。

## 输入与时序

在新会话 `b3d5aaa6-654d-42a1-a6ae-20637425ab4d` 要求只执行一次：

```json
{
  "command": "python3 -u probe.py notify",
  "block_until_ms": 1000,
  "notify_on_output": {
    "pattern": "PROBE_DONE",
    "reason": "probe completion",
    "debounce_ms": 5000
  }
}
```

脚本打印 START、等待 45 秒、打印 DONE；要求初始回复后不轮询。下面时间均为北京时间。

| 时间 | 实际行为 | 结果 |
| --- | --- | --- |
| 11:15:20.776 | 脚本 START | 唯一一次执行 |
| 11:15:25.721 前 | 初始回复 `CASE_F_STARTED` | 初始两次模型调用完成 |
| 11:16:05.786 | 脚本 DONE | 脚本耗时 45.010 秒 |
| 11:16:05.849 | 首次自动恢复 | 仅含 TaskProgress，被拒绝 |
| 11:16:05.905 | 终端记录结束 | status=succeeded，exit_code=0，总耗时 48.946 秒 |
| 11:16:08.834 / 13.824 / 23.810 | 三次自动重试 | 相同的纯进度载荷，全部被拒绝 |
| 11:16:35.416 | 界面点击 Resume 一次 | 没有新增提示或重跑脚本 |
| 11:16:35.546 | 收到手动恢复请求 | TaskProgress + TaskFinished |
| 11:16:48.019 前 | 恢复运行完成并显示续答 | 新增 1 次模型调用，未新增 Shell 执行 |

四次自动请求 ID、时间、脱敏字段和 action SHA-256 均保存在 [证据 JSON](2026-10-16-issue155-resume-evidence.json)。自动请求 HTTP 状态为 200，但流内返回 `invalid_argument`；不能只用 HTTP 状态判断成功。

## 实际通知与拒绝原因

直接解码记录下来的 `BidiAppendRequest → AgentClientMessage → AgentRunRequest.action`，自动请求只有这一项：

```json
{
  "task_id": "684965:task_progress:1",
  "kind": 1,
  "status": 1,
  "title": "probe completion",
  "reason": 2,
  "tool_call_id": "call_4580b87ce76540a4bf2963c83388942e"
}
```

协议枚举为 `SHELL / SUCCESS / TASK_PROGRESS`。detail 明确表示输出第一次匹配 `/PROBE_DONE/`。这是一条输出观察器的进度通知；标题由测试参数指定，`status=SUCCESS` 也不能单独解释为进程已经退出。

```text
probe.py 输出 PROBE_DONE
  → Cursor 输出观察器发 TaskProgress
  → Cursor 自动恢复原会话
  → server/src/cursor/compile/insert_messages.rs::project
      过滤所有非 TaskFinished 项
      过滤后为空 → background task notification contains no finished task
  → Cursor 自动重试同一载荷 → Error resuming chat
```

首次请求在终端结束记录前约 56 毫秒到达。即使之后进程已经结束，后续三次自动重试仍只包含原来的进度项。因此不能假设进度通知总会和完成通知一起发送。

手动 Resume 请求 `d0c5623d-a519-4fb8-b6a5-31f38f2ccd3c` 带有两个条目：原进度项，以及 `task_id=684965 / reason=1 (TASK_FINISHED) / status=1 (SUCCESS)`。现有代码跳过前者、接受后者，恢复成功。模型只收到完成项的投影，进度项的匹配详情仍被丢弃。

界面最终显示 `CASE_F_AUTO_COMPLETED`，这是提示词指定的回复标签；**实际成功由人工点击 Resume 触发，不能据这个文本宣称自动恢复通过。**点击后的截图曾返回 macOS -3812，但服务端在点击后约 130 毫秒收到请求，随后 AX 确认错误区消失并出现完整续答，证明点击已生效。

## 修复边界

后续应让通知投影明确支持进度与完成两种语义，避免把合法纯进度通知报为协议错误，也不能把进度冒充任务退出。仍需处理上一轮确认的功能配置身份/开关问题，并验证纯进度、纯完成、混合通知和重试的行为。已记录的 debounce 单位差异没有在本轮修改，也没有被证明是此次拒绝的原因。

本轮只更新证据与主清单；没有提交、合并、发布或评论上游。清单保持“通知配置及恢复处理待修”。

## 收尾

已切回原 `cursor-server` 并重新启动 Cursor，确认 Bootstrap 缓存恢复为原来的 `local_ultra`、无 `long_running_jobs`。随后从 IDE 执行 `Developer: Dump Feature Gates`，运行值 `long_running_jobs=false`，实验开关已撤销；`nal_agent_retries=false` 也符合原配置身份不一致时的刷新结果，没有额外改写为 true。

服务停止后已恢复六个代理设置键，保留其他设置；两端口释放，正式数据库与原二进制 SHA-256 均未变。临时数据库、源代码、身份文件、实验可执行文件及其专属构建产物、此次功能诊断文件已删除。共享构建缓存和 Cursor 测试会话保留，完整收尾结果见证据 JSON 的 `cleanup`。
