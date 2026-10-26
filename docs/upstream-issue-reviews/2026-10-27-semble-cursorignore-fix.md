# Semble：尊重 .cursorignore，并在查询前重新检查排除规则

**已修复本机 Semble 索引忽略 `.cursorignore`、热缓存短暂绕过新增排除规则的两个缺口。远程 Semble 的采集与来源隔离仍未实现，Issue #469 / PR #498 不标记为整体解决。** 本次是审查远程快照时发现的共享索引层问题；没有采用原 PR 的快照代码。仅源码修复，v1.0.17 安装包不包含。

## 修改前后证据

基线为 `95a9b8ff057f0f8d454101be5364598874a9e325`。同一可重跑探针分别运行原基线、只增加 Cursor 过滤的中间版本、完整修改版本；使用真实 `StaticEmbedder` / `minishlab/potion-code-16M-v2`（256 维），没有网络模型调用。夹具包含 20 个目录、1000 个公开 Python 文件和 1 个合成排除文件。

先索引并确认能找到 `private.py`，随后写入 `.cursorignore`，立即查询，不通过等待来避开缓存问题。

| 检查 | 原基线 | 仅增加过滤 | 完整修复 |
| --- | --- | --- | --- |
| 新规则生效后的首次热查询排除文件 | 失败 | **失败** | 通过 |
| 相关代码查询拒绝被排除文件作为输入 | 失败 | **失败** | 通过 |
| 新引擎加载磁盘缓存后排除文件 | 失败 | 通过 | 通过 |
| 全新缓存的冷查询排除文件 | 失败 | 通过 | 通过 |
| 冷建索引时，被排除正文进入嵌入器的次数 | 1 | 0 | **0** |
| 更新后的索引文件数 | 1001 | 1000 | 1000 |

[结构化证据](2026-10-27-semble-cursorignore-evidence.json)保留三组原始结果与计时；[探针源码](reproductions/semble_cursorignore.rs)保留合成输入及观察逻辑。探针输出结果，不自行把失败状态当成修复成功；本轮另外断言完整版本四项排除均为 true、冷建正文编码次数为 0。

## 代码与执行边界

```text
crates/semble-core/src/
├── source/
│   ├── local.rs       # 既有 Git/.ignore/.sembleignore 候选文件选择
│   └── cursor.rs      # 新增独立 .cursorignore 过滤，复用 ignore 库
├── index/builder.rs   # 每次检查文件集合与时间戳，复用未变分块及向量
└── search/engine.rs   # 普通搜索和相关代码查询都进入上述检查

查询 / prepare / refresh
    → 解析本机来源
    → 既有候选文件 ∩ Cursor 允许文件
    → 文件集合与时间戳比较
        ├──未变：复用内存或磁盘索引
        └──变化：复用未变文件向量，重建索引并保存
    → 检索、读取片段 → 返回路径和行号
```

两个独立 walker 使 `.sembleignore` 的 `!` 规则不能取消 Cursor 排除，Cursor 的 `!` 规则也不能恢复已被第一层排除的文件。第二次遍历只进入包含候选文件的分支，不遍历被第一层排除的整个目录树。父目录、嵌套目录及非 Git 目录的 `.cursorignore` 均参与检查；这是 Semble 自身的规则，不读取或声称同步 Cursor 界面中的层级忽略开关。

删除了 `SEARCH_REFRESH_INTERVAL`、`CachedIndex` 和 `load_for_search`，不保留绕过规则检查的一秒快捷路径。普通搜索与相关查询统一调用 `load_or_build`；未变化的文件无需重新编码。规则解析错误通过 `DirEntry::error()` 返回错误，不静默使用部分规则。

保证范围是**下一次查询发现文件时**检查规则，不是文件系统快照事务。并发修改、同大小且保留时间戳的正文替换、远程同路径身份隔离仍需各自处理。本补丁也不宣称擦除所有历史缓存、供应商已经接收的正文或其他工具的访问记录。

## 回归与代价

- 新增 5 个测试：独立排除/否定规则；父级/嵌套/非 Git；代码/文档/配置与非法规则；旧磁盘索引更新且公开文件不重新编码；热搜索/相关查询/规则删除/全部文件排除。
- 修改 2 个旧测试，要求文件修改在下一次查询可见，不再断言一秒内返回旧内容。
- 真实模型三组对照中，完整修复的 20 次热查询中位数 **19.80 ms**，P95 **21.04 ms**；原基线中位数 **6.05 ms**。规则变更后的首次刷新 **61.92 ms**。这是 macOS 调试构建、1001 个小文件的单轮观察，同时有全工作区回归负载；不是受控性能基准或生产延迟承诺。每次发现文件带来可见开销，大仓库仍需单独测量。
- `cargo test --workspace --all-targets`：**469 通过、0 失败、0 忽略**，其中 Semble 42 项；通过环境变量提供真实模型资产，模型编码测试实际执行。
- `cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --all -- --check` 通过。提交后的 CI 单独跟踪；本地回归不代替跨平台 CI。

## 重跑真实模型探针

先准备已有的 `model.safetensors` / `tokenizer.json`，不需要供应商密钥。以下脚本只在临时 example 和新建测试目录运行，结束后移除 example；合成文件与 JSON 结果保留在打印的临时目录。macOS 使用可用 SDK，例如本轮的 `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`。

```python
import os, pathlib, shutil, subprocess, tempfile

repo = pathlib.Path.cwd()
assets = pathlib.Path.home() / ".cursor-byok-v3/cache/semble/models/potion-code-16M-v2"
assert all((assets / name).is_file() for name in ["model.safetensors", "tokenizer.json"])
source = repo / "docs/upstream-issue-reviews/reproductions/semble_cursorignore.rs"
example = repo / "crates/semble-core/examples/semble_cursorignore.rs"
assert not example.exists()
directory_existed = example.parent.exists()
example.parent.mkdir(exist_ok=True)
run = pathlib.Path(tempfile.mkdtemp(prefix="semble-cursorignore-")) / "run"
try:
    shutil.copyfile(source, example)
    subprocess.run(["cargo", "run", "-p", "semble-core", "--example",
                    "semble_cursorignore", "--", str(run), str(assets)], check=True)
finally:
    example.unlink(missing_ok=True)
    if not directory_existed:
        example.parent.rmdir()
print(run / "result.json")
```

要重跑原基线对照，将本探针复制到 `95a9b8f` 的独立源码目录，执行相同命令。仅过滤对照使用该基线加当前 `source/{cursor,local,mod}.rs` 三个文件；保持基线索引和搜索代码不变。

## 与远程问题的关系

[真实 SSH 原生工具采集实验](2026-10-27-issue469-native-files-check.md)确认，不能直接将 Glob 或 Grep 的输出当作安全快照清单：前者包含 Git 排除文件，后者跟随工作区内的符号链接。Read 的分页输出也不等同于原始文件字节。

远程修复仍需在正文传输前完成过滤、确认普通文件边界，绑定来源和快照身份，处理审批、取消、迟到结果、容量限制与清理。当前本机规则修复不能代替这些条件，也不能让 BYOK 本机读取到远端独有路径。上游仓库仍归档只读，本次未发评论、未发版。
