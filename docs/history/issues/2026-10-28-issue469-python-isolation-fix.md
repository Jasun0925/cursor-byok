# Issue #469：工作区采集器的 Python 导入隔离

> 历史记录：状态、版本、路径及测试数仅对应记录日期；当前交付状态见 [工作状态](../../work-status.md)。复现脚本按文中基线使用，不作为当前生产操作步骤。

**已复现并修复开发分支中的独立缺口：采集命令会导入工作区或 `PYTHONPATH` 内的同名 Python 模块，导致搜索失败，并执行本不应作为依赖加载的项目代码。** 这不是 #469 原始代理连接或 CA 报错的根因结论。远程 Semble 仍保留开发状态，真实审批与 Windows Cursor UI 尚未验收，未合入 main、未发布。

生产代码基线：`d89d52a2fdc1f77822c69c836a563561795d1daf`。本轮重新读取上游 #469 正文与评论，GitHub 状态仍为 OPEN；原报告是 SSH 代理连接和证书错误，没有报告本次发现的 Python 导入问题。

## 失败基线

真实 Shell 请求将 `working_directory` 设置为模型选择的工作区，但此前主机测试没有切换到这个目录。Python 的默认启动方式将当前目录加入模块搜索路径，也会读取 `PYTHONPATH`。启动代码及采集器使用 `base64`、`json` 等标准库名称时，可能加载项目中的同名文件。

新增两项原生命令回归，先在未修复的生产代码上运行：

| 场景 | 预期 | 基线结果 |
| --- | --- | --- |
| 工作区含 `base64.py`、`json.py`、`sitecustomize.py`，并用 `.cursorignore` 排除 `*.py` | 两阶段采集只返回 `public.rs`，不执行被排除的模块，不生成工作区缓存 | 元数据阶段失败；堆栈指向工作区 `base64.py`，出现 `RuntimeError: WORKSPACE_CODE_EXECUTED` |
| `PYTHONPATH` 指向隔离的合成目录，其中含同名 `base64.py` | 完成采集，环境目录内代码不作为采集依赖执行 | 元数据阶段失败；堆栈指向环境目录 `base64.py`，出现 `RuntimeError: ENVIRONMENT_CODE_EXECUTED` |

基线命令为 `cargo test --package cursor-server --lib search::snapshot::tests::host::real_host_does_not_import -- --nocapture`，macOS / Python 3.9.6 上结果 **0 通过、2 失败**，执行耗时 0.07 秒（不含编译）。所有文件均为临时合成夹具，失败时也由临时目录所有者清理。

## 修改与验证

```text
server/src/search/snapshot/
├── command.rs  # POSIX / Windows 的版本检查和采集都用 Python -I -S
└── tests.rs    # 在实际工作目录执行命令，验证同名模块和环境路径不被导入
```

`-I` 隔离当前目录、用户模块路径与 `PYTHON*` 配置；`-S` 跳过 site 初始化及其启动钩子。采集器只依赖标准库，没有增加依赖、临时脚本或工作区写入。两个平台均在版本检查和正式执行时使用同一组参数，避免检查阶段先加载项目的 Python 配置。

修复后本机 `cargo test --package cursor-server --lib search::snapshot:: -- --nocapture` **14 项通过、0 项失败**。两项新增用例均实际执行生成的主机命令，完成元数据和正文两阶段，检查返回路径以及未产生 `__pycache__`；工作区同名模块用例还检查顶层文件数量没有变化。既有分页、大正文、Unicode、多批、过滤、链接与变化检测同时通过。

完整回归首次还暴露一项与采集器无关的环境问题：`provider::attempt::tests::request_transport_failure_is_one_failed_attempt` 期望连接失败，但实际经本机代理得到 `Provider("test 502 Bad Gateway: ")`；单独复跑得到相同结果。三个 HTTP attempt 夹具改为 `no_proxy()` 直连自己创建的本机测试端口，修复后均通过。该修改仅在测试模块内，不改变生产供应商请求的代理选择。

本机完整检查：

