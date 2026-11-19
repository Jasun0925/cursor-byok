# Issue #99：发现模型后批量选择添加

2026-11-13，在当前 fork 架构中完成 #99 的批量模型部分。设置页原先只能把 `/models` 返回结果放进单个模型下拉框；现在可以在发现多个模型后多选，并一次提交到已有批量创建接口。Issue 中的 Token 用量统计部分已由 #107/#150 的按模型统计和逐模型定价覆盖，本次没有重复实现。

上游 [PR #272](https://github.com/leookun/cursor-byok/pull/272) 已检查，但它只是旧架构的发布提交，没有可移植的批量模型实现，因此没有直接采用。

## 结构

```text
apps/desktop/src/features/models/
├── CursorSettingsPage.tsx       # 保存发现结果，构造批量 ModelInput 并提交
├── CursorModelEditor.tsx        # 多选发现结果，触发批量添加
└── CursorSettings.module.scss   # 多选区域布局
apps/desktop/src/shared/ui/
└── ModelSelect.tsx              # 复用已有多选、分组全选和清空控件
server/src/control/
├── models.rs                    # 既有 POST /__byok-api__/api/models 入口
└── service.rs                   # 既有批量 create_models 服务
server/tests/external_api.rs     # 批量创建控制 API 回归
```

主要路径：

```text
服务器地址 + API Key
        ↓
GET /__byok-api__/api/models/discover
        ↓
CursorModelEditor 的 ModelSelect（默认全选）
        ↓
CursorSettingsPage 按模型 ID 生成 ModelInput[]
        ↓
POST /__byok-api__/api/models
        ↓
SQLite 事务一次写入全部模型并刷新列表
```

`display_name` 和 `tooltip_data` 留空时使用模型 ID，URL、API Key、协议、请求头和 Token 限制沿用当前编辑表单。批量写入继续使用现有事务语义：任一配置无效或与同批模型重复时整批失败并显示服务端错误，不会留下半批结果。编辑已有模型时仍显示单模型表单，避免把批量发现动作误用于更新。

现有代码行数（包含空行和注释，不含生成目录）：`CursorModelEditor.tsx` 260 行，`CursorSettingsPage.tsx` 453 行，相关 SCSS 384 行；新增测试位于已有 `server/tests/external_api.rs`，没有新增数据库表、列或迁移。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过；macOS 与 Oracle Linux 9.8 ARM64 均通过 |
| 控制 API 批量创建回归 | 通过；macOS 与 Oracle Linux 9.8 ARM64 均为 `model_create_accepts_a_batch_of_discovered_models` 1 项通过、0 失败 |
| `cargo clippy --locked -p cursor-server --test external_api -- -D warnings` | 通过；Oracle Linux 9.8 ARM64 通过 |
| `npm run i18n:scan` | 通过；新增文案已填充 en-US、pt-BR、ru-RU、zh-CN |
| `npm run check` | 通过；TypeScript、Node 类型检查和生产构建通过，保留既有动态导入/大 chunk 警告 |
| `git diff --check` | 通过 |

本次没有真实供应商账号，因此没有验证不同服务商的实际 `/models` 返回、重复模型的操作提示或已安装 Tauri 应用中的鼠标流程。当前 PR 没有 GitHub Actions 检查，以上 macOS 与 Oracle Linux 证据来自本地和隔离远程运行；这些边界不计为手工验收通过。
