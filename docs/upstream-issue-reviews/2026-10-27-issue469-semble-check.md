# Issue #469 / PR #498：远程 Semble 失败基线与候选补丁审查

**远端独有路径的 Semble 搜索已复现失败，尚未修复。PR #498 的快照部分不原样采用：原命令不能启动；仅修正启动参数后，仍有忽略规则失效和跨来源索引复用问题。** 这是基础路由恢复后的独立工具缺口，不改变此前基础 Agent 与 WebFetch 的验收结果。

## 固定版本与验证范围

- 当前源码：`3d309480d55ece3021688dd36228ad48b76cd56c`。
- [PR #498](https://github.com/leookun/cursor-byok/pull/498)，作者 sunrisegm；2026-10-27 重新读取，head 仍为 `c7114878c33076340c0c2f1e77ce4e7512d1960e`，OPEN、未合并。
- 原 `server/src/search/host_snapshot.rs` 整个文件由固定 head 获取，与此前完整新增文件 patch 逐字节一致。Git blob：`db52774199f7bcec1c9810bcca028fe0677f3e48`；SHA-256：`d5e9041ed16f07a27c96d9c283bedc80a85a3fe63761426fec7a674779bd293e`。
- 使用真实 `StaticEmbedder`（`minishlab/potion-code-16M-v2`）、当前 `SearchEngine` / `IndexRepository` / `ToolDispatcher`。已有模型资产复制到临时目录，索引与快照也只写入临时目录。
- 文件全部为本次生成的测试代码。SSH 只用于独立核实远端文件确实存在；本轮没有发送真实 Cursor 对话、调用供应商或运行整个 PR。跨来源测试使用本机两个目录模拟两台主机的快照，不能称为真实双 SSH 会话验收。
- [结构化证据](2026-10-27-issue469-semble-evidence.json)；[可重跑探针](reproductions/issue469_semble.rs)。探针断言的是本轮缺陷仍存在，不是修复后的 CI 测试。

## 1. 当前源码：搜索发生在错误的主机

在 Linux SSH 主机新建 `/tmp/cursor-byok-semble-eza2p1hj/auth.py`，内容只有 `remote_semble_probe` 和合成标记。SSH 核实文件存在；macOS BYOK 主机同路径不存在。

| 对照 | 实际结果 |
| --- | --- |
| 当前 `ToolDispatcher` 分派该绝对路径的 SembleSearch | 产生 **0** 个 Cursor Exec 请求，运行中的 Exec 列表为空 |
| 当前真实搜索引擎搜索该远端路径 | `source does not exist: /tmp/cursor-byok-semble-eza2p1hj` |
| 相同文件复制到本机独立对照目录，再搜索相同符号 | 正确返回 `auth.py:1–2` 及完整测试标记 |

```text
当前结构
server/src/cursor/tools/tool_call_dispatch/search.rs  # 启动本机搜索任务，没有 Cursor 文件请求
    ↓
server/src/search/search_provider.rs                  # 将 repo 原样交给本机引擎
    ↓
crates/semble-core/src/source/security.rs              # 在 BYOK 主机检查路径是否存在
    ↓
本机路径不存在 → 工具失败；同名路径存在时也不能证明它属于远端工作区
```

这组证据定位了主机归属边界；没有用引擎测试冒充完整 Cursor 界面验收。

## 2. 原候选命令：脚本参数占用了仓库参数

直接调用固定 head 的 `snapshot_shell_command`，在真实存在的合成 Git 仓库执行，退出码为 **2**，stdout 为：

```json
{"v": 1, "ok": false, "error": "not a directory"}
```

启动器形如 `python3 -c '…exec(base64.b64decode(sys.argv[1]))' SCRIPT ROOT`。被执行的脚本也把 `sys.argv[1]` 解码成仓库路径，因此读到的是脚本文本。此前仅单独运行 `DUMP_SCRIPT` 的测试没有覆盖这条实际启动命令；本轮补齐这个边界。

为检查后续风险，探针**只在临时执行字符串中**将启动器改为 `code=base64.b64decode(sys.argv.pop(1));exec(code)`，使脚本看到正确的 ROOT。原文件未修改，未合入生产代码。下面的过滤发现以这个参数修正为前提，不能宣称原 PR 已在真实用户环境上传了文件。

## 3. 修正参数后：先传输忽略文件，且 Git 排除在索引阶段失效

夹具包含三个排除文件和一个公开文件。`git check-ignore -v ignored.py` 确认 `.gitignore:1` 命中。

| 文件 | 原目录的当前 Semble 扫描 | 候选快照序列化 | 候选快照落盘后的当前扫描 |
| --- | --- | --- | --- |
| `visible.py` | 包含 | 包含 | 包含 |
| `.gitignore` 排除的 `ignored.py` | 排除 | **包含内容** | **包含** |
| `.sembleignore` 排除的 `semble-private.py` | 排除 | **包含内容** | 排除 |
| `.cursorignore` 排除的 `private.py` | 包含 | 包含内容 | 包含 |

原因分两层：候选 Python 遍历只排除固定目录名，不应用 ignore 文件；其落盘快照又没有 `.git` 标记，当前 `ignore::WalkBuilder` 的 Git 仓库条件不再成立。真实搜索引擎在落盘快照中查询 `gitignored_probe`，成功返回了原本被 Git 排除的 `ignored.py` 正文。

`.cursorignore` 是当前本机 Semble 也未支持的独立边界，不能全部归为 PR 新回归。`.sembleignore` 虽然在最终索引中生效，但正文已经进入快照序列化；在接收端补过滤不能消除这一步传输。

## 4. 相同远端路径：第二个来源拿到第一份索引

候选 `source_identity` 只计算远端路径的散列。按候选 `search_prepared` 的调用方式，向当前 `IndexRepository::load_for_search` 连续传入两个不同快照目录、相同 `/workspace/project` 身份：

- A 目录只有 `alpha.py`，B 目录只有 `beta.py`。
- 第二次调用在一秒刷新间隔内直接返回与 A **同一个 Arc 索引**，文件列表仍是 `alpha.py`。
- 对 B 强制重新扫描后，文件列表才变成 `beta.py`。

这证明候选来源标识和当前快速缓存不适配。未测试真实并发 SSH、供应商内容泄漏或进程重启；结论限于错误复用索引，不能扩展成已发生跨会话泄漏。

## 后续实现与验收要求

远程实现仍需完成，不能用当前本机路径访问或原样快照代替：

```text
拟实现的数据流（尚未合入）
Cursor 工作区主机
    ├──按该工作区的排除规则选择文件   # 正文传输前完成过滤
    └──通过 Cursor Exec 交付所选内容  # 保留审批、拒绝、超时和取消
               ↓
BYOK 搜索适配层                       # 绑定来源、会话与本次不可变快照，约束容量与生命周期
               ↓
Semble 索引                           # 正确区分来源及快照版本，复用现有检索实现
               ↓
返回工作区路径、行号与片段 → 同一工具调用完成 → 模型继续
```

验收要覆盖远端独有路径、同路径不同主机、忽略规则及嵌套规则、符号链接、容量上限、审批拒绝、采集中取消、索引中取消、迟到/重复结果、文件更新和临时快照清理。不得在失败时静默转而读取 BYOK 本机同名目录。当前没有选定或实现完整采集机制，也没有声称上述边界通过。

本次只新增审查证据和探针、修正清单与用户指南；没有生产代码修复，没有发版。SSH 测试目录在核对唯一测试正文后清理，本机正式代理、CA、Cursor 设置、数据库均未修改。上游仍归档只读；未发送评论。

## 重跑探针

在本仓库根目录运行下列 Python。它校验固定上游文件的 SHA-256、临时放入 Cargo example、使用已有本地模型资产，并在结束后移除本次 example。输出和合成文件保留在新建的临时目录。所需工具为 `gh`、Git、Python 3、Rust；macOS 本轮使用 `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`。

默认的不存在路径只重现本机路径解析失败；如需复现 SSH 对照，应先在远端创建同样的 `auth.py`，独立确认其存在，再将 `remote` 改为对应路径。探针不会连接 SSH。

```python
import base64, hashlib, json, os, pathlib, shutil, subprocess, tempfile

repo = pathlib.Path.cwd()
parent = pathlib.Path(tempfile.mkdtemp(prefix="issue469-semble-audit-"))
root = parent / "run"
remote = str(parent / "not-on-byok-host")
assets = pathlib.Path.home() / ".cursor-byok-v3/cache/semble/models/potion-code-16M-v2"
example = repo / "server/examples/issue469_semble.rs"
source_dir = example.parent / "issue469_snapshot_source"
assert not example.exists() and not source_dir.exists()
assert all((assets / name).is_file() for name in ["model.safetensors", "tokenizer.json"])
data = json.loads(subprocess.check_output([
    "gh", "api", "repos/leookun/cursor-byok/contents/server/src/search/host_snapshot.rs"
    "?ref=c7114878c33076340c0c2f1e77ce4e7512d1960e"
]))
source = base64.b64decode(data["content"])
assert hashlib.sha256(source).hexdigest() == "d5e9041ed16f07a27c96d9c283bedc80a85a3fe63761426fec7a674779bd293e"
source_dir.mkdir(parents=True)
try:
    (source_dir / "mod.rs").write_bytes(source)
    shutil.copyfile(repo / "docs/upstream-issue-reviews/reproductions/issue469_semble.rs", example)
    env = dict(os.environ, SEMBLE_CACHE_LOCATION=str(root / "cache"))
    subprocess.run(["cargo", "run", "-p", "cursor-server", "--example", "issue469_semble",
                    "--", str(root), remote, str(assets)], env=env, check=True)
finally:
    example.unlink(missing_ok=True)
    (source_dir / "mod.rs").unlink(missing_ok=True)
    source_dir.rmdir()
print(root / "audit-result.json")
```
