# Issue #469：原生 Ls 限制与物理文件清单实验

**远程 Semble 仍未修复。本轮排除了直接用原生 Ls 建立完整快照的方案，并验证了一个不读取源码正文的元数据探针。** 后者尚未接入 Cursor Exec、忽略规则选择器或搜索引擎，不能标记为远程搜索验收通过。

## 原生 Ls：真实 SSH 请求与结果

基线 `c47db4af679a525a02e0b2433876cff26606f7d9`，Cursor 3.27.14，macOS BYOK → Linux SSH。使用归档源码副本、独立数据库和端口、真实已配置供应商；模型为 `gpt-6-astra`。目录及链接目标都是本次生成的合成文件。

生产工具集没有 Ls。本轮[临时协议探针](reproductions/issue469_ls_wire.patch)把专用 Grep 标记改为实际 `LsArgs`，保存原始 `LsResult` 的树与状态，再包装为 Grep 文本供既有渲染器和模型继续。**客户端执行的确是 Ls，但模型工具名和最终渲染是实验适配，不能据此宣称生产 Ls 支持已完成。** Read 不改变请求，只记录原始响应元数据。

| 场景 | 实际结果 | 判断 |
| --- | --- | --- |
| 小目录，根目录加一个嵌套目录 | 返回全部 7 个文件，`children_were_processed=true` | Git / Cursor 排除文件与文件、目录符号链接均未列出；Semble 排除文件仍列出 |
| 增加 160 个长文件名、`Case.py` 和 `case.py` | Ls **success**，返回 44 个文件；根节点 `processed=false`、`num_files=125` | 成功状态不代表完整清单；该夹具共 169 个未被 Ls 排除的文件 |
| `ignore=["bulk/**", "Case.py"]` | 全部处理完成，但 **Case.py 和 case.py 均被排除** | 不能通过排除已返回文件进行可靠分页，大小写不同的文件可能一起消失 |
| Read 90 字节 CRLF 文件的 1–3 行、4–6 行 | `total_lines=7`、`file_size=90`、`range_applied=true`、`truncated=false`；内容规范化为 LF | `truncated=false` 仍可能只是一个行范围；分页字符串不能直接拼成原始字节 |
| Read 同一文件全文；再读第 7 行 | 全文保留空行和结尾 LF；最后一个范围返回空字符串 | 行数、行范围与结尾空行必须一起处理；全文也不是原始 CRLF 字节 |

`num_files` 在这个截断响应中表示该节点子树未包含的文件数；父子节点统计有包含关系，不能相加。根节点遗漏 125 中，bulk 子树遗漏 118、nested 子树遗漏 1，其余为根目录文件。

[脱敏请求与原始字段证据](2026-10-27-issue469-ls-evidence.json)包含同会话的 10 次供应商调用：9 次完成、1 次供应商错误后重试成功，不能写成 10 次均成功。7 次客户端工具执行分别为 3 次 Ls、4 次 Read。界面最终显示 `LS_LIMIT_PROBE_DONE`。逐次结构化比较中，前次消息均为后次请求的前缀，工具定义不变；这是本次实验的观察，不替代将来实现的前缀回归。

## 安装产物中的解释

使用 `reverse-engineer-anything` 技能及已有 REA 6.7.0 CLI，对实际安装的 `cursor-agent-exec` 选择 `package.json` 和 `dist/main.js` 做静态分析。CLI 用现有 Node 24.18.0 运行，没有安装引擎或改动 Cursor 文件。

- `dist/main.js` SHA-256：`66c26ae001b20362c5f8b0169e2cf752e81e1fa98dac6056b1ba86f9c2292291`。
- REA 证据 ID：`ev_a1e98d64eedbe236a4db4f36ab8ff10effc861df240233a8ca88bf72f45d2641`。
- **以下方法解释来自普通源码文本定位**：`LocalLsExecutor` 的目录树预算是 2500 字符，达到预算后继续统计遗漏并置 `childrenWereProcessed=false`，仍可返回 success；公共文件枚举器对排除项使用 `--iglob`；该 Ls 调用没有启用 `followSymlinks`。
- 文件散列、定位偏移、来源范围与 REA 限制均在证据 JSON。REA 未执行 bundle；211 个未选择文件和动态关系不在分析覆盖内；不能用静态结构替代上述真实 SSH 结果。

