# #420 验证记录：MCP schema 整数约束规范化

当前状态：已在隔离分支修正原补丁的大整数精度问题，新增边界测试 8/8 通过；完整回归结果见文末。以下保留原 PR 的失败证据。

- 日期：2026-10-02（北京时间）。
- PR：https://github.com/leookun/cursor-byok/pull/420
- 固定 head：`4d51f9714057f83c347877e436ebc5c7190f4da6`；查询时为 OPEN / Draft。
- 本地基线：`8daa330c248e1f6d4ed56982e5949b838c926342`（已包含 v1.0.6 修复）。
- 隔离分支：`codex/verify-pr420`。原补丁可无冲突应用。
- 已按用户要求在隔离分支补齐修复，未合并主分支、推送、发布或替换本机应用。

## 执行路径

```text
AgentRunRequest / RequestContext
    ↓ MCP 描述：优先 input_schema_json，否则解码 protobuf Value
server/src/cursor/compile/context.rs
    ├─ dynamic_mcp：保留原始 wire 描述供 Cursor 执行
    └─ normalize_mcp_parameters：规范化模型侧工具 schema
           ↓ BTreeMap，确定性排序
server/src/cursor/compile/run.rs
    └─ PromptSpec.tools / checkpoint_prompt：编译模型侧工具定义
           ↓
server/src/provider/openai_responses.rs
    └─ tools[].parameters：序列化至供应商请求
```

本补丁没有修改历史消息、数据库或 checkpoint 恢复逻辑；新增测试确认相同 MCP 输入重复编译得到相同 schema，原始 wire 描述保持不变。未发生模块移动、合并、拆分或删除。

## 对照测试

增加 7 项 `pr420_validation` 测试，先在原版运行，再应用原 PR 运行。

| 场景 | 原版 | #420 |
| --- | --- | --- |
| protobuf 数字规范化、原始描述保留、重复编译一致 | 失败（仍为 1.0） | 通过 |
| 8 种整数约束与嵌套 schema 容器 | 失败 | 通过 |
| 根 object union 补全与非法根拒绝 | 失败（嵌套约束仍为 1.0） | 通过 |
| examples/default/const/enum/扩展字段及普通数值约束不被改写 | 通过 | 通过 |
| 负数、小数、超范围值及非数字不被强制转换 | 通过 | 通过 |
| 规范化幂等 | 通过 | 通过 |
| JSON 字符串输入中的既有大整数保持精确 | 通过 | **失败** |

原版 4/7 通过；#420 为 6/7。原 PR 自带的 2 个测试另在回归中运行。

## 确认的问题

输入通过正式 `dynamic_mcp` 的 `input_schema_json` 分支，避免 protobuf 本身只能表示双精度数字的影响：

```json
{"type":"object","maxProperties":9007199254740993}
```

期望 `maxProperties` 保持 `9007199254740993`；应用 #420 后实际成为 `9007199254740992`。

根因：`normalize_json_schema_integer_constraints` 对所有 JSON Number 调用 `as_f64()`，再经字符串解析回 `u64`。精确整数在第一次转换时已丢失精度。这是原版不存在的新回归。场景罕见，但规范化不能静默修改已有合法约束。

建议最小修复：已是整数表示的 Number 原样保留，仅处理浮点表示，并对浮点转整数的范围及精确性增加断言。应以该失败测试转绿作为继续评估合并的条件；该建议已在后续修复中实现，见文末。

## 验证边界

这是实际 Rust 编译路径及服务端自动测试，没有请求真实模型接口，也没有进行真实 Cursor UI 操作。验证证实模型侧 schema 的 JSON 数字表示变化，不能宣称已用真实供应商复现或消除接口拒绝。

原始日志：`/tmp/cursor-pr420-20261002/`。结构化结果见 [证据](2026-10-02-pr420-evidence.json)。本机使用 MacOSX26.5 SDK 与 Rust 自带 ld64.lld，未更改全局工具链。

## 回归结果

- 排除上述已确认失败的 1 项后，`cargo test -p cursor-server --all-targets -- --skip pr420_validation::json_schema_large_integers_survive_exactly`：295 项通过，1 项筛除。包含原 PR 的 2 个测试、其余新增边界测试，以及 prefix_stability、checkpoint_recovery、runtime_modes、compaction、MCP 工具链路等现有套件。
- `cargo clippy -p cursor-server --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check` 和 `git diff --check`：通过。
- 未将筛除已知失败后的回归描述为全部验证通过；#420 当前结论仍为需修正后再评估。

## 用户授权后的修复

生产修改仍仅位于 `server/src/cursor/compile/context.rs` 的模型侧 schema 规范化函数：

1. 已有整数类型原样保留，不经过 `f64`。
2. 仅对浮点类型检查非负、无小数部分且小于 2^64；满足条件后直接转换为整数。
3. 不再使用浮点数的十进制显示字符串解析整数，避免显示舍入改变已有浮点值。
4. 上界使用严格小于，因为 `u64::MAX as f64` 已舍入到 2^64；避免 Rust 浮点转整数的饱和行为。

补充的边界用例覆盖 `-0.0`、0、1、2^53、2^63、2^64 以下最后一个可表示浮点整数，以及 2^64 和其上方值、负整数与负小数。补充用例在原补丁上与既有大整数测试共同失败（6/8 通过），修复后 8/8 全通过。

protobuf 在接收前已经丢失的数字精度无法由该修复恢复；本修复确保不进一步损坏已有整数，且转换忠实于收到的浮点值。

## 修复后的最终验证

- 新增专项测试：8/8 通过，未筛除用例。
- `cargo test -p cursor-server --all-targets`：297 项通过，0 失败、0 忽略、0 筛除。包含原 PR 的 2 项测试和现有前缀、checkpoint、压缩及工具回归。
- `cargo clippy -p cursor-server --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`：全部通过。
- 日志：`fix-red.log`、`fix-green.log`、`fixed-regression.log`、`fixed-clippy.log`，均位于上述临时日志目录。
- 当前结论：原补丁的精度缺陷已在候选分支补齐，服务端验证通过；尚未真实 Cursor UI 复测、合并、发布或替换本机应用。
