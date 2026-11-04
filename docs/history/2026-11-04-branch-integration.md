# 2026-11-04：分支合并与验证

按维护者要求推送并合入现有 PR。原有已提交分支均已推送；#4 / #6 为解决主线文档迁移冲突新增了合并和文档修正提交，并补齐详细日志开启时 OAuth 重试的请求记录冲突修复。

| PR | 验证 head | main 合并提交 |
| --- | --- | --- |
| [PR #8](https://github.com/haohaomin/cursor-byok/pull/8) | `6b0f35a18eec07d9c18000c5e49f2f8944911942` | `07f99518acba1f1918e93539b6a04fe5a7fb7c54` |
| [PR #4](https://github.com/haohaomin/cursor-byok/pull/4) | `411f3214b0a65e99a89b2fd9fb7cf9e5ea77ed1b` | `e24eddf9cb3a19bc049d1d581e5ca9f49150b5bf` |
| [PR #6](https://github.com/haohaomin/cursor-byok/pull/6) | `5ac290d3adb646b73163cba1b44c18799a9b677f` | `91db8269381148b4b450ba78bd13f1ad175be380` |

## CI

以 GitHub 本次查询的固定 head 作业为准；完整记录：

- PR #8 · [Rust (Linux)](https://github.com/haohaomin/cursor-byok/actions/runs/41562364845/job/125675700280)：通过。
- PR #8 · [Frontend](https://github.com/haohaomin/cursor-byok/actions/runs/41562364845/job/125675700360)：通过。
- PR #8 · [Desktop Rust (windows-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41562364845/job/125675700039)：通过。
- PR #8 · [Desktop Rust (macos-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41562364845/job/125675700346)：通过。
- PR #4 · [Rust (Linux)](https://github.com/haohaomin/cursor-byok/actions/runs/41761473050/job/126349916285)：通过。
- PR #4 · [Frontend](https://github.com/haohaomin/cursor-byok/actions/runs/41761473050/job/126349916514)：通过。
- PR #4 · [Desktop Rust (windows-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41761473050/job/126349916509)：通过。
- PR #4 · [Desktop Rust (macos-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41761473050/job/126349916634)：通过。
- PR #6 · [Rust (Linux)](https://github.com/haohaomin/cursor-byok/actions/runs/41761492123/job/126349973031)：通过。
- PR #6 · [Frontend](https://github.com/haohaomin/cursor-byok/actions/runs/41761492123/job/126349973232)：通过。
- PR #6 · [Desktop Rust (windows-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41761492123/job/126349973113)：通过。
- PR #6 · [Desktop Rust (macos-latest)](https://github.com/haohaomin/cursor-byok/actions/runs/41761492123/job/126349972870)：通过。

从以上已完成作业日志提取的计数（各行独立执行，不相加）：

| 验证 | PR #4 | PR #6 |
| --- | --- | --- |
| Linux 全工作区 | 522 通过，0 失败，10 忽略 | 522 通过，0 失败，14 忽略 |
| Linux / macOS 插件生命周期，含真实 Deno 进程 | 各 37 通过，0 忽略 | 各 41 通过，0 忽略 |
| Windows 插件生命周期 | 38 通过，0 忽略 | 42 通过，0 忽略 |
| Windows / macOS 原生采集 / 来源选择 / SQLite | 各 14 / 10 / 6 通过 | 各 14 / 10 / 6 通过 |
| Grok / Codex / Google 插件脚本 | 23 / 12 / 5 通过 | 23 / 15 / 16 通过 |

全工作区中忽略的 Deno 项由插件生命周期命令单独执行；真实嵌入模型测试仍为忽略项，本轮没有重新运行。所有 OAuth 网络和账号数据均为合成数据。

PR #4 合并后的文件树与已验证 head 相同。PR #6 改为 main 后，预计算合并树与已验证 head 相同（`9feb0c0345190c882b9fff38840bc4774b7dab88`），最终合并后再次比较无差异。

## 合并处理

- #4：保留主线的新文档目录与索引；迁移 Grok 验证记录和失败探针，保留上游贡献者署名与固定 SHA。
- #4 合并前补测：详细日志开启时，首次 401 后的新 Worker 调用曾因重复插入同一请求主键而失败；现在为认证重试建立独立日志记录，保留首次失败与后续响应。另修正迁移后的复现脚本导入路径，并断言凭据最终确实写入最新值。
- #6：同步 #4；迁移 GPT/Google 验证记录；#4 合入后将 base 改为 main，再完成合并。
- 合并后同时保留 Semble 原生采集测试、Deno 脚本契约检查和真实 Deno 进程的插件生命周期测试。
- 本机 10 个 Semble 审批候选代码/测试文件的摘要与合并前一致，未提交、未推送；远端合入的是已提交采集版本。

## 保留边界

- 真实 OAuth 账号、Semble 批准/拒绝和 Windows Cursor UI 未验收；不关闭相关上游 Issue。
- 已知 Cursor 审批曾自动取消，独立终端卡片候选仍在本地。
- 插件 API 2 不兼容 API 1 的外部插件，需适配新协议。
- #155 保持完成一半，按用户要求不测 20 分钟。
- 本轮没有修改正式账号、生产设置或发布新安装包。
