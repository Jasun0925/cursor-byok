<div align="center">

# Cursor BYOK · 无广告改版

在 Cursor 中使用自己的模型 API，保留 Agent、工具调用、Skills 和 MCP 工作流。

[English](./README.md) · [下载最新版本](https://github.com/Jasun0925/cursor-byok/releases/latest) · [版本记录](https://github.com/haohaomin/cursor-byok/releases) · [问题反馈](https://github.com/haohaomin/cursor-byok/issues) · [修复进度](./docs/maintenance-status.md) · [提交改进](https://github.com/haohaomin/cursor-byok/pulls) · [上游项目](https://github.com/leookun/cursor-byok)

[![Release](https://img.shields.io/github/v/release/Jasun0925/cursor-byok?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)
[![CI](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)

</div>

![Cursor BYOK 控制面板](./images/en-home-1.png)

## 这是什么版本

这是 [leookun/cursor-byok](https://github.com/leookun/cursor-byok) 的个人维护分支，由 **haohaomin** 维护。应用在本机运行模型网关，将 Cursor 的 Agent 请求转换为你配置的模型接口请求。

本分支移除了广告组件，使用独立的 GitHub Releases、更新源和更新签名，并保留 TAB 公益服务、官方直连和自定义服务选项。上游更新按需同步；候选 PR 经检查和验证后再整合。

源码 `main`、测试分支与正式安装包可能处于不同进度。安装请以本仓库 [Latest Release](https://github.com/Jasun0925/cursor-byok/releases/latest) 为准，具体改动以该版本的发布说明为准。

> cursor-byok 免费开源，模型 API 可能收费。本项目与 Cursor 官方无隶属关系。本地网关仍会向你选择的模型服务发送请求，并不意味着模型在本机运行。

## 上游归档后的维护

原仓库已归档。本 fork 继续整理原有 Issue 与 PR，复现后修复并保留贡献者署名。查看[已修复内容与剩余问题](./docs/maintenance-status.md)，区分已发布版本和仅在源码中的改动。2026-10-27 清单覆盖 104 个开放 Issue、51 个开放 PR；收集完成不代表所有问题已经解决。

遇到问题请到[本仓库 Issues](https://github.com/haohaomin/cursor-byok/issues)反馈，并附原问题链接。原仓库归档后无法新增评论。

在 Remote SSH 中使用 Agent，请按[远程主机配置指南](./docs/remote-ssh.md)设置路由。基础 Agent 已实测可经本机 BYOK 使用；WebFetch 分页已在源码修复并实测，v1.0.17 安装包尚未包含。

## 主要功能

- **自带模型服务**：配置接口地址、API Key、模型 ID、上下文容量和生成参数。
- **三种请求协议**：OpenAI Responses、OpenAI Chat Completions、Anthropic Messages。
- **模型管理与测试**：添加、复制、排序、批量测试，查看首字延迟和生成速度。
- **Agent 工作流**：工具调用、子代理、Skills、MCP、多轮对话与上下文压缩；实际能力取决于模型和接口兼容性。
- **调用记录**：查看请求状态、Token 用量、缓存指标和错误信息。
- **插件与外部 API**：管理内置插件，也可通过设置中的外部 API 向其他本地应用提供模型接口。
- **TAB 独立配置**：选择公益服务、官方直连或自定义服务。
- **跨平台桌面应用**：macOS、Windows 和 Linux。

### 本分支已发布的修复

| 版本 | 修复内容 |
| --- | --- |
| [v1.0.4](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.4) | 子代理在 Steer 中断后保留执行，并将完成结果交付给主对话。 |
| [v1.0.5](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.5) | Anthropic 明确拒绝时不执行附带工具、不自动重试，并向 Cursor 标记为不可重试。 |
| [v1.0.6](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.6) | 按输出项补齐 Responses 文本，修复已覆盖场景中的重复输出；调整工具完成事件与后续文字的顺序，修复成功工具卡片误显示 Skipped。 |
| [v1.0.7](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.7) | 补齐仅在 Responses 最终快照中返回的文字和缺失后缀，避免重复显示已有内容。 |

后续版本见[完整发布记录](https://github.com/haohaomin/cursor-byok/releases)。测试分支中的修复不会自动进入正式版本。

## 下载与安装

前往 [Latest Release](https://github.com/Jasun0925/cursor-byok/releases/latest)，在 **Assets** 中选择与你的系统和处理器一致的文件。

| 平台 | 选择的安装包 |
| --- | --- |
| macOS · Apple Silicon（M 系列） | `aarch64.app.tar.gz`，解压后将应用放入“应用程序”目录。 |
| macOS · Intel | `x64.app.tar.gz`，解压后将应用放入“应用程序”目录。 |
| Windows · x64 | `x64-setup.exe` 安装包；需要便携版时选择 `windows-amd64.zip`。 |
| Linux · x64 | 按发行版选择 `.AppImage`、`.deb` 或 `.rpm`；AppImage 需具有执行权限。 |

`.sig` 是更新签名文件，`latest.json`、`portable-latest.json`、`update.json` 是更新清单，无需手动打开或安装。

## 从源码构建安装包

需要使用源码中尚未发布的修复时，可在自己的电脑上构建安装包。

1. 安装 Rust stable、Node.js 22 与 npm、Make，以及 [Tauri 2 对应平台依赖](https://v2.tauri.app/start/prerequisites/)。Windows 可通过 Scoop 或 Chocolatey 安装 Make。
2. 拉取代码并构建：

   ```bash
   git clone https://github.com/Jasun0925/cursor-byok.git
   cd cursor-byok
   npm --prefix apps/desktop ci
   make build-desktop
   ```

   首次构建需要编译全部依赖，耗时较长。构建时会在 `.tauri/cursor-byok.local.key` 生成本地更新签名密钥，该目录已被 git 忽略。
3. 先完全退出正在运行的 Cursor BYOK（包括托盘中的后台实例），再安装构建产物：

| 系统 | 构建产物 | 安装方式 |
| --- | --- | --- |
| macOS | `target/release/bundle/macos/Cursor BYOK.app` | 在 Finder 中拖入“应用程序”目录，替换旧版。 |
| Windows | `target/release/bundle/nsis/*-setup.exe` | 运行安装程序。 |
| Linux | `target/release/bundle/` 下的 `deb`、`rpm`、`appimage` 目录 | 按发行版安装对应文件。 |

- 在本机构建的 macOS 应用可以直接打开。将 `.app` 复制到其他 Mac 后，如提示“已损坏”或无法验证开发者，请执行 `xattr -dr com.apple.quarantine "/Applications/Cursor BYOK.app"`。
- macOS 的“App 管理”权限可能阻止终端替换 `/Applications` 中的应用，请改用 Finder 拖拽。
- 自建包沿用仓库中的版本号和更新源。应用提示更新时，安装的是 Releases 中的正式包，会覆盖自建包。需要保留源码修复时请忽略更新，之后拉取新代码重新构建。
- 替换应用不会影响数据目录，模型和设置会保留，详见[升级、替换与数据保留](#升级替换与数据保留)。

## 快速开始

1. 安装并启动 Cursor BYOK，同时准备服务商提供的接口地址、API Key 和模型 ID。
2. 根据应用中的 Cursor 配置提示完成本地 CA 初始化、证书信任和本地连接配置；需要系统授权时按提示操作。
3. 在 **模型设置** 中添加模型，选择实际支持的协议，保存后运行 **测试**。
4. 按界面提示启用 Cursor 本地接入，确认连接状态正常，并保持 Cursor BYOK 运行。
5. 完全退出并重启 Cursor，新建对话，在模型列表中手动选择配置的模型。使用 BYOK 模型时不要选 **Auto**。
6. 先测试一条简单对话，再尝试读取文件或执行一条可确认结果的工具调用。

Cursor 升级后若连接失效，重新检查应用中的 Cursor 配置，并完全重启 Cursor。通用安装说明可参考[上游中文文档](https://docs.leokun.cn/zh/docs)；下载与更新请使用本分支入口，反馈方式见文末。

## 模型配置

![模型设置](./images/en-model-1.png)

| 服务商支持的接口 | 应选择的协议 |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

以服务商文档为准，不要只凭模型名称判断协议。缓存支持、费用及思考参数同样取决于具体服务，不是选择某一协议就能保证。

- **服务器地址**：支持基础地址和完整请求 URL；确认最终路径与服务商要求一致。
- **模型 ID**：必须是接口接受的标识；显示名称只影响 Cursor 中的名称。
- **上下文窗口与最大输出**：按模型实际能力配置，避免超过接口限制。
- **自定义 Headers / 额外参数**：使用 JSON 对象，只填写服务商支持的字段。
- **工具调用**：连接测试通过后仍需实测；能生成文字不代表工具调用兼容。

## 升级、替换与数据保留

本分支安装包使用独立更新源。可在应用中检查更新，也可从本仓库 Releases 手动下载。原上游应用不会因为存在这个 fork 就自动切换到本分支。

桌面版默认把运行数据保存在用户主目录下，独立于应用安装目录：

| 系统 | 默认数据目录 |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

```text
.cursor-byok-v3/
├── cursor-byok.db   # 模型、设置、运行记录等 SQLite 数据
├── ca/              # 本地 CA 证书与私钥
├── plugins/         # 已安装插件、插件数据与运行环境
└── logs/            # 桌面运行日志
```

**从现有 v3 安装切换到本分支时：**

1. 完全退出 Cursor BYOK，包括托盘中的后台实例；备份整个数据目录。
2. 安装或替换为本仓库的应用，保留原数据目录，不使用会清理应用数据的卸载选项。
3. 以同一个系统用户启动，检查模型、设置和记录，再确认 Cursor 接入状态。
4. 同一份数据库只运行一个应用实例。需要回退时，先退出应用，再配合升级前的备份恢复，避免旧程序读取新版本数据库。

停止应用后再备份，可避免只复制 SQLite 主文件而漏掉尚在 WAL 中的数据。备份中可能包含 API Key、插件凭据和 CA 私钥，请妥善保管。上述路径适用于默认桌面配置；自行部署服务端时，数据库也可以通过 `CURSOR_DATABASE_URL` 指定。更旧的数据格式不能仅凭复制文件假定兼容。

## TAB 与官方账号

在 **系统设置 → TAB 设置** 中选择：

| 模式 | 行为 |
| --- | --- |
| 公益服务 | 保留上游提供的公共 TAB 服务入口，可用性由服务提供方决定。 |
| 直连 | 使用当前 Cursor 账号对应的官方 TAB 服务，受账号权益和额度限制。 |
| 自定义 | 填写你自己的 TAB 服务地址。 |

移除广告组件不等于移除公益 TAB 入口。TAB 与 Agent 模型通道独立，Agent 的 API Key 不会自动提供官方 TAB 额度。

可在 Cursor 中使用自己的官方账号；**Auto 使用官方模型，不会自动选择 BYOK 模型**。官方功能仍受账号权限及 Cursor 版本支持情况影响。

## 常见问题

| 现象 | 建议检查 |
| --- | --- |
| Cursor 找不到配置的模型 | 确认模型配置和本地接入已启用，完全重启 Cursor，并新建对话。 |
| 模型测试成功，但 Agent 工具失败 | 检查服务商是否支持所选协议的流式工具调用，查看调用记录中的错误。 |
| 401 / 403 | 核对密钥、接口地址、模型权限或插件登录状态。 |
| 429 / 配额不足 | 查看服务商额度与限速，等待恢复或切换模型通道。 |
| 升级后看不到原数据 | 确认系统用户、数据目录和运行的应用副本；不要先清空数据库。 |
| TAB 不可用但 Agent 正常 | 单独检查 TAB 模式、服务可用性及官方账号额度。 |

## 运行方式与项目结构

```text
Cursor Agent 请求 / 工具结果
    ↓
本地接入与协议编译 → Agent 运行时 ↔ SQLite（会话与运行状态）
    ↓                       ↑
供应商协议适配 → 模型 API → 文本 / 工具请求
                            ↓
                       Cursor 执行工具 → 返回结果，继续本轮
```

```text
cursor-byok/
├── apps/desktop/       # React 界面与 Tauri 桌面生命周期
├── server/
│   ├── src/
│   │   ├── cursor/    # Cursor 协议、请求编译与展示
│   │   ├── local_app/ # 本地应用接入、代理与证书
│   │   ├── run/       # Agent 循环、工具轮次与压缩
│   │   ├── provider/  # 模型协议及响应适配
│   │   ├── store/     # SQLite 持久化
│   │   ├── plugin/    # 插件宿主与生命周期
│   │   ├── api/       # HTTP 与 Connect 路由
│   │   └── control/   # 管理界面使用的控制 API
│   ├── plugins/       # 内置插件实现
│   ├── prompt/        # 提示词与运行时模板
│   └── migrations/    # 数据库结构变更
├── crates/semble-core/ # 本地代码索引与搜索
├── protocols/cursor/  # Cursor protobuf 协议源
├── support/           # 协议提取、调试与基准测试工具
└── .github/workflows/ # 持续集成与桌面发布
```

## 本地开发

需要 Rust stable、Node.js 22 与 npm，以及 [Tauri 2 对应平台依赖](https://v2.tauri.app/start/prerequisites/)。使用 Make 命令时还需安装 Make；仅构建 Docker 镜像时需要 Docker。

```bash
git clone https://github.com/haohaomin/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci

make dev-web         # 启动本地服务与 Web 管理界面
make dev-desktop     # 启动 Tauri 桌面开发模式
```

上面两条开发命令按需选择。开发实例也会使用本机数据，测试时应与正式实例隔离。

```bash
make check           # Rust 格式、Clippy、测试及前端检查
make build-web       # 构建前端
make build-server    # 构建独立 Rust 服务端
make build-desktop   # 构建本地桌面包
make build-docker    # 构建 Docker 镜像
```

`make build-desktop` 使用本地开发签名，不等于正式发布。正式安装包与更新产物由仓库发布工作流生成。

## 反馈、贡献与致谢

问题请提交到[本仓库 Issues](https://github.com/haohaomin/cursor-byok/issues)，代码改进可提交到[本仓库 Pull Requests](https://github.com/haohaomin/cursor-byok/pulls)。原仓库已归档；与既有上游报告相同的问题，请在反馈中附原链接、使用的版本和改动。反馈请包含操作系统、Cursor 与 BYOK 版本、协议、复现步骤及脱敏后的错误信息，不要附带密钥、凭据或完整私人对话。

欢迎提交范围明确的 PR。先阅读 [AGENTS.md](./AGENTS.md) 和相关目录说明，添加必要的回归测试并运行 `make check`。上游通用修复应独立于本分支的发布配置和界面定制。

感谢原作者 [leookun](https://github.com/leookun) 与[上游贡献者](https://github.com/leookun/cursor-byok/graphs/contributors)。通用使用说明与社区入口见[上游项目](https://github.com/leookun/cursor-byok)及[上游项目文档](https://docs.leokun.cn)。页面截图沿用上游素材，实际界面以安装版本为准。

本项目遵循 [MIT License](./LICENSE)，保留原有版权声明。
