# 上游 PR #427：向 Cursor AvailableModels 提供上下文上限

日期：2026-11-13

## 结论

上游 PR #427 的改动在当前 fork 中没有重复实现。Cursor 的 `AvailableModels` 响应此前没有填写 `context_token_limit` 和 `context_token_limit_for_max_mode`（protobuf 字段 15、16），导致客户端无法看到已配置模型的上下文窗口。该补丁已单独实现于本 fork PR [#48](https://github.com/haohaomin/cursor-byok/pull/48)，没有修改插件模型的协议声明。

## 实现范围

- `server/src/cursor/services/model_catalog.rs`
  - 内置模型从 `context_window_tokens` 填充两个上下文上限字段。
  - 插件模型继续返回 `None`，因为插件模型没有统一的上下文窗口来源。
  - 使用 `i32::try_from`，超出 Cursor protobuf `int32` 范围时省略字段，不发生截断。
  - 增加回归测试，确认 272,000 token 配置会同时出现在两个字段中、插件模型不宣告内置限制，且超大值不会截断。

## 验证

| 检查 | 结果 | 说明 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 通过 | 在 macOS 工作副本执行 |
| `git diff --check` | 通过 | 无空白错误 |
| `cargo test --locked -p cursor-server --lib cursor::services::model_catalog::tests` | 通过 | macOS；9 passed、301 filtered out；使用 `sqlx-macros.strip=none` 避免既有动态库装载问题；Oracle Linux 9.8 ARM64；9 passed、299 filtered out |
| `cargo clippy --locked -p cursor-server --lib -- -D warnings` | 通过 | Oracle Linux 9.8 ARM64；远程 clippy 组件安装后检查通过 |
| 真实 Cursor 客户端 / provider | 未执行 | 需要运行中的 Cursor 与可用 provider 账号；代码级协议测试不等价于 UI 验收 |

## 与相邻候选的关系

上游 PR #409 处理不可恢复的 provider 状态码；当前 fork 已有 `ProviderRefusal` 生命周期和状态码判断。直接套用 #409 会覆盖现有拒绝处理，因此本轮未重复实现。#427 只涉及模型目录协议字段，边界独立，适合单独审查和回滚。