## 元数据探针：后续采集路径的一个已验证边界

[探针](reproductions/issue469_inventory.py)只返回文件相对路径、物理文件标识、大小和时间戳，以及工作区内四种 ignore 文件的文本；不打开源码正文。逐层持有目录描述符，打开目录和规则文件时使用 `O_NOFOLLOW`，不跟随文件或目录链接；规则文件被替换为链接、类型异常或读取中变化时整体失败。容量、深度、时间及 UTF-8 错误返回错误对象，不返回成功的部分清单。

```text
本轮已实现的实验
Linux 合成工作区
    → 元数据枚举（持有目录描述符；不读取源码正文）
    → JSON 文件清单、规则文本、文件标识
    → 验证数量、大小写、Unicode、链接边界和输出中无源码标记

后续尚未实现
清单 + 完整规则来源 → 选择允许读取的文件
    → Cursor 权限与执行流程 → 传输选中正文 → 独立快照 → Semble 搜索
```

[9 项边界测试](reproductions/test_issue469_inventory.py)在 macOS 和真实 Linux SSH 均通过，包括在规则文件打开前主动换成符号链接的测试。另一个真实 Linux 夹具返回全部 **163** 个候选文件（160 个长文件名、大小写不同的两个文件、一个 Unicode 名称），保留 `Case.py` 与 `case.py`，跳过两个外部链接；31612 字节输出中没有测试源码正文标记。此次通过 SSH 命令运行，**没有通过 Cursor Shell 发送这份清单**。[独立证据](2026-10-27-issue469-inventory-evidence.json)

远端默认 `python3` 为 3.6.8，最初测试驱动在 `subprocess.run(capture_output=...)` 处失败，尚未执行枚举。改用远端已有 Python 3.12.13 后成功，没有安装软件。探针现在对低于 3.9 的解释器返回明确错误；这不代表已选定产品的最终运行时依赖。

**该探针故意返回 `policy_complete=false`：** 工作区外父目录规则、Git 全局规则和 exclude、嵌套 Git 仓库、Cursor repo-block 权限尚未纳入。它会列出被 ignore 文件匹配的候选名称，尚未作允许读取正文的决定。不能把这份清单直接用于上传或索引，也不能宣称已经实现“所有排除规则都在正文传输前生效”。Windows、Cursor 传输上限、审批、取消、迟到/重复响应、跨主机同路径身份及索引清理仍需后续实现和验收。

## 重跑与清理

元数据探针不改变 Cursor 配置，本机测试可直接运行：

```sh
python3 -B -m unittest discover -s docs/upstream-issue-reviews/reproductions -p test_issue469_inventory.py -v
```

探针需要 Python 3.9+、POSIX 的目录描述符与 `O_NOFOLLOW`，命令参数是物理目录的绝对路径。Linux 额外夹具使用长文件名及大小写文件对。真实 SSH 的启动命令验证了 `sys.argv.pop(1)` 移除脚本参数后再执行，避免原候选 PR 把脚本文本当仓库参数的问题；没有修改原 PR 文件。

原生 Ls 补丁仅供固定基线的独立 checkout / 源码副本使用。应用前检查 `git apply --check`；运行 example 前用 SQLite backup API 制作独立 `live.db`，备份 Cursor User settings，设置 `LS_PROBE_RECORD` 到私有测试目录，并为 SSH 建立合成工作区。example 会临时切换本地 Cursor 代理，不能在未准备恢复流程时直接运行。用 Ctrl-C 关闭服务后还需恢复设置备份并重载 Cursor，不能仅以进程退出判断清理成功。

本轮已经关闭测试 SSH 窗口、停止测试服务，确认两个测试监听端口消失；User settings 逐字节恢复并重载窗口，正式服务 healthz 为 204。远端 Machine 设置恢复为原先不存在。两组远端实验目录均核对专用标记后删除；含凭据的数据库副本和设置备份已删除。保留的只有合成证据、探针和静态分析产物。没有改生产代码、发布版本或向归档上游发送评论。
