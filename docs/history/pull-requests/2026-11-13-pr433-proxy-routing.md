# 上游 PR #433：本地代理路由的选择性实现

## 结论

本 fork 只采用 PR #433 中边界清楚的两项代理行为：

1. Cursor host 的非 Tab 请求统一改写到本地后端，由现有路由继续使用配置的出站代理。
2. CONNECT 隧道保持原始 authority，不参与本地 URI 改写，确保 hudsucker 仍能完成 TLS MITM 判断。

未采用上游 PR 中的硬编码 400/404 本地短路，也未强制开启 Tauri `custom-protocol`。仓库既有审查记录将这两项列为未验证的行为风险，不能随代理路由一起带入。

## 执行路径

```text
Cursor host 请求
  ├─ CONNECT                         → 原样交给 hudsucker 的 TLS 判断
  ├─ Tab + Direct                   → 原始上游路径
  ├─ Tab + Public/Custom            → 本地后端 → Tab 路由
  └─ 其他 Cursor host 请求           → 本地后端 → proxy::forward → 配置的出站代理
```

对应代码：

```text
server/src/local_app/proxy.rs       # 判断 CONNECT、Tab 模式和 Cursor host 路由
apps/desktop/src-tauri/Cargo.toml   # 保持 fork 原有 Tauri feature，不引入 custom-protocol
```

## 验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --locked -p cursor-server local_app::proxy::tests`：4 项通过，覆盖非 Tab fallback、Tab Direct/代理模式、CONNECT 保护和既有本地路径表。
- `cargo clippy --locked -p cursor-server --lib -- -D warnings`：通过。
- `npm --prefix apps/desktop run check`：TypeScript、Node 类型检查和 Vite 构建通过；仅有既有 chunk/dynamic import 警告。
- `cargo check --locked -p cursor-byok-desktop`：通过。
- 最终代码的全工作区回归：`cargo test --locked --workspace --all-targets` 通过（服务端 288 项单元测试、14 项忽略，所有集成测试通过）。

## 边界

本轮没有在仅允许代理出网的真实网络中启动 Cursor 并测量 75 秒超时下降，也没有验证 Tab 三种模式的真实 Cursor UI。PR 只提交经过本地协议测试证明的路由子集，真实网络和客户端验收仍需单独记录。
