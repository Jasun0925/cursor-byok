# Issue #341：Cursor 原生文件变更登记回调

> 本文保留 2026-10-10 的调查时点。后续用户手动验证两个回滚入口通过，见 [2026-10-11 实测更新](2026-10-11-issue341-check.md)。

## 结论及边界

基线 BYOK `2e2193a`，本机 Cursor 3.24.9。此次继续调查，不修改生产代码或 Cursor 安装包。

从已安装客户端提取原始方法，在隔离 Node VM 中执行，使用内存文件系统、替代服务和手动时钟。12 个场景断言通过，其中包括预期的异常行为：**原生 Write 返回 success，内存文件已变更，但因气泡等待超时，恢复登记全部丢失。**

这证明了一个具体可发生的失败机制，不证明用户现场一定触发了该机制。当前没有真实 UI、扩展宿主 RPC 或磁盘回滚实测；普通路径的 checkpoint 存储服务和 diff 服务也是替代实现，验证的是调用参数及控制流，不是实际持久化成功。

## 当前职责与执行路径

```text
BYOK/server/src/
├── run/model_cycle.rs                       # 接收供应商工具 ID，形成运行事件
└── cursor/
    ├── conversation/output.rs               # 发送工具气泡更新与执行请求
    └── tools/
        ├── mod.rs                          # 工具开始消息排在隐藏 Read 前
        └── codec/request.rs                # 用相同 call_id 构造 WriteArgs

Cursor.app/Contents/Resources/app/
├── extensions/cursor-agent-exec/dist/main.js # 原生读写、变更 tracker、执行会话
└── out/vs/workbench/
    ├── api/node/extensionHostProcess.js     # 扩展宿主到界面进程的回调 RPC
    └── workbench.desktop.main.js            # 气泡查找、检查点及差异登记、回滚 UI
```

```text
服务端交付一条响应流
  → Cursor 拆分并并发运行
      ├→ interactionStream → 串行处理界面更新 → 创建工具气泡 → signalBubbleCreated
      └→ execStream → 原生 Write
           → 读取写入前内容 → 写文件 → await fileChangeTracker.trackChange
                → 执行会话回调（仅 toolCallId 非空时转发）
                → $onAgentExecFileChange(sessionId, change)
                → 界面进程从 sessionId 找 fileChangeHandler
                → handleFileChange(composerId, change)
                     → JZi：在会话及可关联子代理中找工具气泡
                     ├→ 找到：
                     │    originalFileStates[uri]（仅首次写入）
                     │    → updateLastCheckpointWithFileIfNotTracked
                     │    → 更新祖先会话检查点
                     │    → 普通文本 inline diff（原始行、新行、所属会话及工具 ID）
                     │    → 发出文件已编辑事件
                     └→ 未找到：暂存回调，启动等待，然后立即返回
                          ├→ 气泡及时出现 → 再查找并登记
                          └→ 5000ms 超时/会话清理 → 删除暂存项
           → WriteResult.success → BYOK 完成编辑结果（这里才裁剪 before/after）
```

回调携带 `{path, before, after, metadata: {toolCallId}}`。`before === undefined` 表示新文件；删除的 `after === undefined` 会走删除分支。文件系统变更监听本身不等于此处的登记回调。

**发送顺序不等于登记完成顺序。** `ClientInteractionController.run` 将更新串到 Promise 队列，exec controller 与它并行。BYOK 的工具开始消息虽然先发，执行器仍可能先完成写入；客户端因此设置了等待气泡的路径。

## 隔离验证结果

