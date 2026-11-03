# Issue #469 / PR #498：工作区主机采集与独立搜索快照

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

**开发分支 `codex/remote-semble` 已通过本机回归、跨平台 CI 及真实 Cursor SSH 复测，未合入 main、未发布。** 早期的大响应截断已通过[响应分页](2026-10-27-issue469-output-limit-fix.md)修复；18,144 文件、大正文、两种查询、过滤与采集中止均有[新一轮实测证据](2026-10-27-issue469-paging-live.md)。真实审批及 Windows Cursor UI 尚未覆盖，不能把 #469 标为全部解决。

基线：`23e1a7b8c7a2e38d694786af8e653ed595b3c123`。上游 PR #498 head 仍为 `c7114878c33076340c0c2f1e77ce4e7512d1960e`，未直接采用原补丁。原方案的三个缺陷见[先前复现](2026-10-27-issue469-semble-check.md)，原生工具替代方案的限制见[Ls/元数据实验](2026-10-27-issue469-inventory-check.md)。上游仓库本轮重新查询仍为 archived，未再次尝试发送评论。

## 实际执行路径

```text
模型 SembleSearch / SembleFindRelated（原始工具调用 ID）
  → 当前 Cursor 工作区路径校验
  → Cursor Shell Exec：主机元数据、ignore 文本、Git 排除结果
  → BYOK 规则选择器：先排除，再请求正文
  → Cursor Shell Exec：分批传输选中的原始字节（base64）
  → BYOK：核对路径、大小、批次及完整工作区指纹
  → 请求内存快照 → 原有切块、真实嵌入模型及检索排序
  → 原始工具调用收到一个结果，模型继续

取消 / 替换 run
  → 标记本次搜索取消，终止异步等待，向仍在执行的 Shell 发 Abort
  → 索引线程在文件边界检查取消标记
  → 丢弃迟到结果，释放快照；不创建新生命周期
```

HTTP(S) Git URL 仍走显式获取仓库的路径。文件系统路径统一由 Cursor 的工作区主机提供，避免把远端 `/workspace` 误当成本机同名目录。采集代码只读，不请求网络。Shell 请求沿用 Cursor 已配置的执行策略，`requested_sandbox_policy=None`、`skip_approval=false`，不要求提权或绕过审批。之前强制请求只读沙箱，导致该 Linux 主机在正文采集前因 Landlock/Bubblewrap 不可用而失败；普通 Shell 对照成功，因此修正为与普通 Shell 一致的策略选择方式。实际权限由客户端决定，不能声称具有额外沙箱保证。

```text
server/src/
├── cursor/compile/run.rs               # 从请求取得工作区路径，不改历史消息
├── cursor/tools/semble.rs              # 一次工具调用拥有全部 Exec 与结果（310 行）
├── cursor/tools/runtime.rs             # 拥有取消标记、执行登记和异步任务句柄
└── search/snapshot/
    ├── mod.rs                         # 两阶段采集、分批、完整性校验（208 行）
    ├── transfer.rs                    # 校验并重组有界响应页（156 行，含内嵌测试）
    ├── command.rs                     # POSIX / PowerShell 启动与编码（112 行）
    ├── collect.py                     # 元数据、Git 规则、指纹和选定正文（262 行）
    ├── posix.py                       # 目录描述符与不跟随链接的读取（62 行）
    └── windows.py                     # Win32 句柄、物理路径和变更检查（172 行）
crates/semble-core/src/
├── source/inventory.rs                # 传输正文前执行排除策略（219 行）
├── source/snapshot.rs                 # 验证请求拥有的路径和字节（43 行）
├── index/builder.rs                   # 复用切块/嵌入，快照索引不写磁盘缓存
└── search/engine.rs                   # 共用检索逻辑，快照结果不回读服务器文件
```

行数为本轮实测，包含注释和空行，不包含独立测试与生成代码；transfer.rs 的内嵌测试计入其总行数。现有本机索引 API 继续服务显式 Git URL、基准工具等用途；Cursor 文件系统搜索的旧直接读取服务器路径入口已替换。

## 过滤与隔离