- `cargo test --package cursor-server --all-targets`：**447 项通过、0 项失败、1 项忽略**。忽略项需要显式提供真实嵌入模型，本轮未执行。
- `cargo clippy --package cursor-server --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。

代码提交 `75caeaefa743815c2051d1f91df1eace92fc8092`（采集隔离）及其父提交 `d1084f3`（HTTP 测试直连）已推送到 `codex/remote-semble`。[CI 40577400975](https://github.com/haohaomin/cursor-byok/actions/runs/40577400975) 核对 head 为上述完整代码 SHA，四组全部成功：

| 作业 | 结果 |
| --- | --- |
| Rust Linux | 工作区 **510 项通过、0 失败、1 忽略**；格式及 Clippy 通过 |
| Windows | 原生采集 14、选择与快照 10、SQLite 6，合计 **30 项通过**；Clippy 通过 |
| macOS | 原生采集 14、选择与快照 10、SQLite 6，合计 **30 项通过**；Clippy 通过 |
| 前端 | 类型检查与生产构建通过 |

逐项核验 CI 日志，两项新增 `real_host_does_not_import_*` 在 Linux、Windows、macOS 均实际执行并通过；没有用本机结果替代 Windows 命令运行。随后仅追加此 Markdown 验证结果，不改变已测代码。

本轮还重新尝试访问 Cursor，界面工具明确返回 **Mac 已锁定，自动解锁失败**，已向用户请求解锁。因此真实审批继续待验收，自动化不能代替它。本轮没有发送 Agent 请求、改动正式代理或生产数据，也没有执行 20 分钟任务。

## 最新分支的实际 SSH 主机复测

2026-10-28 在 `f90cb668497da2bf0bcc30298b1d504d6df30995` 上补测实际 `aliyun-server` Linux 主机。这里的 `python3` 为 **3.6.8**，生产命令按既有探测顺序选中 **Python 3.13.13**。测试直接编译当前 `Collector` 模块，由它生成每阶段命令并验证返回分页、指纹及正文；通过 SSH 在临时工作区执行原始命令，没有另写采集算法。见[脱敏证据](2026-10-28-issue469-ssh-isolation-evidence.json)和[命令生成与校验夹具](reproductions/issue469_remote_collector.rs)。

| 远端场景 | 实际结果 |
| --- | --- |
| 工作区含 `base64.py`、`json.py`、`sitecustomize.py`，用 `.cursorignore` 排除，同时设置含同名模块的 `PYTHONPATH` | 元数据及正文各执行一次，均退出 0；只返回 `receipt.rs`，同名模块内的失败标记未触发 |
| 工作区只有 `receipt.rs`，`PYTHONPATH` 仍指向含失败标记的 `base64.py` | 两阶段均退出 0，只返回 `receipt.rs` |

两组都返回 66 字节的预期源码，SHA-256 均为 `ed96d7d25eb2698908b839cdaa8f4c7cd185f661ff33fd384316ef2cc41ee4c6`。四份响应的分页长度及摘要均通过校验；工作区顶层条目保持不变，两个工作区和环境模块目录均未生成 `__pycache__`。带本轮标记的远端临时目录已删除。

这项结果证明最新采集器在实际 SSH 主机上的解释器选择、导入隔离及两阶段传输。**命令由 SSH 工具发送，没有经过 Cursor Agent、模型调用或审批弹窗，不能计为 Cursor 端到端验收。** Windows Cursor UI 仍待验证。

### 首次真实审批尝试

Mac 解锁后可读取设置，实际 Run Mode 为 **Allowlist (with Sandbox)**，Command Allowlist 含 `python3`。这不能仅凭先前的“已改为逐次审批”确认审批必然出现，也不是认定本次采集命令必然被自动批准的证据。

新建独立空白窗口后，原生菜单可以操作，但坐标点击继续返回 `noWindowsAvailable`，点击设置后窗口标题与实际渲染内容不一致。未成功发送测试提示、未观察到批准或拒绝弹窗。已构建最新隔离服务夹具，但没有启动它，也没有切换代理、修改审批模式、复制正式凭据数据库或重启 Cursor。尝试关闭空白窗口同样返回上述错误，窗口是否关闭未确认。正式后端健康检查为 204；20 分钟任务未执行。

### 解锁后续测：隔离服务可用，真实审批仍未验收

后续在 `c7c03945a45db1722a77e3fb5b325d7fc092c037` 基线上改进[实测服务夹具](reproductions/issue469_snapshot_live.rs)：移除会接管 Cursor 配置的 `CursorHarness`，直接复用生产代理和 CA 模块，只启动独立的后端与代理。夹具不再负责修改设置或重启 Cursor；操作者在确认测试窗口可用后才配置路由，并负责恢复。运行时要求传入完整源码 SHA，替换原先写死的旧版本标签。

将夹具复制到 `server/examples/issue469_approval_live_20261028.rs` 后，`cargo build --package cursor-server --example issue469_approval_live_20261028` 及定向 rustfmt 检查通过。通过 SQLite backup API 创建权限受限的数据库副本，实际启动并停止隔离服务，结果见[结构化证据](2026-10-28-issue469-approval-attempt.json)：

- 隔离后端健康检查为 **204**；本机代理的 HTTPS `NetworkService/IsConnected` 探测为 **200**，使用已有本机 CA 验证证书。
- 启动前、运行中和停止后，Cursor 主进程 PID 均为 `19578`；设置文件 SHA-256 完全一致，正式代理没有切换。
- SSH 连接及合成目录正常；曾通过原生菜单进入 SSH 窗口，但未成功打开新的测试目录并发送提示。用户确认解锁后，界面捕获仍连续报 `ScreenCaptureKit -3811`，重建界面工具会话也未恢复。
- 隔离数据库的 `cursor_run_traces` 始终为 **0**。未发送 Agent 请求，**批准、拒绝两项均未验收**；健康检查不能替代真实审批证据。

远程验收继续沿用已验证的 `cursorAgentHost.remoteInferenceRoute=always` 本机转发路径。辅助远端直连代理的 TLS 探测因远端缺少本机 CA 而失败，但本轮未进入该路径的 Cursor 请求，不能据此归因 #469。没有上传 CA、关闭 TLS 验证或写入远端 Machine 设置。

结束时隔离进程已正常退出，独立端口关闭，含凭据的临时数据库副本、设置备份、临时 example 和带标记的远端合成目录已清理；没有活动 SSH 转发。正式后端仍为 **204**。界面工具不可用，测试窗口是否关闭无法核验。用户确认没有 OAuth 测试账号，真实 OAuth 验收保留未完成。未运行 20 分钟任务；本次仅改测试夹具与记录，不代表生产功能完成验收，也未合入 main 或发布。
