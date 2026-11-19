# Issue #302：明确后台终端文件名后缀

日期：2026-11-13。上游 Issue：[leookun/cursor-byok#302](https://github.com/leookun/cursor-byok/issues/302)。本 fork 候选：[PR #49](https://github.com/haohaomin/cursor-byok/pull/49)。

## 问题

后台 Shell 的 ID 是数字，但服务端持久化的终端文件名是 `<shell_id>.txt`。模型可能根据裸 ID 生成 `Read(terminals/<shell_id>)`，收到文件不存在错误；原提示词虽然提到 `*.txt`，却没有把 Shell ID 和实际路径后缀明确关联起来。

## 实现

在 Agent、Ask、Debug、Multitask、Plan 和 Subagent 六个内置模式的终端文件说明中增加同一条明确规则：Shell ID 只是数字文件名主体，终端路径始终以 `.txt` 结尾，并给出完整路径示例 `terminals/610184.txt`。

本次只修改提示资产和嵌入提示回归测试，没有修改终端协议、文件存储或运行时逻辑。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |
| 六个嵌入提示的 Python 检查 | 通过，6/6 |
| macOS 定向 Rust 测试 | 被现有 `sqlx_macros` 动态库 `mis-aligned LINKEDIT string pool` 装载错误阻断，未进入测试执行 |
| Oracle Linux 9.8 ARM64：`cargo test --locked -p cursor-server --lib cursor::prompting::assets::tests::embedded_mode_prompts_identify_terminal_file_suffix` | 通过，1 passed、0 failed、305 filtered |
| Oracle Linux 9.8 ARM64：`cargo clippy --locked -p cursor-server --lib -- -D warnings` | 通过 |

远程验证脚本以分支 `codex/issue302-terminal-file-suffix` 为输入，退出码为 0。该候选尚未进行真实 Cursor UI 验收；提示资产修复不应被描述为已覆盖所有终端交互场景。
