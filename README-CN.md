<div align="center">

# Cursor BYOK

在 Cursor 中使用自己的模型 API，保留 Agent、工具调用、Skills 和 MCP 工作流。

[English](./README.md) · [下载最新版本](https://github.com/Jasun0925/cursor-byok/releases/latest) · [版本记录](https://github.com/Jasun0925/cursor-byok/releases) · [问题反馈](https://github.com/Jasun0925/cursor-byok/issues)

[![Release](https://img.shields.io/github/v/release/Jasun0925/cursor-byok?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)
[![CI](https://github.com/Jasun0925/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/Jasun0925/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)

</div>

![Cursor BYOK 控制面板](./images/en-home-1.png)

## 这是什么

这个仓库是 [Jasun0925/cursor-byok](https://github.com/Jasun0925/cursor-byok)。应用在本机运行网关，把 Cursor 的 Agent 请求转换成你配置的模型接口请求。

它基于已归档的 [leookun/cursor-byok](https://github.com/leookun/cursor-byok)。安装包、更新和问题反馈都使用本仓库。当前发布版本是 [v1.0.22](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.22)。

> cursor-byok 免费开源，模型 API 可能收费。本项目与 Cursor 官方无隶属关系。本地网关会把请求发到你选择的模型服务。

## 本仓库已发布的版本

| 版本 | 改动 |
| --- | --- |
| [v1.0.18](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.18) | Agent 对话继续走 `AgentService/RunSSE`。Cursor 打开 `agent_host_local_loop` 后会把请求改到 `InferenceService/RunInference`，本网关无法按原协议解码，并报 `ConversationStateStructure.summary` 的 `ThirtyTwoBit`。更新地址和更新签名改为本仓库。 |
| [v1.0.19](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.19) | 解码历史消息里以 `{ "__type": "Uint8Array", "hex": "..." }` 保存的用户图片。已有图片的对话不再报 `Cursor message is missing image`。 |
| [v1.0.20](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.20) | 工具读到的图片在下一轮仍然交给模型。成功编辑写入 `file_states_v2`，重载后 Review 仍能列出文件。macOS 重新打开窗口时保持 Dock 图标的隐藏状态。 |
| [v1.0.21](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.21) | 可在系统设置里开启 CLIProxyAPI 标签页：在所选目录启动 `cli-proxy-api`、查看输出、打开它的管理页面。最小化收到菜单栏托盘。移除「使用教程」标签页。 |
| [v1.0.22](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.22) | 系统设置可以清理已没有对话、图片或进行中任务使用的会话数据块，并回收空间。内置的 Grok、ChatGPT/Codex、Google 插件会自动续期 OAuth 凭据（插件 API 2，来自上游 v1.1.0）。BidiAppend 解码失败时日志不再写入请求内容。 |

请从[最新发布](https://github.com/Jasun0925/cursor-byok/releases/latest)安装。每个版本的具体说明以该 Release 页面为准。

## 主要功能

- **自带模型服务**：配置接口地址、API Key、模型 ID、上下文容量和生成参数。
- **三种请求协议**：OpenAI Responses、OpenAI Chat Completions、Anthropic Messages。
- **模型管理与测试**：添加、复制、排序、批量测试，查看首字延迟和生成速度。
- **Agent 工作流**：工具调用、子代理、Skills、MCP、多轮对话和上下文压缩。实际能力取决于模型和接口。
- **调用记录**：查看请求状态、Token 用量、缓存指标和错误信息。
- **插件与外部 API**：管理内置插件，也可以把模型接口提供给其他本地应用。
- **TAB 设置**：公益服务、官方直连或自定义地址。
- **桌面应用**：macOS、Windows 和 Linux。

## 下载与安装

前往[最新发布](https://github.com/Jasun0925/cursor-byok/releases/latest)，选择与系统和处理器一致的文件。

| 平台 | 安装包 |
| --- | --- |
| macOS · Apple Silicon（M 系列） | `aarch64.app.tar.gz`，解压后放入“应用程序”。 |
| macOS · Intel | `x64.app.tar.gz`，解压后放入“应用程序”。 |
| Windows · x64 | `x64-setup.exe`；便携版使用 `windows-amd64.zip`。 |
| Linux · x64 | `.AppImage`、`.deb` 或 `.rpm`。AppImage 需要执行权限。 |

`.sig` 是更新签名。`latest.json`、`portable-latest.json`、`update.json` 是更新清单。

## 快速开始

1. 安装并启动 Cursor BYOK，准备服务商的接口地址、API Key 和模型 ID。
2. 按应用里的 Cursor 配置完成本地 CA、证书信任和本地连接。
3. 在 **模型设置** 中添加模型，选择与接口一致的协议，保存后运行 **测试**。
4. 保持 Cursor BYOK 运行，完全退出 Cursor 后再启动。
5. 新建对话，手动选择配置的模型。**Auto** 使用 Cursor 自己的模型。
6. 先发一条短对话，再做一次能核对结果的文件读取或其他工具调用。

Cursor 升级后如果连接失效，重新做一次应用里的 Cursor 配置，并完全重启 Cursor。

Remote SSH 中使用 Agent 时，把该主机的 `cursorAgentHost.remoteInferenceRoute` 设为 `always`。应用内 **SSH** 按钮有同样的步骤，说明见 [Remote SSH](./docs/remote-ssh.md)。

## 模型配置

![模型设置](./images/en-model-1.png)

| 服务商接口 | 应选择的协议 |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

协议、缓存、费用和思考参数以服务商文档为准。

- **服务器地址**：可以填基础地址或完整请求 URL，最终路径要和服务商一致。
- **模型 ID**：填写接口接受的标识。显示名称只影响 Cursor 里的名称。
- **上下文窗口与最大输出**：按模型实际能力填写。
- **自定义 Headers / 额外参数**：使用 JSON 对象，只填服务商支持的字段。
- **工具调用**：文字测试通过后仍要单独试一次工具调用。

## 数据目录

替换应用不会改动用户主目录里的数据：

| 系统 | 目录 |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

```text
.cursor-byok-v3/
├── cursor-byok.db   # 模型、设置和运行记录
├── ca/              # 本地 CA 证书与私钥
├── plugins/         # 已安装插件和插件数据
└── logs/            # 桌面运行日志
```

复制这个目录前，先完全退出 Cursor BYOK，包括托盘进程。应用仍在运行时，数据可能还在 SQLite 的 WAL 里。备份里可能有 API Key 和 CA 私钥。单独运行服务端时，可以用 `CURSOR_DATABASE_URL` 指定数据库。

可以在应用里检查更新，也可以从本仓库 Releases 下载。同一份数据库只运行一个应用实例。

## TAB 与官方账号

在 **系统设置 → TAB 设置** 中选择：

| 模式 | 行为 |
| --- | --- |
| 公益服务 | 使用公共 TAB 服务，可用性由该服务决定。 |
| 直连 | 使用当前 Cursor 账号的官方 TAB 服务。 |
| 自定义 | 使用你填写的 TAB 地址。 |

TAB 和 Agent 模型通道是分开的。Agent 的 API Key 不会带来官方 TAB 额度。**Auto** 选择的是 Cursor 的模型。

## 常见问题

| 现象 | 检查 |
| --- | --- |
| Cursor 找不到配置的模型 | 模型配置和本地接入已启用，Cursor 已完全重启，并且是新建对话。 |
| 模型测试成功，Agent 工具失败 | 所选协议是否支持流式工具调用，并查看调用记录里的错误。 |
| `Cursor message is missing image` | 安装 [v1.0.19](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.19) 或更新版本。旧版本遇到 `Uint8Array` 图片会停住。 |
| `ConversationStateStructure.summary` / `ThirtyTwoBit` | 安装 [v1.0.18](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.18) 或更新版本。 |
| 401 / 403 | 密钥、接口地址、模型权限或插件登录状态。 |
| 429 / 配额不足 | 服务商额度，或换一个模型通道。 |
| 升级后看不到原数据 | 系统用户、数据目录，以及实际启动的应用副本。 |
| TAB 不可用但 Agent 正常 | TAB 模式、服务可用性和官方账号额度。 |

## 从源码构建

需要 Rust stable、Node.js 22 与 npm、Make，以及 [Tauri 2 对应平台依赖](https://v2.tauri.app/start/prerequisites/)。Windows 可通过 Scoop 或 Chocolatey 安装 Make。

```bash
git clone https://github.com/Jasun0925/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci
make build-desktop
```

首次构建会编译全部依赖，并在 `.tauri/cursor-byok.local.key` 生成本地更新签名密钥。该文件已被 git 忽略。

替换已安装的应用前，先完全退出 Cursor BYOK，包括托盘。

| 系统 | 构建产物 | 安装方式 |
| --- | --- | --- |
| macOS | `target/release/bundle/macos/Cursor BYOK.app` | 在 Finder 中拖入“应用程序”。 |
| Windows | `target/release/bundle/nsis/*-setup.exe` | 运行安装程序。 |
| Linux | `target/release/bundle/` 下的 `deb`、`rpm`、`appimage` | 按发行版安装。 |

本机构建的 macOS 应用可以直接打开。复制到其他 Mac 后如果提示已损坏，执行 `xattr -dr com.apple.quarantine "/Applications/Cursor BYOK.app"`。macOS 的“App 管理”权限可能阻止终端替换 `/Applications` 里的应用，请用 Finder 拖拽。应用内更新会安装 GitHub Release，并覆盖本地构建的应用。替换应用不会改动 `~/.cursor-byok-v3/`。

```bash
make dev-web         # 本地服务和 Web 界面
make dev-desktop     # Tauri 桌面开发
make check           # 格式、Clippy、测试和前端检查
make build-server    # 独立 Rust 服务端
make build-docker    # Docker 镜像
```

开发实例也会使用本机数据，请和正式安装分开。`make build-desktop` 使用本地开发签名。正式安装包由发布工作流生成。

## 项目结构

```text
Cursor Agent 请求和工具结果
    ↓
本地接入与协议编译 → Agent 运行时 ↔ SQLite
    ↓                    ↑
供应商协议适配 → 模型 API → 文本和工具请求
                      ↓
                 Cursor 执行工具
                      ↓
                 结果继续本轮
```

```text
cursor-byok/
├── apps/desktop/        # React 界面和 Tauri 生命周期
├── server/              # 网关、Cursor 协议、模型适配和 SQLite
├── crates/semble-core/  # 本地代码索引与搜索
├── protocols/cursor/    # Cursor protobuf 源
└── .github/workflows/   # 持续集成和桌面发布
```

## 反馈

问题请提交到 [Issues](https://github.com/Jasun0925/cursor-byok/issues)。请写上操作系统、Cursor 版本、BYOK 版本、协议、复现步骤和脱敏后的错误，不要附带密钥或私人对话。

感谢原作者 [leookun](https://github.com/leookun) 和[上游贡献者](https://github.com/leookun/cursor-byok/graphs/contributors)。本项目遵循 [MIT License](./LICENSE)。
