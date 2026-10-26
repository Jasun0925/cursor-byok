# Issue #469：真实 SSH 原生文件工具采集实验

**原生 Grep 可以执行远端候选检索，但会跟随工作区内部符号链接；Glob 还包含 Git 排除文件，因此不能直接用其输出建立远程 Semble 快照。** 这是实现选择的实测证据，不是远程 Semble 修复验收。

基线 `95a9b8ff057f0f8d454101be5364598874a9e325`；Cursor 3.27.14；macOS BYOK → Linux SSH；界面选择 BYOK 的 gpt-6 High，实际追踪模型为 `gpt-6-astra`。独立端口、SQLite 副本与测试缓存；远端项目及链接目标全部是本次生成的合成文件。未读取真实项目正文。

## 三组实际操作

| 操作 | 实际结果 | 对采集设计的影响 |
| --- | --- | --- |
| Glob `**/*` | 列出 `git-hidden.py`；未列出 `.cursorignore` 排除的 `cursor-hidden.py`；仍列出 `.sembleignore` 排除文件 | 不能作为同时满足 Git / Cursor / Semble 规则的清单 |
| Grep `def` / files_with_matches | 排除 Git 和 Cursor 隐藏文件；包含根目录及嵌套 `.sembleignore` 隐藏文件 | 仍需在读取正文前应用 Semble 规则 |
| Read `.sembleignore` 与嵌套规则文件 | 返回预期规则正文 | 可以取得这些可见规则；未覆盖规则文件本身被 Cursor 排除的情形 |
| Grep `^` | 包含空白文件；排除 `.ignore` 隐藏文件；**包含 `linked.py` 与 `linked-dir/other.py`** | 候选列表不能证明物理文件位于工作区内部 |
| Read `multiline.py` 的 1–3 行和 4–6 行 | 中文/emoji 保留，但 CRLF 规范化为 LF，分页末尾换行不等同原始字节 | 不能直接拼接规范化工具文本并宣称字节完整；没有抓到原始 ReadSuccess 元数据 |
| 请求 Shell 执行 `find -H … -type f -print0` | 模型依据 Shell 工具说明拒绝枚举文件，**未调用 Shell** | `PHYSICAL_FILES_DONE` 文本不代表物理枚举通过；该传输路径仍待验证 |

第二组提示曾要求 `sort` / `sort_ascending`，但实际 Grep schema 没有这两个参数，模型未传入。没有用提示词要求冒充已执行参数或稳定排序证明。

链接指向另一个由本实验创建的临时目录，正文只包含 `OUTSIDE_LINK_TEST_ONLY`。原始 CRLF 夹具为 90 字节：

```python
b'def unicode_probe():\r\n    text = "\xe4\xb8\xad\xe6\x96\x87\xf0\x9f\x99\x82"\r\n\r\n    # line four\r\n    return text\r\n# EOF\r\n'
```

## 请求证据与范围

[脱敏证据 JSON](2026-10-27-issue469-native-files-evidence.json)包含 11 次供应商调用的 ID/模型/完成状态以及 8 次工具调用与对应文本结果。三组操作属于同一会话；隔离追踪中逐次比较请求消息，前一请求是后一请求的结构化前缀，工具定义保持一致。该观察只覆盖这次实验，没有修改会话投影或宣称所有后续采集实现都满足前缀不变量。

没有执行完整远程 Semble、Windows 路径、双 SSH 同路径、审批等待、采集中取消或索引中取消。原 PR 的启动、过滤与身份问题仍以[固定基线审查](2026-10-27-issue469-semble-check.md)为准。

## 清理

已关闭测试 SSH 工作区、停止隔离服务、移除临时 example。Cursor User 设置恢复为实验前的逐字节备份并重载窗口；正式 BYOK healthz 返回 204。远端 Machine 设置恢复为原先不存在，两个测试目录在核对唯一标记后删除，没有遍历符号链接目标。包含凭据的数据库副本及设置备份已删除，仅保留合成工具结果、探针和截图。

后续先补齐物理来源边界与传输前过滤，再实现远程索引调用。共享索引层的 `.cursorignore` 和热缓存问题已独立[修复并验证](2026-10-27-semble-cursorignore-fix.md)，不改变远程采集尚未实现的状态。
