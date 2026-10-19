# 本地修复分支合并记录 · 2026-10-19

四个剩余修复分支均已合入本地 `main`。其中三个分支的代码早已由其他提交进入发布线，本轮合并只补齐提交历史；真正新增的生产修改是 #420 的 MCP schema 整数约束规范化。未推送远端、未发布新版本、未替换正式应用。#155 仍按既有验收记录保持“完成一半”。

## 分支与结果

| 本地分支 | 来源提交 | 本轮合并提交 | 核对结果 |
| --- | --- | --- | --- |
| `codex/upstream-anthropic-refusal` | `24d9850` | `505546d` | `89d90c6` 已包含对应修复；合并前后文件树完全相同 |
| `codex/upstream-responses-text` | `e1e784d` | `949cac3` | `a5de72d` 已包含对应修复；保留后续 completed snapshot 处理和测试，合并前后文件树完全相同 |
| `codex/upstream-task-result-delivery` | `480d357` | `d83ddac` | `51ee8e7` 已包含对应修复；保留后续后台通知去重及规则测试，合并前后文件树完全相同 |
| `codex/verify-pr420` | `4340952` | `e2f17e0` | 合入整数约束修复及原候选测试，保留 main 新增的根 schema 修复，并补一项组合回归 |

前三个分支不是通过 `git cherry` 判定为同一补丁：它们是从上游基线提取的独立投稿提交，和 fork 中包含文档及后续演进的提交不同。核对依据是原修复提交、三方合并差异及最终文件树；没有将旧实现覆盖回当前版本。

## #420 的实际改动

- `server/src/cursor/compile/context.rs`：仅对 schema 的八种整数约束字段转换非负、整数值、严格小于 2^64 的浮点数；已有整数保持精度，普通数值约束、注释和示例保持原样。
- 保留当前空 schema 补全、缺失 `properties` 补全和 `required: null` 转空数组的行为；候选分支中的旧根 schema 实现不覆盖这些修复。
- 原 wire 描述保持不变，模型侧工具定义按既有顺序编译；相同输入重复编译一致。未改写历史消息、检查点或压缩逻辑。
- 两个比较整个 schema 的候选测试显式带上空 `properties`，以测试数值转换本身；另加组合回归确认既有根 schema 修复与整数转换共存。

## 本轮验证

| 检查 | 结果 |
| --- | --- |
| 合入前，将候选的八项边界测试置于当前 main 的实现上 | 4 通过、4 失败；失败来自浮点整数未被规范化 |
| 合入后，全部 context/MCP 定向测试 | 21 通过、0 失败 |
| `cargo test --locked -p cursor-server --all-targets` | 26 个测试结果、387 通过、0 失败、0 忽略，无用例筛除 |
| `cargo clippy --locked -p cursor-server --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all -- --check`、`git diff --check` | 通过 |

首次全量构建被环境中的 `SDKROOT=MacOSX.sdk` 覆盖仓库本地配置所阻断，LLVM 无法解析 macOS 27 SDK 的 `arm64e.x1`。指定已安装的 26.5 SDK 后，既有的连接失败测试仍受本机代理影响：单独运行失败，设置回环地址代理绕过后通过。最终全量检查仅在子进程中设置如下环境，未修改正式代理、全局工具链或仓库构建配置：

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
NO_PROXY=127.0.0.1,localhost,::1
no_proxy=127.0.0.1,localhost,::1
```

完整日志保存在本机临时目录 `/var/folders/94/7x3w4znd6fl1k54t41jbw46m0000gn/T/cursor-byok-branch-merge-20261019-p0teyaqo/`。本轮是 Rust 自动化验证，没有新增真实 Cursor UI 或供应商测试。

四个本地分支在确认其末端均为 `main` 祖先后删除，提交通过上述合并提交保留。远端投稿分支及上游 PR 状态未修改；本轮未重新查询上游 PR 状态。