- 主机 Git 计算 `.gitignore`、`info/exclude`、全局规则及嵌套仓库规则，`--no-index` 也检查已跟踪文件的排除状态。
- `.sembleignore`、`.ignore` 保留既有优先级；`.cursorignore` 独立生效，不能被 Semble 白名单重新纳入。父目录已排除的根或祖先目录不能通过选择其子目录绕过。
- 元数据阶段没有源码正文。固定忽略目录、类型、文件大小与规则先于正文请求生效。正文阶段只接受精确请求的文件；缺失、重复、额外文件、错误大小或指纹变化整体失败。
- 正文保留 CRLF 和非 UTF-8 字节；两份快照即使 `repo` 字符串相同也不共享文件或磁盘索引。大小写不同的远端名称保留在内存中。
- 采集不写工作区临时文件。POSIX 不跟随符号链接；Windows 使用 Win32 句柄检查链接与最终路径，原生 Windows CI 已覆盖 junction 等边界，Windows Cursor UI 尚未覆盖。
- “不写快照磁盘缓存”不表示关闭应用原有的协议/请求日志。测试证据只使用合成文件。

## 已验证与待验收

| 验证 | 当前证据 |
| --- | --- |
| macOS 实际采集命令 | 7 项采集测试通过，其中 5 项执行实际命令；覆盖规则、原始字节、Unicode 140 文件多批、嵌套 Git、父目录排除、变化检测和链接 |
| 选择与快照自动化 | 6 项规则测试、4 项快照测试通过，包含同路径隔离、大小写、取消、路径检查与不写索引缓存 |
| Cursor 协议模拟 | 7 项通过：沿用客户端策略、拒绝、断流、重复/迟到、取消、等待线程中止、超限 Abort；不是实际 Cursor UI |
| 真实嵌入模型与实际本机 Shell | 显式运行 `real_model_host_exec_round_trip_search_and_related` 通过：两种工具各两次实际采集、结果回到原调用 ID、排除文件未请求；协议响应由测试驱动构造 |
| 全工作区 | 在增加超限用例与更新工具说明前，492 项通过；`SEMBLE_TEST_MODEL` / `SEMBLE_TEST_TOKENIZER` 指向实际缓存文件，模型夹具确实执行 |
| 前缀及最终静态检查 | 工具说明明确改为工作区主机路径，限制 `top_k ≤ 100`；旧工具摘要因此失败，已按新定义更新快照；前缀/恢复/压缩合计 23 项复测通过，最终全工作区 Clippy、格式及 diff 检查通过 |
| Windows / macOS CI | `2956bbf` 的 [CI 40435694002](https://github.com/haohaomin/cursor-byok/actions/runs/40435694002) 全部成功：两平台各 7 项采集、6 项规则、4 项快照；Linux 491 项通过、1 项忽略。它不覆盖随后的执行策略修正 |
| 真实 Cursor SSH | Cursor 3.27.14：Search/Related 各一次成功；排除正文未传输；144 文件分成 128+16 两批，Unicode 路径正确；6,144 文件搜索成功；采集中 Stop 中止 Exec，结果不再交付 |
| 模型历史与内部数据 | 同一测试对话 18 个供应商请求逐项比较：前次消息均为下次的完整结构前缀；工具定义摘要一致；未出现采集指纹、正文 base64 或排除文件 canary |
| 大文件集 / 审批 / Windows UI | 18,144 文件的截断问题已修复并通过真实 SSH 分页复测；实际审批弹窗、Windows Cursor UI 尚未验证。测试环境已恢复、含凭据临时副本已删除 |

模型测试默认 `ignored`，避免普通 CI 隐式下载模型；手动执行：

```sh
cargo test -p cursor-server --lib real_model_host_exec_round_trip_search_and_related -- --ignored --nocapture
cargo test -p cursor-server --lib search::snapshot::tests
cargo test -p cursor-server --lib cursor::tools::semble::tests
cargo test -p semble-core --test inventory_policy --test request_snapshot
```

## 当前限制与合入门槛

工作区主机需要 Python 3.9+；Git 仓库需要可用 Git。POSIX 启动器可选择主机已安装的新版本 Python，不安装依赖。Windows 使用 PowerShell 和已安装 Python，编码后的命令有长度上限。

当前显式限制：20,000 个枚举项、每份规则 32 KiB / 总计 256 KiB、单源码文件 1,000,000 字节、选中正文总计 16 MiB；单批最多 128 文件 / 2 MiB。元数据超过限制或采集超过时间预算时报告失败，不返回成功的部分搜索。目录必须是物理路径；不把链接路径静默换成本机路径或其他目录。每次搜索重新采集并在内存建立索引，尚未证明大型工作区性能。

合入前仍需验证真实审批。大元数据及接近传输上限的正文分页已在真实 Cursor SSH 验收。已通过的 CI、两种查询、排除正文、取消和 UI 结果展示不能代替这些剩余项。结果不足时继续保留开发状态，不发布为修复完成。
