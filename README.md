<div align="center">

# Cursor BYOK

**在 Cursor 中使用自己的模型 API，继续使用熟悉的 Agent 工作流。**

免费开源 · 无广告 · macOS / Windows / Linux

[English](./README-EN.md) · [Русский](./README-RU.md) · [下载最新版](https://github.com/haohaomin/cursor-byok/releases/latest) · [快速开始](#快速开始) · [维护进度](#持续维护与上游问题处理) · [文档](docs/README.md) · [免责声明](#免责声明) · [问题反馈](https://github.com/haohaomin/cursor-byok/issues)

[![Release](https://img.shields.io/github/v/release/haohaomin/cursor-byok?style=flat-square)](https://github.com/haohaomin/cursor-byok/releases/latest)
[![CI](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)

</div>

> [!IMPORTANT]
> **上游已停止维护，本仓库由 [haohaomin](https://github.com/haohaomin) 个人持续维护。** 原项目 [leookun/cursor-byok](https://github.com/leookun/cursor-byok) 已归档。我正在继续排查和修复上游遗留的 Issue，评估尚未合入的 PR，并维护后续版本。下载、更新、问题反馈与贡献请使用本仓库的入口。感谢原作者与所有贡献者。

## 持续维护与上游问题处理

**当前维护重点是继续解决上游遗留问题，并把经过验证的社区改进带入这个 fork。** 我会结合问题影响、复现条件和个人可投入的时间推进，公开记录已完成的修复、未解决的问题和验证边界。

| 维护方向 | 处理方式与进度 |
| --- | --- |
| 上游 Issue | 逐项复现、定位、修复和回归验证；缺少证据或尚未完成实机验收的条目继续保留待办。查看 [Issue 清单](docs/issues.md)。 |
| 上游 PR | 审查适用性与风险，选择采纳、适配补全或重新实现，保留贡献者署名与验证记录。收录进清单不代表已经合并。查看 [PR 清单](docs/pull-requests.md)。 |
| 版本与兼容性 | 持续修复本 fork 的问题、跟进 Cursor 变化并发布更新；源码合入、自动化测试、真实环境验收和安装包发布分别记录。查看 [维护状态](docs/maintenance-status.md)与 [版本记录](https://github.com/haohaomin/cursor-byok/releases)。 |

已有工作覆盖 Responses 输出、子代理执行与结果交付、SQLite、OAuth 续期及 Remote SSH 等方向；后台任务、真实账号和客户端审批等场景仍有待处理或验收的部分。**这不表示所有上游 Issue 已解决，也不表示所有 PR 都会合入。** 最新已完成/未完成事项见 [工作状态](docs/work-status.md)，详细资料见 [文档导航](docs/README.md)。

**源码中的修复可能尚未进入安装包，下载前请以对应版本的发布说明为准。**

![Cursor BYOK 控制面板](./images/en-home-1.png)

## 可以做什么

Cursor BYOK 在本机运行模型网关，将 Cursor 的 Agent 请求接入你配置的模型服务。你可以管理自己的模型、查看调用记录，并在 Cursor 中继续完成对话、文件操作和工具调用。

| 能力 | 说明 |
| --- | --- |
| 自选模型服务 | 配置接口地址、API Key、模型 ID、上下文容量及生成参数。 |
| 三种 API 协议 | OpenAI Responses、OpenAI Chat Completions、Anthropic Messages。 |
| Agent 工作流 | 工具调用、子代理、Skills、MCP、多轮对话与上下文压缩。实际可用性取决于模型、接口和 Cursor 版本。 |
| 模型管理与诊断 | 添加、复制、排序、批量测试；查看首字延迟、生成速度、Token 用量、缓存指标与错误。 |
| 插件与外部 API | 管理内置插件，也可为其他本地应用提供模型接口。 |
| TAB 独立配置 | 保留公益服务、官方直连和自定义服务选项。 |

应用免费开源，模型 API 费用由服务商收取。请求会发送到你选择的模型服务；使用本地网关不代表模型在本机推理。

## 下载与安装

**前往 [Releases 下载最新版](https://github.com/haohaomin/cursor-byok/releases/latest)**，在 **Assets** 中按系统和处理器选择安装包：

| 平台 | 文件名特征 | 安装方式 |
| --- | --- | --- |
| macOS · Apple Silicon（M 系列） | `*_aarch64.app.tar.gz` | 解压，将应用放入“应用程序”。 |
| macOS · Intel | `*_x64.app.tar.gz` | 解压，将应用放入“应用程序”。 |
| Windows · x64 | `*_x64-setup.exe` | 运行安装程序；便携版选择 `*-windows-amd64.zip`。 |
| Linux · x64 | `*_amd64.AppImage` / `*_amd64.deb` / `*.x86_64.rpm` | 按发行版选择；AppImage 需具有执行权限。Linux 依赖与兼容边界见 [安装指南](docs/linux.md)。 |

首次安装优先选择上表中的桌面包。`.sig` 是更新签名，`latest.json`、`portable-latest.json`、`update.json` 是更新清单，无需手动安装。

安装包使用本仓库独立的更新源和签名。**从上游版本切换，请先看[升级与数据保留](#升级与数据保留)**；原上游应用不会自动切换到本仓库的更新源。

## 快速开始

准备好模型服务的 **接口地址、API Key 和模型 ID**，然后：

1. **启动 Cursor BYOK**，按应用内提示完成 Cursor 本地接入、CA 初始化和证书信任；公司设备禁止安装证书时，请先阅读 [CA 与无证书使用说明](docs/certificates.md)。
2. **添加模型**：进入模型设置，填写服务信息，选择对应协议，保存并运行连接测试。
3. **确认接入状态**：按界面提示启用 Cursor 本地接入，保持 Cursor BYOK 运行。
4. **重启 Cursor 并选择模型**：完全退出后重新打开 Cursor，新建对话，手动选择刚配置的模型。**Auto 使用官方模型，不会自动选择 BYOK 模型。**
5. **验证一次完整操作**：先发送简单消息，再让 Agent 读取一个文件，确认文字和工具结果都能正常返回。

Cursor 更新后如果连接失效，重新检查应用内的 Cursor 配置并完全重启 Cursor。远程开发请继续阅读 [Remote SSH 配置指南](docs/remote-ssh.md)。

## 模型配置

按服务商实际支持的接口选择协议，不要仅凭模型名称判断：

| 服务商接口 | 应选择的协议 |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

服务器地址支持基础地址或完整请求 URL；模型 ID 必须使用 API 接受的标识，显示名称只影响 Cursor 中的名称。上下文窗口、最大输出、自定义 Headers 和额外参数均按服务商文档填写；Headers 和额外参数使用 JSON 对象。

连接测试通过后，还需验证流式工具调用。缓存支持、思考参数和费用也取决于具体模型服务。

<details>
<summary>查看模型设置界面</summary>

![模型设置](./images/en-model-1.png)

</details>

## 升级与数据保留

可在应用内检查更新，也可从本仓库 [Releases](https://github.com/haohaomin/cursor-byok/releases) 手动下载。桌面版的数据保存在用户主目录，独立于应用安装目录：

| 系统 | 默认数据目录 |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

**更新或从现有 v3 安装切换时：**

1. 完全退出 Cursor BYOK，包括托盘中的后台实例，备份整个数据目录。
2. 安装本仓库的版本，保留原数据目录，避免使用会清理应用数据的卸载选项。
3. 以同一个系统用户启动，检查模型、设置、记录和 Cursor 接入状态。同一份数据库只运行一个实例。

停止应用后再备份整个目录，避免遗漏 SQLite WAL 数据。备份可能包含 API Key、插件凭据与 CA 私钥，请妥善保管。回退时应配合升级前的备份恢复；更旧的数据格式是否兼容需单独确认。

<details>
<summary>数据目录包含什么</summary>

```text
.cursor-byok-v3/
├── cursor-byok.db   # 模型、设置与运行记录
├── ca/             # 本地 CA 证书与私钥
├── plugins/        # 插件、插件数据与运行环境
└── logs/           # 桌面运行日志
```

以上为默认桌面配置；独立服务端可通过 `CURSOR_DATABASE_URL` 指定数据库。

</details>

## TAB 与远程开发

在 **系统设置 → TAB 设置** 中，可选择公益服务、官方直连或自定义服务地址。公益服务的可用性由提供方决定；官方直连受当前 Cursor 账号权益和额度限制。TAB 与 Agent 模型通道独立，Agent API Key 不会提供官方 TAB 额度。

**Remote SSH 的基础 Agent 已有真实验证。** 需要按远程主机配置请求路由，见 [Remote SSH 指南](docs/remote-ssh.md)。远程 WebFetch、Semble 与审批的修复进度和验证范围不同，请同时查看[维护状态](docs/maintenance-status.md)。

## 常见问题

| 现象 | 建议检查 |
| --- | --- |
| Cursor 找不到配置的模型 | 确认模型和本地接入已启用，完全重启 Cursor，并新建对话。 |
| 模型测试成功，但 Agent 工具失败 | 核对所选协议的流式工具调用支持，查看调用记录中的具体错误。 |
| 401 / 403 | 核对密钥、接口地址、模型权限或插件登录状态。 |
| 429 / 配额不足 | 查看服务商额度与限速，等待恢复或切换模型通道。 |
| 后台命令结束后，对话没有继续 | 该问题仍有待验收的边界。可要求 Agent 读取已有终端输出并继续，避免重跑命令；进度见[维护状态](docs/maintenance-status.md)。 |
| 升级后看不到原数据 | 先确认系统用户、数据目录和运行的应用副本，不要先清空数据库。 |
| TAB 不可用但 Agent 正常 | 单独检查 TAB 模式、服务可用性和官方账号额度。 |

## 本地开发

需要 Rust stable、Node.js 22、npm、Make，以及 [Tauri 2 平台依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/haohaomin/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci
make dev-desktop
```

也可以用 `make dev-web` 启动本地服务与 Web 管理界面。开发实例应与正式数据隔离。

| 命令 | 用途 |
| --- | --- |
| `make check` | Rust 格式、Clippy、测试与前端检查。 |
| `make build-web` | 构建前端。 |
| `make build-server` | 构建独立 Rust 服务端。 |
| `make build-desktop` | 使用本地开发签名构建桌面包。 |
| `make build-docker` | 构建 Docker 镜像，需安装 Docker。 |

<details>
<summary>项目结构与请求流转</summary>

```text
cursor-byok/
├── apps/desktop/       # React 界面与 Tauri 桌面生命周期
├── server/            # 本地网关与 Agent 运行时
│   ├── src/cursor/    # Cursor 协议、请求编译与展示
│   ├── src/local_app/ # 本地接入、代理与证书
│   ├── src/run/       # Agent 循环、工具轮次与压缩
│   ├── src/provider/  # 模型协议及响应适配
│   ├── src/store/     # SQLite 持久化
│   ├── src/plugin/    # 插件宿主与生命周期
│   ├── plugins/       # 内置插件实现
│   ├── prompt/        # 提示词与运行时模板
│   └── migrations/    # 数据库结构变更
├── crates/semble-core/ # 代码索引与搜索
├── protocols/cursor/  # Cursor protobuf 协议源
├── docs/              # 使用说明、维护状态与历史证据
├── support/           # 协议提取、调试与基准测试
└── .github/workflows/ # 持续集成与桌面发布
```

```text
Cursor → 本地接入 / 协议编译 → Agent 运行时 ↔ SQLite 会话状态
                                   ↕
                              协议适配 ↔ 模型 API
                                   ↓
                        文字返回 Cursor / 工具请求
                                   ↓
                        Cursor 执行工具 → 结果回到运行时
                                         继续本轮，直至结束或取消
```

模型协议可在 `server/src/provider/` 扩展，插件由 `server/src/plugin/` 宿主管理。正式发布包与更新产物由仓库发布工作流生成。

</details>

## 反馈与贡献

欢迎到[本仓库 Issues](https://github.com/haohaomin/cursor-byok/issues)反馈问题，或通过 [Pull Requests](https://github.com/haohaomin/cursor-byok/pulls)提交改进。反馈请提供 **操作系统、Cursor/BYOK 版本、模型协议、复现步骤和脱敏错误**；与上游已有 Issue 或 PR 相关时，请先查看维护清单，并附上原链接；欢迎补充复现样例、脱敏日志和不同平台的验证结果。

提交代码前请阅读 [AGENTS.md](./AGENTS.md)，保持改动范围明确，按改动补充必要的回归验证并运行相关检查。新功能或较大改动建议先通过 Issue 讨论。不要在公开反馈中附带密钥、凭据或私人对话。

## 免责声明

- **独立维护项目**：本仓库是社区个人维护的 fork，与 Cursor 官方及模型服务商无隶属、合作或背书关系，也不代表原作者指定了本仓库作为官方继任项目。相关名称与商标归各自权利人所有。
- **兼容性与维护范围**：可用性取决于 Cursor 版本、模型能力、接口协议和第三方服务状态。维护者不保证所有功能、账号或环境持续兼容，也不承诺每个问题的修复时间；已通过的测试仅代表所记录的验证范围。
- **账号、数据与费用**：请使用有权访问的账号、密钥和服务，并遵守相关服务条款及适用法律。你需要自行选择和评估模型服务商，妥善保管凭据、备份重要数据、审阅 Agent 的文件和命令操作，并承担所选服务产生的费用。本地网关不等于离线处理，请勿向无权接收相关信息的服务提交敏感内容。
- **无担保与责任限制**：软件按 [MIT License](./LICENSE)“按现状”提供，不作明示或默示担保。在适用法律允许的范围内，作者和版权持有人不对因使用本软件产生的索赔、损害或其他责任承担责任；完整的授权、担保与责任限制以许可证正文为准。

## 致谢与许可

本仓库基于 [leookun/cursor-byok](https://github.com/leookun/cursor-byok) 独立维护，感谢 [leookun](https://github.com/leookun) 与[原项目贡献者](https://github.com/leookun/cursor-byok/graphs/contributors)。页面截图沿用原项目素材，实际界面以安装版本为准。Semble 的来源与署名见 [UPSTREAM.md](./UPSTREAM.md)。

项目采用 [MIT License](./LICENSE)，保留原有版权声明及贡献者署名。
