# 历史发布与资产核验

记录按原始发布日期保存；保留提交、测试、资产摘要及“未监视”的限制。当前可用范围见 [维护状态](../maintenance-status.md)。

<a id="v104"></a>

## v1.0.4 发布验证记录

来源日期：2026-09-29。以下为该次发布记录，不代表当前 Latest 状态。

- 仓库：haohaomin/cursor-byok（原 renhao12356578/cursor-byok 重定向至相同仓库 ID 1384772977）。
- 修复提交：51ee8e7；主分支发布提交：ec5a47af257e200c6c0149731d304ee292360111。
- 标签：v1.0.4，指向已通过 CI 的 origin/main 提交。
- Release：https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.4
- CI：https://github.com/haohaomin/cursor-byok/actions/runs/36567521769（全部成功）。
- 发布：https://github.com/haohaomin/cursor-byok/actions/runs/36568086803（四个平台及 finalize 全部成功）。
- GitHub Latest=v1.0.4；draft=false；prerelease=false。
- npm check、workspace all-targets Clippy、Tauri debug no-bundle 构建、工作流 YAML 解析、portable 更新清单测试通过。
- latest.json、portable-latest.json、update.json 均为 1.0.4；下载 URL 指向当前 fork，资产存在。
- 原安装版本的更新地址经重定向最终返回 HTTP 200。
- 使用现有 updater 公钥，实际下载并验证下列 7 个包的完整 Minisign 签名，全部通过。
- 本次未安装新版本到本机，不把清单及签名校验描述为应用内自动安装验证。

| 产物 | SHA-256 | 签名 |
| --- | --- | --- |
| Cursor.BYOK_1.0.4_aarch64.app.tar.gz | `a4c2e0bfc85066d8db6e2e43abeffbd0a317263473786e99a4e60105ab8be790` | valid |
| Cursor.BYOK_1.0.4_amd64.AppImage | `748149fc77b5b857101b1f7f3d0042566dad4da980b441408e9e91d0005c38c9` | valid |
| Cursor.BYOK_1.0.4_amd64.deb | `ff9a1d2e5464d971e8f8150f4104c7a8d9879b40170ecd5b7dcc2a5b4eb9f8a5` | valid |
| Cursor.BYOK-1.0.4-1.x86_64.rpm | `400e907578f72df1ccc6c2d1de982936dca1a2c89f4b67a271e05e0d6a388da2` | valid |
| Cursor.BYOK_1.0.4_x64-setup.exe | `f090fb50fefc1e4c59146f2ffc39e5540bc5b3d465a56eeb4767c08fd5768f13` | valid |
| Cursor.BYOK_1.0.4_x64.app.tar.gz | `5d8b9d28faeaaf6185e746bf0bb232d4c16af7f220b370ff31273166c36e740d` | valid |
| cursor-byok-1.0.4-windows-amd64.zip | `94642d6f78b54fc302a686b1f6f8da7419454b811beafbbff5705fe6583c41d1` | valid |


<a id="v105"></a>

## v1.0.5 发布验证记录

来源日期：2026-09-30。以下为该次发布记录，不代表当前 Latest 状态。

- 仓库：haohaomin/cursor-byok（旧地址 renhao12356578/cursor-byok 重定向到同一仓库，ID 1384772977）。
- 发布账号：haohaomin，与当前仓库所有者一致，具有 ADMIN / push 权限。
- 修复提交：89d90c68e45fa2c04fa6812082139e8b47837e08。
- 发布提交：c46fb1e2ea70b47e66e2ffa5754ca7fe07c07b03。
- 标签：v1.0.5，指向已推送到 origin/main 的发布提交。
- 发布流水线：https://github.com/haohaomin/cursor-byok/actions/runs/36699430053
- Release：https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.5
- 本地前端 check、Tauri debug no-bundle 构建、工作流 YAML 和 tag-only 触发检查通过。
- 五个版本清单/锁文件保持 1.0.5；cursor-server 版本未改。
- 沿用原 updater 公钥和 GitHub Actions 私钥 Secret，未改签名、证书或更新地址。
- #423 修复的 270 项服务端测试及工作区 Clippy 已通过，三个真实 Cursor UI 场景已通过（供应商端为本地模拟 SSE）。
- 状态：四个平台及 finalize 全部成功；GitHub Latest=v1.0.5，draft=false，prerelease=false。
- latest.json、portable-latest.json、update.json 均为 1.0.5，下载 URL 指向当前 fork，资产存在。
- 实际下载 10 个包：7 个 Minisign 签名通过，4 个旧版平台包的大小和 SHA-256 均与 update.json 一致（Windows 包同时属于两类）。
- 原安装版本的旧仓库更新地址可经重定向获取 v1.0.5 清单。

