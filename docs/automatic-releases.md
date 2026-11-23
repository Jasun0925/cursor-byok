# 自动发布桌面版本

主分支代码通过 CI 后自动发布稳定版。默认递增补丁版本，例如 `1.1.0 → 1.1.1`；如需 minor/major 版本，先在五处版本文件统一设置更高的稳定版本号。

```text
.github/
├── workflows/ci.yml                  # Rust、前端、平台测试和发布脚本测试
├── workflows/release.yml             # 接收成功 CI，准备版本、构建、签名、发布
└── scripts/
    ├── prepare-auto-release.mjs      # 改动检测、五处版本同步、原子推送提交和标签
    └── prepare-auto-release.test.mjs # 本地 Git 仓库模拟并发、重复执行和失败
```

```text
代码推送/合并到 main
  → CI 全部成功
  → 确认该次 CI 的提交仍是 main 最新提交
  → 对比上次稳定标签；有应用/构建改动才发版
  → 同步 Cargo/npm 锁文件、package.json、Cargo.toml、tauri.conf.json
  → 原子推送版本提交和 v* 标签
  → 四个平台从该版本提交构建并签名
  → 生成 latest.json / portable-latest.json / update.json 和发布说明
  → 全部成功后公开为普通 Latest Release
```

- `docs/`、`.agents/`、仓库说明与维护文档修改会跳过发版；应用内 Markdown 提示词、代码删除、依赖和工作流改动仍会发版。
- PR 检查、功能分支检查、失败或取消的 CI 不发布。旧 CI 结果不会发布未测试的新代码。
- 发布流程串行执行，正在构建的版本不会因后续提交被取消。版本准备时若 main 已变化，则交给新提交的 CI。
- 版本提交使用仓库内置 `GITHUB_TOKEN`，不会再次触发 CI，也不依赖新标签事件触发另一次工作流，因此无需额外个人令牌。
- 已存在的标签永不覆盖。构建失败保留旧 Latest；检查 Actions 中的失败原因后再处理原运行，不移动标签。
- 自动发布安装包不等于自动替换本机应用。客户端仍按现有更新流程检测、下载和安装。

普通发版只需合并代码到 main，不需要手动改版本号或打标签。查看 GitHub Actions 中同一提交对应的 `CI` 与随后的 `Release desktop app`；只有发布任务成功且 Release 已公开，才表示新安装包可用。

脚本测试：`node --test .github/scripts/*.test.mjs`。触发机制参见 [GitHub workflow_run](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run) 和 [GITHUB_TOKEN 的递归触发限制](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow#triggering-a-workflow-from-a-workflow)。

## 提交后完全没有 CI 运行

先查看仓库的 Actions 网页。GitHub 可能因 fork 的 Actions 使用规模暂停整个仓库，此时 REST API 的 `enabled: true` 和单个 workflow 的 `state: active` 仍不足以证明可以运行。

如果页面显示 `Workflows on this fork have been disabled` 或 `Workflows are not currently being run on this fork due to the scale of GitHub Actions usage`，由仓库维护者核对工作流后点击 `I understand my workflows, go ahead and re-enable them`。页面确认 Actions 已启用后，再推送新的 main 提交启动 CI；暂停期间的旧 push 不会自动补跑。

2026-11-23：在启用自动发布时确认并恢复了这一仓库级暂停。恢复后仍需以实际 CI 和发布运行结果为准，不能只看设置页开关。
