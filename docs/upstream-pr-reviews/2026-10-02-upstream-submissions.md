# 三项修复的上游提交记录

2026-10-02，用户授权将个人 fork 中补齐并验证的修复提交给上游。核对上游 main 仍为 `9a8fde2`，原 #424、#423、#421 方案不作为完整验证结论；各修复独立提交，不带入 fork 定制和发布配置。

| 原 PR | 本地补齐的问题 | 我们提交的上游 PR | 当前状态 |
| --- | --- | --- | --- |
| #424 | Steer 后保留 Task，并完整交付迟到结果和 Cursor 完成状态 | [#487](https://github.com/leookun/cursor-byok/pull/487) | 已有 PR，OPEN，可合并，无需重复提交 |
| #423 | 拒绝工具调用时终止服务端重试，并向 Cursor 标记不可重试 | [#500](https://github.com/leookun/cursor-byok/pull/500) | 本次新建，OPEN，非草稿，可合并 |
| #421 | 保留各输出项累计文本，防止交错、迟到完成事件及首次编号出现时重复输出 | [#501](https://github.com/leookun/cursor-byok/pull/501) | 本次新建，OPEN，非草稿，可合并 |

“可合并”来自 GitHub mergeable=MERGEABLE，只表示没有合并冲突，不表示维护者已批准或合入。

## 独立分支及验证

- #500：`haohaomin:codex/upstream-anthropic-refusal`，提交 `24d9850dd4292dbc8412ad170034850568b94e00`。从上游 main 单独提取 7 个生产文件和 1 个集成测试文件；服务端全量 267 项测试通过。
- #501：`haohaomin:codex/upstream-responses-text`，提交 `e1e784df4220836d7659c251794582a1d6a50cff`。从上游 main 单独提取 1 个生产文件和 1 个集成测试文件；服务端全量 264 项测试通过，其中 Responses 专项 9 项全部通过。
- 两分支均通过 `cargo fmt --all -- --check`、`cargo clippy -p cursor-server --all-targets -- -D warnings`、`git diff --check`。
- 两份 PR 均注明 kevin9327 的原方案来源，提交保留 Co-authored-by 署名；明确描述原方案未覆盖的场景及验证边界。
- 两份 PR 互不依赖，也不依赖 #487；未带入版本、签名、更新源、广告定制、本地文档及截图。
- 独立 Skipped 展示修复不在这两份 PR 中，避免混入其他问题。

## 上游 CI 状态

两份新 PR 的 GitHub Actions 均返回 `action_required`，未获得测试执行结果，需要上游维护者处理运行权限；不能将本地通过描述为上游 CI 已通过。

- [#500 CI](https://github.com/leookun/cursor-byok/actions/runs/36890765895)
- [#501 CI](https://github.com/leookun/cursor-byok/actions/runs/36891031228)

本次仅提交 PR，没有合并上游分支，也没有关闭或修改他人的原 PR。
