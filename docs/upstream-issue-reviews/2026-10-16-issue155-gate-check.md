# #155：功能配置与后台恢复请求复测

2026-10-16，接续 [前台/后台对照](2026-10-16-issue155-notification-check.md)。**临时对齐配置身份并开启 `long_running_jobs` 后，Cursor 自动发起了原会话的恢复请求，但 BYOK 返回 `background task notification contains no finished task`，界面显示 Error resuming chat。后台通知没有到达的阻断已被绕过，后续通知处理仍存在缺口；尚未修复。**

本轮重新查询上游 [#155](https://github.com/leookun/cursor-byok/issues/155)：仍为 OPEN，最后更新时间 2026-07-31。原报告的人工审批等待和约 20 分钟卡住，与本轮确定的后台通知问题分别记录，不将本轮等同于完整复现原报告。

## 实验范围

- 基线仍为 `a7ae78ab1d064206fdbb5aeb0659bd48e3dadb7d`，Cursor 3.24.18，界面 gpt-6 High，真实模型 `gpt-6-astra`。
- 从该提交导出临时源代码，仅修改测试版 Analytics Bootstrap 响应；独立构建 `issue155-gate-probe`。正式源代码、原 `cursor-server` 二进制和正式数据库均不修改。
- 使用隔离数据库、两个隔离工作区和端口 57573 / 57584。两组都运行相同的 45 秒脚本，`block_until_ms=1000`，`notify_on_output` 的 pattern/reason/debounce_ms 相同。
- 临时服务只改变配置中的用户标识和 `long_running_jobs`。没有修改登录凭据、审批模式、Cursor 安装包或其他功能开关。配置身份匹配只用于测试本机已有身份，原始标识不进入证据。
- 功能开关通过 Cursor 自带 `Developer: Dump Feature Gates` 导出实际评估值，未把 SQLite 缓存值直接当作运行值。导出命令在同一安装实例的 IDE 窗口执行；E 组的 Agent 自动恢复请求另提供了行为证据。

## 配置加载结果

| 阶段 | 配置用户与登录身份 | 缓存的 long_running_jobs | 诊断运行值 | nal_agent_retries 运行值 |
| --- | --- | --- | --- | --- |
| 初始诊断 | 不一致 | 缺失 | false | true |
| 只补通知开关，刷新后 | 不一致 | true | false | false |
| D：只对齐身份 | 一致 | 缺失 | false | true |
| E：对齐身份并补通知开关 | 一致 | true | true | true |

初始诊断中的重试开关为 true，刷新后才变为 false，不能笼统声称身份不一致在所有阶段都立即清空配置。安装包代码解释了缓存加载与网络刷新两条路径的差异：

```text
BYOK local_response()                  user.userID 固定为 local_ultra
  → Statsig dataAdapter.setData()      按响应内的 user 存储配置
  → Cursor 刷新时替换查询 userID        使用当前访问令牌的 sub
  → Statsig updateUserSync()           按当前身份读取配置
```

本轮只在本地比较身份是否一致，不保存或展示令牌、账号标识。身份对齐后的运行值恢复与上述代码路径一致。D/E 两组使用同一个实验二进制，主要控制变量是 `long_running_jobs` 是否存在且为 true；模型自行生成的描述、会话 ID 和工作目录不同。

## 两组真实后台结果

| 检查项 | D：身份对齐，通知开关关闭 | E：身份对齐，通知开关开启 |
| --- | --- | --- |
| 脚本耗时 | 45.007 秒 | 45.005 秒 |
| DONE 输出 | 有 | 有 |
| 终端退出码证据 | 重载后终端文件未保留，不据此断言退出码 | 0，status=succeeded，终端总耗时 50.762 秒 |
| 初始回复 | CASE_D_STARTED | CASE_E_STARTED |
| 结束后的行为 | 观察 69.664 秒，没有续答 | 自动尝试恢复原会话，随后报错 |
| Shell / 真实模型调用数 | 1 / 2 | 1 / 2，恢复请求没有进入新的模型调用 |

两组都没有人工追问或轮询工具，也没有重复执行脚本。E 组脚本在 01:50:39.795 完成；客户端 01:50:39.803 匹配 `PROBE_DONE`，01:50:39.858 为原会话重新选择模型。服务端在 01:50:55.744 收到自动恢复请求 `2ad5b773-dd31-4533-ad35-975ae7986694`。

E 组原会话 ID 为 `7970ada5-d50d-4d43-9ea1-064f8628ee69`。恢复错误保存在这个会话的消息中，客户端日志也使用同一个 composerId。这说明该次通知确实回到了所属会话，不能再仅凭匹配日志中的 `conversation_id=local` 认定错绑。

## 恢复失败的准确边界

```text
后台命令打印 DONE 并退出                 已验证
  → Cursor 输出观察器匹配                 已验证
  → long_running_jobs 允许通知转发        E 组验证
  → 原会话自动发起恢复                    客户端、服务端和消息归属共同验证
  → BYOK 接收并投影后台通知                返回 invalid_argument
  → 新一轮模型调用                        未发生
```

Cursor 保存的错误堆栈首行为：

```text
RetriableError: [invalid_argument] background task notification contains no finished task
```

对应服务端 `server/src/cursor/compile/insert_messages.rs::project`：只保留 `TaskFinished`，跳过其他 reason；过滤后没有完成项则返回上述错误。**“收到的通知没有被识别为 TaskFinished”已经由真实错误证实；具体是 TaskProgress 还是其他 reason 尚未捕获原始协议，不能把它写成已证实的具体枚举值。**

本轮新建数据库开始时未启用详细协议记录，因此自动恢复请求没有保存原始 protobuf。发现错误后已在隔离库打开详细记录，准备点击一次 Resume；但 CUA 在点击前报告 Mac 锁定，未执行点击。随后从该测试会话的本地错误记录取得了上述准确服务端错误。人工重试与原始通知载荷仍待补测，不能宣称重试通过。

后续修复应分别处理：本地 Bootstrap 配置与当前身份一致、明确所需后台功能开关，以及后台进度/完成通知的实际语义。先捕获该恢复请求的真实 reason，再设计通知投影；不能仅放宽错误条件后声称整个功能修复。另有已记录的 debounce 单位差异，本轮未变更。

## 证据与收尾

[脱敏证据](2026-10-16-issue155-gate-evidence.json) 保留测试参数、真实模型调用、脚本事件、实际开关值、临时实验补丁、匹配日志、终端成功状态和 Cursor 保存的恢复错误。它不包含原始数据库、访问令牌、身份原文或无关对话。

实验结束使用原二进制生成原始 Bootstrap 响应，并通过 SQLite 事务仅恢复 Cursor 的这一个配置缓存键；其余存储未改。由于锁屏，当前窗口内存是否重载原配置尚未验证，下次重新打开 Cursor 会读取恢复后的缓存。测试服务已停止、两端口释放，六个代理键恢复且其他设置保持不变；正式数据库与原二进制 SHA-256 均未变。临时源代码、数据库、身份文件、测试可执行文件和本轮导出的诊断文件已删除。详情见证据的 `cleanup`。本轮没有提交、合并、发布或评论上游。
