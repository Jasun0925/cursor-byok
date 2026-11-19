# Issue #150 / #417：按模型设置 Token 定价

2026-11-13，完成 #150 与 #417 中“按模型设置自定义 Token 价格”的可交付实现，并在 #107 的按模型用量明细上接入逐模型估算。全局价格仍作为默认值；只有保存了模型哈希覆盖的模型使用单独价格。插件模型暂不提供独立覆盖，继续使用全局价格。

## 结构

```text
server/src/store/settings.rs                    # 校验、排序并持久化模型价格覆盖
server/src/control/settings.rs                  # GET/PUT 控制面路由
apps/desktop/src/shared/api.ts                  # API 类型和请求
apps/desktop/src/shared/store/appStore.ts       # 全局价格与覆盖的快照/刷新
apps/desktop/src/features/settings/
└── PricingSettingsCard.tsx                     # 全局价格和已配置模型的编辑界面
apps/desktop/src/shared/utils/tokenPricing.ts   # 逐模型价格选择与费用聚合
apps/desktop/src/features/home/
├── HomePage.tsx                                # 按筛选后的 model_usage 汇总费用
└── metrics/
    ├── HomeMetrics.tsx                         # 首页估算卡片
    └── ModelUsageDialog.tsx                    # 明细按模型应用有效价格
```

```text
设置页输入
    ↓
PUT /api/settings/model-pricing
    ↓
service_settings（model_token_pricing）
    ↓
appStore.modelPricing
    ↓
model_hash → 覆盖价格，否则使用全局价格
    ↓
首页总计 / 按模型明细
```

价格覆盖以 `model_hash` 作为身份，不以显示名称或模型 ID 作为身份。服务端会去除首尾空白、拒绝空哈希、重复哈希、负数和非有限数，并按哈希排序后保存。没有覆盖的模型保留全局价格；未配置模型的历史用量仍可在明细中显示。

本次没有新增数据库表、列或迁移，复用现有 `service_settings` 键值存储。Demo API 同步补齐启动所需的全局定价、提交设置和插件运行时空实现，使 Demo 能完整加载设置页并覆盖真实 React 路径。

## 验证

| 检查 | 结果 |
| --- | --- |
| 模型价格存储单元测试 | 9 项通过；覆盖默认值、持久化排序、空/重复/负数/NaN 校验 |
| 控制 API 集成测试 | 1 项通过；GET 空值、PUT 排序保存、GET 回读、重复哈希 400 |
| 工作区 Rust 全量测试 | 578 通过、0 失败、14 忽略 |
| 服务端 Clippy | `-D warnings` 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `npm run i18n:scan` | 通过，四个语言目录完整 |
| `npm run check` | 类型检查和生产构建通过；保留既有动态导入和大 chunk 警告 |
| Tauri debug/no-bundle 构建 | 通过，产物为 `target/debug/cursor-byok-desktop` |
| React + Chromium Demo | 设置页 16 个模型可编辑；修改 GPT-5.2 输入价并保存后，主页总价和模型明细立即按覆盖价计算；无运行时错误 |
| `git diff --check` | 通过 |

浏览器验证使用仓库 Demo 的受控数据和真实 React 组件，没有访问真实供应商，也没有证明插件模型已经支持独立价格。实现已通过 [PR #39](https://github.com/haohaomin/cursor-byok/pull/39) 合入当前 `main`，尚未随新版本发布；GitHub Actions 在仓库侧不可用，本地联合回归、Clippy 和桌面检查已在合并前完成。

结构化证据见 [JSON](2026-11-13-issue150-model-pricing-evidence.json)。
