<div align="center">

# Cursor BYOK

Use your own model APIs in Cursor with Agent, tool calling, Skills, and MCP workflows.

[中文说明](./README-CN.md) · [Download](https://github.com/Jasun0925/cursor-byok/releases/latest) · [Releases](https://github.com/Jasun0925/cursor-byok/releases) · [Issues](https://github.com/Jasun0925/cursor-byok/issues)

[![Release](https://img.shields.io/github/v/release/Jasun0925/cursor-byok?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)
[![CI](https://github.com/Jasun0925/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/Jasun0925/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square)](https://github.com/Jasun0925/cursor-byok/releases/latest)

</div>

![Cursor BYOK dashboard](./images/en-home-1.png)

## About

This repository publishes Cursor BYOK from [Jasun0925/cursor-byok](https://github.com/Jasun0925/cursor-byok). The application runs a local gateway that turns Cursor Agent requests into requests to the model APIs you configure.

It is based on [leookun/cursor-byok](https://github.com/leookun/cursor-byok). That upstream repository is archived. Installers, updates, and issues for this line are on this repository. The current release is [v1.0.22](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.22).

> Cursor BYOK is free and open source. Your model provider may charge for API usage. This project is independent of Cursor. The local gateway sends requests to the model service you choose.

## Releases

| Release | What changed |
| --- | --- |
| [v1.0.18](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.18) | Keep Agent turns on `AgentService/RunSSE`. Cursor's `agent_host_local_loop` gate was sending turns to `InferenceService/RunInference`, which this gateway cannot decode and reported as `ConversationStateStructure.summary` wire type `ThirtyTwoBit`. Updates and the updater signing key come from this repository. |
| [v1.0.19](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.19) | Decode user images stored as `{ "__type": "Uint8Array", "hex": "..." }` in conversation history. Chats that already contained those images no longer stop with `Cursor message is missing image`. |
| [v1.0.20](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.20) | Keep images returned by tools available on the next turn. Record successful edit snapshots in `file_states_v2` so Review can list files after a reload. On macOS, preserve a hidden Dock icon when the window is opened again. |
| [v1.0.21](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.21) | Optional CLIProxyAPI tab, enabled in Settings: start `cli-proxy-api` from a chosen directory, view its output, and open its management page. Minimize to the menu bar tray. Remove the tutorial tab. |
| [v1.0.22](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.22) | Settings can remove conversation data blocks that no saved conversation, image, or running task uses, then reclaim the space. Built-in Grok, ChatGPT/Codex, and Google plugins renew OAuth credentials (plugin API 2, from upstream v1.1.0). BidiAppend decode errors no longer write request content to the log. |

Install from the [latest release](https://github.com/Jasun0925/cursor-byok/releases/latest). Release notes there describe that build.

## Features

- **Your own model services:** Configure endpoints, API keys, model IDs, context limits, and generation parameters.
- **Three API protocols:** OpenAI Responses, OpenAI Chat Completions, and Anthropic Messages.
- **Model management and testing:** Add, duplicate, reorder, and batch-test configurations. Inspect time to first token and generation speed.
- **Agent workflows:** Tool calls, subagents, Skills, MCP, multi-turn conversations, and context compaction. The model and API still have to support the feature you use.
- **Call records:** Inspect request status, token usage, cache metrics, and errors.
- **Plugins and external API:** Manage built-in plugins, and optionally expose model endpoints to other local applications.
- **TAB settings:** Public service, official direct, or a custom TAB address.
- **Desktop support:** macOS, Windows, and Linux.

## Download and installation

Open the [latest release](https://github.com/Jasun0925/cursor-byok/releases/latest) and choose the asset for your system.

| Platform | Package |
| --- | --- |
| macOS · Apple Silicon | `aarch64.app.tar.gz`. Extract it and move the application to Applications. |
| macOS · Intel | `x64.app.tar.gz`. Extract it and move the application to Applications. |
| Windows · x64 | `x64-setup.exe`, or `windows-amd64.zip` for the portable build. |
| Linux · x64 | `.AppImage`, `.deb`, or `.rpm`. AppImage needs execute permission. |

Files ending in `.sig` are updater signatures. `latest.json`, `portable-latest.json`, and `update.json` are update manifests.

## Quick start

1. Install and open Cursor BYOK. Have the provider endpoint, API key, and model ID ready.
2. Follow the in-app Cursor setup to create the local CA, trust the certificate, and enable the local connection.
3. In **Model Settings**, add a model, choose the protocol that matches the endpoint, save, and run **Test**.
4. Keep Cursor BYOK running, quit Cursor completely, then start Cursor again.
5. Start a new conversation and select the configured model. **Auto** uses Cursor's own models.
6. Send a short prompt, then a file read or another tool call whose result you can check.

If a Cursor update breaks the connection, repeat the in-app Cursor setup and restart Cursor.

For Agent over Remote SSH, set `cursorAgentHost.remoteInferenceRoute` to `always` on that host. The application includes the same steps under **SSH**. See [Remote SSH](./docs/remote-ssh.md).

## Model configuration

![Model settings](./images/en-model-1.png)

| Provider endpoint | Protocol |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

Use the provider's documentation for the protocol, cache, pricing, and reasoning fields.

- **Server address:** A base address or a full request URL is accepted. The final path has to match the provider.
- **Model ID:** Use the identifier the API accepts. The display name only changes the label in Cursor.
- **Context and output limits:** Set these from the model's actual limits.
- **Custom headers and extra parameters:** JSON objects containing fields the provider accepts.
- **Tool calling:** A successful text test does not show that streaming tool calls work. Try a tool call separately.

## Data directory

Runtime data stays in the home directory when you replace the application:

| System | Directory |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

```text
.cursor-byok-v3/
├── cursor-byok.db   # models, settings, and runtime records
├── ca/              # local CA certificate and private key
├── plugins/         # installed plugins and plugin data
└── logs/            # desktop logs
```

Quit Cursor BYOK, including the tray process, before copying this directory. The database WAL can still hold data while the app is running. Backups can contain API keys and the CA private key. A standalone server can set its database with `CURSOR_DATABASE_URL`.

Check for updates in the application, or download a build from this repository's Releases. Run one application instance against a database.

## TAB and official accounts

In **Settings → TAB**:

| Mode | Behavior |
| --- | --- |
| Public service | Uses the shared TAB service. Availability depends on that service. |
| Direct | Uses the official TAB service for the current Cursor account. |
| Custom | Uses the TAB address you enter. |

TAB is separate from Agent model channels. An Agent API key does not add official TAB quota. **Auto** selects Cursor's models.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| The configured model is missing in Cursor | The model and local integration are enabled, Cursor was fully restarted, and the conversation is new. |
| The connection test passes, but Agent tools fail | The selected protocol supports streaming tool calls. Read the error in the call record. |
| `Cursor message is missing image` | Install [v1.0.19](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.19) or newer. Older builds stop when history contains a `Uint8Array` image. |
| `ConversationStateStructure.summary` / `ThirtyTwoBit` | Install [v1.0.18](https://github.com/Jasun0925/cursor-byok/releases/tag/v1.0.18) or newer. |
| 401 / 403 | Credentials, endpoint, model permission, or plugin login. |
| 429 / quota | Provider limits, or another model channel. |
| Data is missing after an upgrade | The same OS user and data directory, and the application copy you launched. |
| TAB fails while Agent works | TAB mode, service availability, and the official account quota. |

## Building from source

Install Rust stable, Node.js 22 with npm, Make, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform. On Windows, Make is available through Scoop or Chocolatey.

```bash
git clone https://github.com/Jasun0925/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci
make build-desktop
```

The first build compiles every dependency. It writes a local updater signing key to `.tauri/cursor-byok.local.key`. Git ignores that file.

Quit Cursor BYOK, including the tray, before replacing the installed app.

| Platform | Build output | Installation |
| --- | --- | --- |
| macOS | `target/release/bundle/macos/Cursor BYOK.app` | Drag it into Applications in Finder. |
| Windows | `target/release/bundle/nsis/*-setup.exe` | Run the installer. |
| Linux | `deb`, `rpm`, and `appimage` under `target/release/bundle/` | Install the package for your distribution. |

A macOS app built on the same machine opens directly. If another Mac reports the copied app as damaged, run `xattr -dr com.apple.quarantine "/Applications/Cursor BYOK.app"`. macOS App Management can block a terminal from replacing `/Applications`; use Finder. An in-app update installs the GitHub Release over a locally built app. Replacing the application leaves `~/.cursor-byok-v3/` in place.

```bash
make dev-web         # local server and web UI
make dev-desktop     # Tauri desktop development
make check           # formatting, Clippy, tests, and frontend checks
make build-server    # standalone Rust server
make build-docker    # Docker image
```

Development instances use local data. Keep them separate from the installed application. `make build-desktop` signs with the local development key. Published packages come from the release workflow.

## Layout

```text
Cursor Agent requests and tool results
    ↓
Local integration and request compilation → Agent runtime ↔ SQLite
    ↓                                          ↑
Provider adapter → model API → text and tool requests
                                    ↓
                              Cursor runs tools
                                    ↓
                              results continue the turn
```

```text
cursor-byok/
├── apps/desktop/        # React UI and Tauri lifecycle
├── server/              # gateway, Cursor protocol, providers, SQLite
├── crates/semble-core/  # local code indexing and search
├── protocols/cursor/    # Cursor protobuf sources
└── .github/workflows/   # CI and desktop releases
```

## Feedback

Report problems in [Issues](https://github.com/Jasun0925/cursor-byok/issues). Include the operating system, Cursor version, BYOK version, protocol, steps, and a redacted error. Leave out credentials and private conversations.

Thanks to [leookun](https://github.com/leookun) and the [upstream contributors](https://github.com/leookun/cursor-byok/graphs/contributors). Distributed under the [MIT License](./LICENSE).
