# Issue #107：按模型查看用量统计

2026-11-12，完成 #107 中“用量统计增加模型分类”的第一阶段：概览页的四个统计卡片可以展开，按当前时间范围和模型筛选查看每个模型的调用、Token、缓存和估算价值。实现先覆盖本地 BYOK 历史调用；插件模型沿用已有稳定模型 ID，未改插件生命周期。

最终复核提交 `0949559` 还修正了四个边界：范围请求未就绪或失败时清除旧数据，旧服务缺少 `model_usage` 时回退为空数组，分组总计与明细在同一 SQLite 事务快照中读取，以及同一模型哈希使用最新显示名称。该提交的本地定向 Store 测试、Clippy 和桌面检查均通过；远端 CI 以该提交为准。

上游 [#107](https://github.com/leookun/cursor-byok/issues/107) 还询问 Git 自动提交使用的模型是否走自定义 URL。本次只核对代码路径：`server/src/cursor/services/commit_message.rs` 读取提交设置，配置了模型时走本地模型调用，未配置模型时保持直连转发；这不是本次统计 UI 的变更，也没有将其误记为真实 Cursor Git UI 验收。

## 结构

```text
server/src/store/overview.rs              # 一次分组查询，生成总计与模型小计
server/src/model/observability.rs         # Overview / ModelUsage API 数据结构
server/src/control/overview.rs            # 复用现有时间和模型筛选边界
apps/desktop/src/shared/api.ts            # TypeScript API 类型
apps/desktop/src/features/home/
├── HomePage.tsx                          # 打开明细并传递当前筛选
└── metrics/
    ├── HomeMetrics.tsx                   # 四个统计卡片的键盘可达入口
    └── ModelUsageDialog.tsx               # 虚拟列表中的模型小计
```

现有 `server/src/store/overview.rs` 约 260 行，`HomePage.tsx` 约 210 行；新增 `ModelUsageDialog.tsx` 约 70 行、对应 SCSS 约 25 行，行数包含空行和注释，不含生成代码与测试。没有新增表、列或迁移。

```text
时间范围 + model_hashes
        ↓
GET /api/overview
        ↓
llm_calls 按 model_hash / model_id / provider_type / provider_url 分组
        ├── model_usage[] → 明细弹窗
        └── metrics        → 首页总计
```

调用数沿用现有口径：排除 `running`；成功为 `completed`；其他已结束状态计为异常。Token 沿用现有口径：包括已记录用量的所有状态；Anthropic 的输入扣除缓存字段，其他协议使用非缓存输入表达式。总计由相同分组结果累加，避免总计和明细采用不同过滤条件。空 `model_hash`、插件稳定 ID 和已从当前配置删除的历史模型均可显示。

## 测试结果

| 检查 | 结果 |
| --- | --- |
| Store 定向回归 | 4 项通过：缓存/进行中状态、半开时间范围和模型筛选、历史模型身份、空用量记录 |
| `cargo test --workspace` | 通过；303 个 server 单元测试及全部 integration/doc tests 通过，14 项 ignored，无失败 |
| `npm run i18n:scan` | 通过；新增文案进入四语言目录 |
| `npm run check` | 通过；保留既有动态导入和大 chunk 警告 |
| React + Chromium | 统计卡片、16 个模型明细、明细滚动、Esc 关闭、焦点返回原卡片通过 |
| `git diff --check` | 通过 |

浏览器使用真实 React 组件和受控管理 API，不访问真实供应商；Tauri debug/no-bundle 构建已通过，但这证明桌面工程可编译，不等同于已安装应用或真实 Cursor 验收。PR CI 仍需对最终提交独立执行。

结构化证据见 [JSON](2026-11-12-issue107-model-usage-breakdown-evidence.json)。