本次发布包含 #423 拒绝处理修复及已同步的上游本地应用外部 API；没有安装到本机，不把下载和签名校验描述为应用内自动安装验证。

| 产物 | SHA-256 | 签名 |
| --- | --- | --- |
| Cursor.BYOK-1.0.5-1.x86_64.rpm | `ce159372e39bdc08c7e361ea1a36ff1f6f98a45eaef9b1ff262483c7424c3bc1` | valid |
| Cursor.BYOK_1.0.5_aarch64.app.tar.gz | `6b58971e3d6b879bc3ab9a58a7b48746b331b6fcbe1f93ebc177682e82b80f79` | valid |
| Cursor.BYOK_1.0.5_amd64.AppImage | `d483bb45d9906f81955edc80567c2fe945aa5c694428a06763c4b73e625809c5` | valid |
| Cursor.BYOK_1.0.5_amd64.deb | `0450a789f735112c1db0f68a536cfda62a8a3d2e6c61db76f3454b2cd2146853` | valid |
| Cursor.BYOK_1.0.5_x64-setup.exe | `83a4fc7500095c67d66d9cf1b7c2ecb8b9bade41e13d5f795aa1897279c087b6` | valid |
| Cursor.BYOK_1.0.5_x64.app.tar.gz | `1a7fc8c327f699876b0e1097c5b6fae881b73704f194a749bd8eef5bbdc3f7b4` | valid |
| cursor-byok-1.0.5-linux-amd64.tar.gz | `5565c0c1f5f86959c7964c3cf9ffec634a26981671400e9a37ef2caf0ee8866e` | 旧版清单 SHA-256 校验通过 |
| cursor-byok-1.0.5-macos-amd64.tar.gz | `1a7fc8c327f699876b0e1097c5b6fae881b73704f194a749bd8eef5bbdc3f7b4` | 旧版清单 SHA-256 校验通过 |
| cursor-byok-1.0.5-macos-arm64.tar.gz | `6b58971e3d6b879bc3ab9a58a7b48746b331b6fcbe1f93ebc177682e82b80f79` | 旧版清单 SHA-256 校验通过 |
| cursor-byok-1.0.5-windows-amd64.zip | `615e472716cdb10323f9994ebd2f56233ab8b0c212e37510a8e0dd0954ac82dd` | valid |

结构化证据：[2026-09-30-v1.0.5-release-evidence.json](pull-requests/2026-09-30-v1.0.5-release-evidence.json)。


<a id="v106"></a>

## v1.0.6 发布记录

来源日期：2026-10-01。以下为该次发布记录，不代表当前 Latest 状态。

### 发布范围

- 发布仓库：`haohaomin/cursor-byok`；原 origin 地址重定向至该仓库。当前 GitHub 身份为仓库所有者，具有 ADMIN 权限。
- #421：Responses 按输出项保留累计文本，补齐缺失后缀，避免交错、迟到完成事件和首次编号出现时重复文字；合并提交 `e08cab5`。
- 工具卡片 Skipped：工具完成事件发送后再释放后续文字和思考，保持 checkpoint 展示顺序；独立修复 `2616014`。
- 两项修复均已完成真实 Cursor 验证，临时测试服务已关闭、正式配置已恢复。
- 桌面版本统一为 `1.0.6`，服务端独立版本保持 `0.1.0`。原 updater 公钥未改变，签名 Secret 已确认存在。