| 场景 | 原文件登记 | 检查点登记调用 | inline diff 创建 | 结论 |
|---|---:|---:|---:|---|
| 气泡已存在 | 1 | 1 | 1 | 原始内容正确传入两条恢复路径 |
| 新文件 | 1 | 1 | 1 | 正确标记新建，原始行为空 |
| 气泡在超时前出现 | 1 | 1 | 1 | 暂存回调被处理 |
| 气泡在 5 秒超时后出现 | 0 | 0 | 0 | 永久丢弃，没有迟到补登记 |
| 其他会话先登记同名工具 ID | 0 | 0 | 0 | 当前气泡未创建时可立即丢登记 |
| 回调缺少工具 ID，直接进入界面 handler | 0 | 0 | 0 | 明确抛错；原生执行会话在更外层会过滤空 ID |
| 检查点服务抛错 | 1 | 失败 | 0 | 后续 diff 创建被阻断，不能保证登记原子性 |
| 没有含 checkpointId 的历史气泡 | — | 真实 checkpoint 方法不写入 | — | checkpoint 方法直接跳过；此场景单独测试 |
| 原生 Write，气泡已存在 | 1 | 1 | 1 | 返回 success，内存文件由 old 变 new |
| 原生 Write，气泡随后超时 | 0 | 0 | 0 | 返回 success，文件已变更，恢复登记却丢失 |
| 原生二进制 Write | 0 | 0 | 0 | 返回 success，按客户端条件跳过回调 |
| 原生旧文件大于 50 MiB | 0 | 0 | 0 | 返回 success，按客户端条件跳过回调 |

大文件测试仅模拟 stat 返回的大小，没有创建或读取大文件。手动时钟断言客户端请求的等待时间确为 5000ms，随后主动触发定时器；没有测量真实 UI 延迟。

## 两个关键丢失机制

### 1. 超时后没有补登记

`_handleFileChangeImpl` 在找不到气泡时排队，但不 await 等待 Promise。原生 Write 所 await 的回调因此可以立即结束。超时 catch 删除队列且不记录错误；后来创建气泡不会恢复已删队列。

因此没有错误日志、Write 成功、编辑卡片出现，都不能独立证明恢复登记完成。当前暂存队列也只保留每个会话/工具 ID 最新 10 条变更；该数量限制此次仅静态核对，未运行覆盖。

### 2. 等待注册表的跨会话工具 ID 判断

`waitForBubbleCreation(composerId, toolCallId)` 的等待条件检查 `_createdBubbleOwners.has(toolCallId)`，没有检查指定 composerId 是否为 owner。`signalBubbleCreated` 也会唤醒其他会话同名工具 ID 的等待者。

若另一个无关联会话已使用该 ID，而当前会话气泡尚未创建，等待立即结束；drain 会删队列并用“不再等待”的方式查找，找不到即返回。这一行为已在原始方法探针复现。正常父子代理共享工具归属与完全无关联会话应分别考虑，不能简单全局禁止跨会话唤醒。

BYOK 当前在单次模型流内拒绝重复工具 ID，并透传供应商 ID；本次未取得用户失败请求的实际 ID，不据此认定存在现场碰撞。

## 与 BYOK 已知快照裁剪的关系

普通文本编辑用 file_text、空 file_bytes、相同 call_id，静态检查未发现直接禁用回调。探针的正常分支也验证独立回调可将完整旧内容登记成功。

原生回调发生在 WriteResult 之前；BYOK 的 gate_edit 裁剪发生在 WriteResult 之后。因此 **gate_edit 清空结果不能直接导致前面的回调不执行**。它会让最终编辑卡片/回放缺少完整内容，是另一项已确认缺口；仅修复裁剪不能保证修复气泡关联或回调丢失。

## 后续现场定位点

需要将同一次编辑的以下事件按 sessionId / composerId / toolCallId 关联：

1. 工具开始及含 path 的 partial 消息到达、气泡创建完成时间。
2. 原生 Write 写入前后内容的长度/哈希及回调时间，不记录源代码正文或密钥。
3. 回调 RPC 的 session handler 是否仍存在。
4. 气泡查找结果、暂存、超时或提前唤醒，以及处理时的会话归属。
5. originalFileStates、目标 checkpoint 和 diff descriptor 是否分别登记。

此次不安装注入脚本、不改 Cursor 代码、不自行给写入加等待、不使用 Git 操作替代原生回滚。

## 复现材料

- [探针](2026-10-10-issue341-callback-probe.mjs)
- [12 项结果与客户端哈希](2026-10-10-issue341-callback-evidence.json)

```sh
node docs/upstream-issue-reviews/2026-10-10-issue341-callback-probe.mjs
```

探针按已知起止标记从本机安装包提取原始方法，依赖当前客户端符号。升级后需重新核对；输出中记录两个包的 SHA256，以区分版本。它不属于生产测试套件。
