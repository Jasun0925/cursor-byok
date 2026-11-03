# Issue #469 / PR #498：真实 Cursor SSH 快照验收

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

**部分通过，仍不能合入或宣称全部修复。** 两种查询、排除正文、Unicode、多批传输和采集中取消已有真实链路证据；另发现 18,144 文件时元数据 JSON 不完整，真实审批尚未覆盖。

候选基线：`2956bbf3869dbbdbfaad0c84adccdd59fe81968b`，开发分支 `codex/remote-semble`。实测增加了本文所述 Shell 策略修正，两个修改文件的 SHA-256 记录在[结构化证据](2026-10-27-issue469-snapshot-live-evidence.json)。测试不是 main 或 v1.0.17 的能力声明。

## 环境和实际链路

```text
macOS Cursor 3.27.14 / Remote SSH Linux
  → 隔离 BYOK（正式数据库的 SQLite backup 副本，独立端口）
  → 实际配置的 OpenAI Chat 供应商 / gpt-6-astra
  → 模型 SembleSearch / SembleFindRelated
  → Cursor 在 SSH 主机执行 Python 采集
  → 本机 BYOK 建请求内存索引、检索
  → 原工具调用结果 → 模型继续 → Cursor 展示路径、代码与最终回复
```

远端测试目录仅包含本次生成的合成代码。开始时有 `login.py`、`audit.py`、大小写不同的 `Case.py` / `case.py`；另有分别被 Git、Cursor、Semble 和父目录规则排除的 canary，以及指向根目录外的符号链接。后续增加 140 个 Unicode 路径文件和 6,000 个取消夹具。没有使用真实项目源码作为测试数据。

协议记录使用仓库现有 prost 类型解码：`BidiAppendRequest` → `AgentClientMessage` → `ExecClientMessage` → `ShellStream`，按请求和执行 ID 汇总。供应商请求只输出消息计数、结构比较、工具摘要及敏感内容检查布尔值，没有提交请求头或完整提示词。

## 修正 Shell 执行策略

最初候选强制发送 `WorkspaceReadonly`。请求 `5307a194-…` 的元数据采集成功，但第二个正文采集在 Linux 沙箱选择阶段失败：Landlock V3 不可用，Bubblewrap 预检无法创建证书路径；退出码 2。因此不能写成“所有命令都未开始”。

普通 Shell 对照 `7849e22c-…` 使用原有客户端策略，成功输出 `NORMAL_SHELL_POLICY_OK`。随后修正 Semble：与普通 Shell 一样省略 `requested_sandbox_policy`，保留 `skip_approval=false`、超时、关闭 stdin 等限制。没有改用户权限设置、申请 `all`、跳过审批或在失败后尝试无沙箱执行。采集器仍只读，但实际沙箱由 Cursor 决定；本机配置返回的策略枚举为 1，不能声称是只读沙箱。

定向测试先因 `requested_sandbox_policy.is_none()` 断言失败，修正后 7 项通过、1 项真实模型测试保持默认忽略；全工作区 Clippy 通过。真实模型测试在候选基线阶段已单独执行通过。

## 验收结果

| 场景 | 实际结果 |
| --- | --- |
| 两种查询 `4e7f3450-…` | 模型分别调用 Search、Related 一次；共 4 次采集 Exec，全部退出 0；两次正文均只有四个允许文件。UI 正确显示路径、代码及 `REMOTE_SEMBLE_SNAPSHOT_DONE` |
| 排除正文 | 对实际传回的 base64 解码检查，未出现 `PRIVATE_` canary；被 Git、Cursor、Semble、父目录规则排除的正文与链接目标均未传回。`.sembleignore` 白名单没有覆盖 `.cursorignore` |
| 144 文件 `73aea70a-…` | 元数据含 152 个条目，允许正文分成 128+16 两批；返回 `bulk/file_139_中文.py` 及正确内容，UI 显示 `REMOTE_SEMBLE_BATCH_DONE` |
| 6,144 文件搜索 | 3 次完整采集均成功：元数据含 6,153 条，48 个正文批次，合计 49 次 Exec，结果路径和内容正确；不是系统性能基准 |
| 早期取消尝试 | 前两次未赶上停止；第三次虽取消了 run，但工具结果已提前约 3 秒完成。这三次不计为采集取消通过 |
| **采集中取消 `db166d19-…`** | UI 点击停止时尚在采集。元数据和 9 个正文批次已完成，Exec 11 正在执行；它随后报告 `aborted=true`。工具只返回取消，run 为 cancelled；未完成其余 39 批，也没有自动重跑或交付搜索成功结果 |
| 供应商历史 | 同一对话 18 个请求，前一个请求的消息始终是后一个请求的完整结构前缀，工具定义摘要不变；未发现内部元数据指纹、源码 base64 或排除文件 canary |
| **18,144 文件 `ab61cffa-…`** | 收到 3 段 stdout，共 832,960 字节，然后退出 0；JSON 在字符串中途结束。工具明确报错，未开始正文采集；这是未解决的真实故障 |

采集中取消的时间（UTC epoch 毫秒）：UI 点击停止 `1793070113097`；工具取消落库 `1793070113209`；客户端中止退出 `1793070113211`。后续再查仍为取消，未产生新的自动 run。采集中止不代表已验证索引线程运行期间的 UI 取消；后者目前只有自动化测试。

## 新故障及下一步

大文件集的错误为 `Invalid workspace collection result: EOF while parsing a string at line 1 column 832960`。收到的最后一段止于条目路径中间，且没有完整 JSON 尾部。它小于本实现的 5 MiB stdout 接收上限，不能简单归因于该上限；也没有证据证明是某个固定大小的 Cursor 截断。仅凭退出码 0 不能把采集标为成功。

下一步应对比主机生成的完整字节数、Cursor 输出转发及 BYOK 入站顺序，定位缺失位置；再验证大元数据和接近单批上限的正文传输。不能用降低工作区限制、返回部分结果或退回服务器本机目录掩盖失败。真实审批弹窗和 Windows Cursor UI 仍需单独验收；Windows 原生命令测试不能替代 UI。

候选 `2956bbf` 的 [CI 40435694002](https://github.com/haohaomin/cursor-byok/actions/runs/40435694002) 四组全部成功。Linux 汇总 491 项通过、1 项忽略；Windows 和 macOS 各通过 7 项采集、6 项规则及 4 项快照测试。后续策略修正的 CI 必须绑定其新提交，不能沿用该结果。

## 恢复与复现

测试服务已停止，测试端口关闭；本地 Cursor 设置恢复为测试前完整字节，SHA-256 `1c86e40d0f46a6174e76598d78d4b2c3c90b601f63ddaad7d91e8f964fc843b3`。正式后端健康检查为 204、正式代理仍监听。远端 Machine 设置恢复为原来的“不存在”，合成目录已清理，SSH 测试窗口已关闭。正式数据库没有被修改。

[隔离服务与协议解码辅助代码](reproductions/issue469_snapshot_live.rs)保留为文档夹具，不参与生产编译。再次使用时复制到临时 `server/examples/`，提供新的隔离目录及 SQLite backup 数据库；测试数据库与设置备份含凭据，不能提交，结束后应删除。原始请求记录不作为公开证据，结构化 JSON 只保留合成数据的摘要。