### 本地验证

- 服务端全量：287 项通过，无失败或忽略。
- `npm run check`：通过；现有前端大 chunk 提示不阻断构建。
- `npm run tauri:build -- --debug --no-bundle`：通过。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- GitHub 工作流 YAML 解析：通过。
- Windows portable 更新清单测试：2 项通过。
- 本机使用 MacOSX26.5 SDK 与 Rust 自带 ld64.lld。

### 发布状态

已发布，四个平台和 finalize 全部成功。

- 发布提交及标签指向：`dcd765842e2df85a2b1713c69fc332c32c767303`，已存在于 `origin/main`。
- [CI](https://github.com/haohaomin/cursor-byok/actions/runs/36881249466)：四项检查全部成功。
- [发布工作流](https://github.com/haohaomin/cursor-byok/actions/runs/36881298760)：全部成功。
- [Release v1.0.6](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.6)：Latest，draft=false，prerelease=false。
- 20 个附件已实际下载，文件大小及 GitHub 提供的 SHA-256 摘要均匹配。
- 使用项目现有的 `minisign-verify 0.2.5` 和原 updater 公钥验证 7 个更新包的完整签名，全部成功；未接触私钥。
- `latest.json`、`portable-latest.json`、`update.json` 版本均为 1.0.6，URL 指向当前 fork 的实际附件；签名、大小及 SHA-256 均已核对。
- 旧 fork 地址的三个 Latest 更新入口经重定向均返回 HTTP 200、版本 1.0.6。
- 本次用户授权涵盖合并并发布，已在 Release 中补入两项修复的说明。

本次发布不代表已替换本机安装的应用，也不表示上游 PR #421 已合并。

### Signature verification

| Asset | SHA-256 | Signature |
| --- | --- | --- |
| cursor-byok-1.0.6-windows-amd64.zip | `eed520db2aa308bbadc59ab6af853c454147dd70f8b099a1e9a54721e40adcc2` | valid |
| Cursor.BYOK-1.0.6-1.x86_64.rpm | `7d915e96244c2e5e57954cd6644517214cff4eee2ecd6ef7d93f1fb8f6f82e7e` | valid |
| Cursor.BYOK_1.0.6_aarch64.app.tar.gz | `94e7943e2266435b45b9b0a3c7f3e4c04d41f164301b51e0a50db3081e315704` | valid |
| Cursor.BYOK_1.0.6_amd64.AppImage | `8a7db65a31907c523bf18e615ef40c0724db2f0e2a9898e5f29c37ddba379ea8` | valid |
| Cursor.BYOK_1.0.6_amd64.deb | `40219139dff2a45b58a580c97ad804117bad5b35e308ec546a1bb9fa68091ff9` | valid |
| Cursor.BYOK_1.0.6_x64-setup.exe | `6a167f7385a3abbabdd57d7e3cf6f94665b5c1603d12c146eca957b001a6ef8b` | valid |
| Cursor.BYOK_1.0.6_x64.app.tar.gz | `d2e258fb10ff33e2c8a457b8581b83db014edd1ee225b51ac10812c3e305e9d9` | valid |


<a id="v107"></a>

## v1.0.7 发布记录

来源日期：2026-10-02。以下为该次发布记录，不代表当前 Latest 状态。

### 内容与授权

用户明确要求发布新安装包。发布到个人 fork `haohaomin/cursor-byok`；当前 actor 为仓库所有者，具有 ADMIN 权限。

- 修复：`7037431`，补齐 `response.completed.response.output` 中尚未显示的文字，保持按输出项补全、防重复及工具调用行为。
- 桌面版本由 1.0.6 升至 1.0.7，独立 cursor-server 版本仍为 0.1.0。
- updater 公钥与本地公钥一致且未变；GitHub 签名 Secret 存在，未读取私钥。
- 原版本 v1.0.6 不包含此次修复。

### 已有修复验证

- 服务端全量测试：294 项通过。
- 服务端 all-targets Clippy（-D warnings）、格式检查及差异检查通过。
- [修复提交 CI](https://github.com/haohaomin/cursor-byok/actions/runs/36991008954)：成功。
- 真实 Cursor + 本地可控 Responses SSE：完整文字、后缀补全、防重复、工具审阅四项通过；工具卡片 Ran 且详情可展开；实际 Shell 只执行一次，标记文件一行。不是原作者的 sub2api 环境复现。
- 复测后已关闭隔离服务，恢复原代理、4 个模型及逐字节一致的 Cursor 设置。

### 发布验证

准备阶段：前端检查、桌面 debug 构建（不打包）、工作流 YAML 解析、portable 清单测试、Rust 格式检查及差异检查全部通过。桌面构建使用本机 macOS 26.5 SDK 与 Rust lld 链接器。

本次发布不自动替换本机已安装应用。

### 发布结果

- 发布提交：`7dfac4bfbaaa1fef973e3ef627ec2e4d1668f35a`，标签 `v1.0.7`。
- [发布流水线](https://github.com/haohaomin/cursor-byok/actions/runs/36992101997)：四个平台构建及最终发布全部成功。
- [发布提交 CI](https://github.com/haohaomin/cursor-byok/actions/runs/36992053137)：全部成功。
- [GitHub Release](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.7)：Latest，非草稿，非预发布。
- 发布后下载校验：20 个附件大小及 SHA-256 与 GitHub 元数据一致；7 个安装/更新包签名均通过现有更新公钥验证。
- `latest.json`、`portable-latest.json`、`update.json` 的版本、产物地址及对应签名或校验和全部通过。
- 旧仓库地址的三个 `/releases/latest/download/` 更新入口均返回 HTTP 200，版本为 1.0.7。
- 校验只读取临时下载文件，未安装或替换本机应用。未执行所有平台安装后的运行测试。


<a id="v108"></a>

## v1.0.8 发布准备记录

来源日期：2026-10-03。以下为该次发布记录，不代表当前 Latest 状态。

用户明确授权合并 #477 修复并发布到个人 fork `haohaomin/cursor-byok`，要求推送发布命令后停止，不监视构建、不下载安装包。

- 修复提交 `b49ae95`：接受独立的父任务请求头，保留关联并拒绝空值/冲突；为缺省消息 ID 的子代理按输入检查点位置建立稳定身份。
- 新增 8 项协议回归，服务端全量 302 项测试、Clippy 和格式检查通过。
- Mac 真实 Cursor 单子代理流程通过：主子均走 BYOK、6 次模型调用 HTTP 200、12 个工具成功、父代理继续总结。Windows 缺省 ID 尚无现场 UI 复测，边界见 Issue #477 修复记录。
- 桌面版本升至 1.0.8；cursor-server 保持独立的 0.1.0 版本。
- 既有更新公钥保持不变并匹配本地公钥；GitHub 签名 Secret 存在，未读取私钥。
- 通过现有 v* 标签工作流发布普通稳定版，成功后自动设为 Latest。

发布前 `npm run check`、`npm run tauri:build -- --debug --no-bundle` 均通过（本机 macOS 26.5 SDK + Rust lld）；工作流 YAML 解析、格式及差异检查通过。本记录不将“已触发发布”记为“安装包已发布成功”；按用户要求，本轮不查询发布完成状态。


<a id="v109"></a>

## v1.0.9 发布准备记录

来源日期：2026-10-04。以下为该次发布记录，不代表当前 Latest 状态。

用户明确授权将 Issue #340 修复发布到个人仓库 `haohaomin/cursor-byok`，要求触发后不监视、不下载安装包。

- 修复提交 `80a4a74`：后台完成通知接受可选的工具调用 ID，事件身份包含任务 ID，避免同一子代理不同轮次结果被合并。
- 服务端 305 项测试通过，Clippy、格式检查通过。
- 普通和 Fast 模式真实 Cursor 测试通过；同一父会话连续两轮后台 Task 完成后自动继续，6 个 Run 和 10 次模型调用均成功。两次通知均携带工具 ID，缺省 ID 分支由自动化回归覆盖。
- 正式环境已恢复，测试后端已停止。详见 Issue #340 修复和 UI 证据记录。
- 桌面版本与锁文件统一为 1.0.9，服务端独立版本保持 0.1.0。
- 发布账号 `haohaomin` 与仓库所有者一致，仓库权限 ADMIN；签名 Secret 存在，未配置密码 Secret；既有更新公钥匹配且保持不变。
- `npm run check`、`npm run tauri:build -- --debug --no-bundle` 通过（macOS 26.5 SDK + Rust lld）。工作流 YAML 解析与仅标签触发策略检查通过。
- 通过 main 中的匹配版本提交创建 v1.0.9 标签，标签推送触发既有跨平台发布工作流。

本记录是发布准备和验证证据，不表示安装包已生成。按用户要求，标签推送后不查询工作流或 Release 完成状态。


<a id="v1010"></a>

## v1.0.10 发布记录

来源日期：2026-10-04。以下为该次发布记录，不代表当前 Latest 状态。

发布目标：`haohaomin/cursor-byok`（origin 旧地址 `renhao12356578/cursor-byok` 重定向至此仓库）。当前 GitHub 身份 `haohaomin` 与仓库 owner 一致，有 ADMIN/push 权限。用户授权：合并并发布，不监视。

相对 v1.0.9 的范围：

- #441：分离 Kimi Code 与 Moonshot API 预设。
- #482：补全 MCP 对象工具参数 schema。
- #310：模型级推理强度“不传入”。
- #294：模型级“移除图片输入”，默认关闭；保留原始会话数据。
- #318 与 #290：现有协议行为的回归验证；#290 没有新增生产代码修复。

验证：桌面版本与 Cargo/npm 锁文件均为 1.0.10；`npm run check`、Tauri debug no-bundle 构建通过（本机使用 macOS 26.5 SDK 与 LLVM linker）。发布 workflow YAML、仅 v* 标签触发、签名公钥匹配及 Actions signing secret 名称已核对。验证过程未读取或输出私钥。

发布方式：先推送 main，再推送其对应的 v1.0.10 标签，由 GitHub Actions 构建平台安装包。按照用户要求，不监视 Actions，不下载安装包，不确认远端构建或最终发布结果。

原供应商环境尚未完成现场复测的 issue 继续保持未勾选，不因本次发版标记为完全解决。


<a id="v1011"></a>

## v1.0.11 发布记录

来源日期：2026-10-08。以下为该次发布记录，不代表当前 Latest 状态。

用户授权：发布且不监视，并准备真实 Cursor 测试环境。

发布到个人 fork `haohaomin/cursor-byok`，origin 旧路径重定向至该仓库；已核实 actor 与 owner 均为 haohaomin，具有 ADMIN/push 权限。

相对 v1.0.10：
- #199：MCP 工具顶层 required:null 规范化为空数组。
- #88：连通性测试提供非空系统指令。
- #86：正文 think 标签回归验证，没有新增生产逻辑。
- #449：删除 Task 写死模型名单，投影客户端子模型信息；保留 Cursor 模型设置优先级。

验证：服务端全量 338 项与 Clippy 通过，桌面 npm run check 与 Tauri debug no-bundle 构建通过。版本文件统一 1.0.11，签名公钥匹配，Actions signing secret 存在；未读取私钥。版本标签及 Release 发布前不存在。

先推送 main 并确认远端提交，再推送 v1.0.11 标签触发工作流。按用户指示不监视 Actions、不下载安装包、不宣称远端构建已完成。#449 真实 UI 验证仍待用户操作。


<a id="v1012"></a>

## v1.0.12 发布记录

来源日期：2026-10-08。以下为该次发布记录，不代表当前 Latest 状态。

用户授权：“ok，你修改，修改后发布”。沿用不监视发布、不下载安装包的要求。

目标为个人 fork `haohaomin/cursor-byok`（origin 旧 URL 重定向至此）。发布前已核实 actor/owner 均为 haohaomin，ADMIN/push 权限，origin/main 基线 6c3feb0；v1.0.12 标签和 Release 均不存在。

### 内容

- 给 Cursor 仅提供哈希 ID 的自定义子代理模型补齐本地显示名称，避免按名称指定时误报不可用。
- 新建子代理的模型优先级改为本次明确指定 → Cursor 子代理默认设置 → 父模型。
- 省略模型或 inherit 仍采用默认值；禁用子代理仍阻止调用。没有静默替换用户明确指定的模型。
- 工具说明和每轮模型上下文同步更新，历史按追加方式保存。

### 验证

服务端全量 344 项通过，Clippy all-targets（-D warnings）、格式与 diff 检查通过。桌面 npm run check、Tauri debug no-bundle 构建通过。五个版本文件一致为 1.0.12；workflow YAML、tag-only 触发、协议源文件、公钥匹配与 signing secret 名称均已核实，未读取私钥。

名称识别已有真实 Cursor 验证。新增优先级由实际发给 Cursor 的 SubagentArgs 回归验证，新优先级的用户手动真实执行仍待复测。见 [修复记录](issues/2026-10-08-issue449-fix.md) 和 [最终证据](issues/2026-10-08-issue449-priority-evidence.json)。

### 发布方式

先合入并推送 main，确认远端提交匹配，再推送 v1.0.12 标签触发工作流。按用户要求不监视 Actions，不下载远端安装包，不宣称远端构建已经完成。隔离测试后端同步更新至最终实现，保留测试会话和数据。


<a id="v1017"></a>

## v1.0.17 发布记录：#155 完成一半

来源日期：2026-10-19。以下为该次发布记录，不代表当前 Latest 状态。

用户已授权提交并发布已修复部分，#155 按“完成一半”记录，不关闭上游 Issue。

发布目标为 `haohaomin/cursor-byok`，当前 `origin`、GitHub 登录账号与仓库所有者一致，权限为 ADMIN。原 `renhao12356578/cursor-byok` 地址仍解析到同一仓库，应用现有更新入口与签名公钥保持不变。本次使用稳定版本 `v1.0.17`，GitHub Release 必须为普通 Latest Release。

### 本次范围

- 修复后台任务开关与 Bootstrap 配置身份，确保 Cursor 能加载后台通知配置。
- 进度和重复通知以 TurnEnded → EOF 确认；按单个任务去重，避免拆批、合批或重发重复唤醒。
- 保存检查点的 Cursor 状态，迟到完成通知保留已压缩历史；首次导入与较新历史继续保留。
- 新增数据库迁移 11。已有迁移未改写，回归覆盖升级前历史保留、外键约束和关闭后重开。

真实 Cursor 25 秒普通后台任务自动续答通过；完成通知四种重放去重通过。真实审批等待、压缩期间完成、最终补丁的输出通知及官方→BYOK 转入尚无完整实机覆盖。20 分钟测试按用户要求跳过。详见 [验收记录](issues/2026-10-19-issue155-acceptance.md)。

### 发布前检查

从 `origin/main=8d74b25f3b09432d6d79736b2f6bcb0555f9229c` 建立隔离发布工作树，包含 #155 修复、关联测试/证据和五个版本文件；原工作区其他改动没有混入。版本文件统一为 1.0.17，独立服务端包版本未变。

`npm run check`、`npm run tauri:build -- --debug --no-bundle`、Clippy、格式与差异检查通过。隔离发布目录完整 Rust 回归 424 项通过、0 失败、0 忽略。测试数与原工作区 426 项不同，因为没有带入其他 Issue 的本地测试改动。

首轮默认环境及单独复核均触发既有 `request_transport_failure_is_one_failed_attempt` 断言；同一已编译测试程序为本地地址设置 NO_PROXY 后立即通过（0.03 秒）。因此完整回归只为测试进程设置 `NO_PROXY=127.0.0.1,localhost,::1`；系统代理和产品源码没有为此改动。

发布工作流验证为仅 `v*` 标签触发，使用 `tauri-action@v1` 的 `uploadUpdaterJson: true`，`prerelease: false`，最终设为 Latest。Proto 构建输入已跟踪。现有公钥与本地公钥匹配；Actions 存在签名密钥 Secret，无密码 Secret。没有读取或上传签名私钥。

本地构建没有替换已安装应用，也没有升级正式数据库。

### 发布执行

代码提交 `2df03e3108f4ca15b835a70ba04c03ca1cb2cdde` 已推送到 `main`，标签 `v1.0.17` 指向同一提交。[发布流水线](https://github.com/haohaomin/cursor-byok/actions/runs/39151624186) 已启动；主分支推送未触发发布，标签推送是唯一发布触发。四个平台构建和最终发布均已成功；北京时间 `2026-10-19T02:13:46+08:00` 发布为普通 Latest Release。

### 发布核验完成

[Cursor BYOK v1.0.17](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.17) 已公开发布，draft=false、prerelease=false，并且是仓库 Latest。共有 20 个资产；`latest.json`、`portable-latest.json`、`update.json` 的版本均为 1.0.17，所有下载地址指向当前仓库和标签。

已下载并核验三份清单引用的全部不同资产：Tauri 各平台及 Windows portable 的签名均使用现有应用公钥通过 Ed25519/Minisign 验证，legacy 四平台归档大小与 SHA256 全部符合清单。资产下载同时核对 GitHub 提供的摘要。发布说明已写明 #155“完成一半”及未验收边界。

没有关闭上游 Issue，没有自动安装新版本。完整摘要见 [发布证据](issues/2026-10-19-issue155-release-evidence.json)。


<a id="v110"></a>

## v1.1.0 发布记录

来源日期：2026-11-04。已发布为 Latest，非草稿、非预发布；以下结果属于本次发布核验。

本地验证：`npm run check`、`npm run tauri:build -- --debug --no-bundle`、Rust 格式与差异检查通过；portable 更新清单测试 2 项通过；工作流 YAML、仅标签触发、五处版本一致性及原签名公钥已核对。前端保留既有 chunk 大小和动态导入提示，构建成功。

本版本集中发布 v1.0.17 之后已合入主线的维护修复，继续处理原仓库遗留的 Issue 和社区 PR。

## 升级前请注意

- **插件协议升级到 API 2**：三个内置插件已适配。API 1 的外部插件需要适配新协议，旧插件会被明确拒绝加载。
- 更新前请完全退出应用并备份整个数据目录 `~/.cursor-byok-v3/`（Windows 为 `%USERPROFILE%\.cursor-byok-v3`），保留模型、凭据和数据库。
- 自动更新地址改为当前维护仓库 `haohaomin/cursor-byok`，沿用原有签名公钥。

## 主要更新

- **OAuth 续期**：Grok、ChatGPT/Codex、Google 账号由宿主准备并持久化轮换凭据；支持有限认证重试、并发保护、取消和重置处理，修复 Google 手动额度刷新及详细日志下认证重试失败。
- **Remote SSH 与搜索**：加入配置指南；WebFetch 按会话分页；远程 Semble 通过 Cursor 采集工作区，支持忽略规则、大目录分页、取消和 Python 导入隔离。相关审批限制见下文。
- **会话与工具**：修复重复用户消息打断工具、MCP schema 整数精度，并补齐 Responses 文字、Anthropic 拒绝处理及 Steer 后子代理结果交付的后续修复。
- **数据库与网络**：清理统计后回收 SQLite 文件空间；避免失败事务污染连接池；系统代理变化后重建客户端；按会话生成提示缓存键。
- **维护文档**：默认中文 README，可切换 English；新增个人持续维护说明、Issue/PR 处理清单、验证边界和免责声明。

## 已知限制

- OAuth 续期通过合成网络与真实 Deno 宿主测试，真实 Grok、ChatGPT、Google 账号尚未完成验收。
- 远程 Semble 采集实现已包含，但 Cursor 审批曾出现自动取消；独立终端审批卡片的 10 个本地候选文件没有进入本版本，真实批准/拒绝及 Windows Cursor UI 仍待验证。
- [上游 #155](https://github.com/leookun/cursor-byok/issues/155) 后台任务保持“完成一半”：已有普通短任务自动续答验证，审批、压缩等边界仍待验收；本轮不测试 20 分钟任务。
- [上游 #505](https://github.com/leookun/cursor-byok/issues/505) 多 reasoning signature 等仍在处理，不能将本次发版视为所有上游问题已解决。

安装包和更新清单以本 Release 的 Assets 为准。更详细的进度见 [工作状态](https://github.com/haohaomin/cursor-byok/blob/main/docs/work-status.md)及 [维护状态](https://github.com/haohaomin/cursor-byok/blob/main/docs/maintenance-status.md)。

### 发布与核验结果

- 发布提交：`d85d8ee0cac3ca73903edc7461320c2ad4c55886`，标签 `v1.1.0` 位于 `origin/main` 历史中。
- [四平台发布工作流](https://github.com/haohaomin/cursor-byok/actions/runs/41792371217)及[发布提交 CI](https://github.com/haohaomin/cursor-byok/actions/runs/41792325239)：全部成功。
- [v1.1.0 Release](https://github.com/haohaomin/cursor-byok/releases/tag/v1.1.0)：Latest，`draft=false`、`prerelease=false`。
- 实际下载全部附件，核对文件大小与 GitHub 提供的 SHA-256 摘要；七个安装/更新包通过原 updater 公钥的 Minisign 签名验证。
- `latest.json`、`portable-latest.json`、`update.json` 均为 1.1.0；平台覆盖、下载 URL、签名以及旧版更新包的大小与 SHA-256 全部匹配。
- 新旧仓库用户名的三个 Latest 更新入口均能获取 1.1.0 清单；没有更换签名身份。
- 本次没有安装或替换本机应用，不把产物下载和签名验证记为应用内自动安装验收。
- [结构化证据](2026-11-04-v1.1.0-release-evidence.json)。

| 签名产物 | SHA-256 | 结果 |
| --- | --- | --- |
| Cursor.BYOK-1.1.0-1.x86_64.rpm | `4f37a76a2ec711b6fabfd6ed88f2ef7adaf29e5e5428aa2028202427c88f26d9` | 通过 |
| Cursor.BYOK_1.1.0_aarch64.app.tar.gz | `4dad48d94aec3ee7fe3c3f07e3d21289b41dcece4eb0524b5f98891523a04741` | 通过 |
| Cursor.BYOK_1.1.0_amd64.AppImage | `c6a38adfa27f4ba8188610e8fba9a054c5d837da76edca2aa2c1e7b154493d9d` | 通过 |
| Cursor.BYOK_1.1.0_amd64.deb | `4b26cb947fce8eb47ac615334eda222beeac61bf77412be47664dff72c6691a3` | 通过 |
| Cursor.BYOK_1.1.0_x64-setup.exe | `64b67236db116a45c5296c62e6b6ca2e358aa180fc821e7ccdf054cd603d7c92` | 通过 |
| Cursor.BYOK_1.1.0_x64.app.tar.gz | `bf500d027ce9c16fe464b53124de31d96d0fe1828ffe04b72315ac6db4213977` | 通过 |
| cursor-byok-1.1.0-windows-amd64.zip | `cb40821f94c5879f24744ff1142ef6df66ed053ff30ba9864c8476498780d348` | 通过 |
