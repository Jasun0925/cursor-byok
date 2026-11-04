<div align="center">

# Cursor BYOK

**Use your own model APIs in Cursor, with the Agent workflow you already know.**

Free and open source · No ads · macOS / Windows / Linux

[中文说明](./README-CN.md) · [Download](https://github.com/haohaomin/cursor-byok/releases/latest) · [Quick start](#quick-start) · [Docs](docs/README.md) · [Report an issue](https://github.com/haohaomin/cursor-byok/issues)

[![Release](https://img.shields.io/github/v/release/haohaomin/cursor-byok?style=flat-square)](https://github.com/haohaomin/cursor-byok/releases/latest)
[![CI](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)

</div>

> **This repository is maintained by [haohaomin](https://github.com/haohaomin).** The original [leookun/cursor-byok](https://github.com/leookun/cursor-byok) project is no longer maintained and has been archived. I continue development, bug fixes, and releases here. Please use this repository for downloads, updates, issues, and pull requests. Thanks to the original author and all contributors.

![Cursor BYOK dashboard](./images/en-home-1.png)

## What it does

Cursor BYOK runs a local gateway that connects Cursor Agent requests to your configured model services. Manage your own models, inspect call records, and continue using conversations, file operations, and tools inside Cursor.

| Capability | Details |
| --- | --- |
| Your choice of model service | Configure endpoints, API keys, model IDs, context limits, and generation parameters. |
| Three API protocols | OpenAI Responses, OpenAI Chat Completions, and Anthropic Messages. |
| Agent workflows | Tool calls, subagents, Skills, MCP, multi-turn conversations, and context compaction. Compatibility depends on the model, API, and Cursor version. |
| Model management and diagnostics | Add, duplicate, reorder, and batch-test models; inspect first-token latency, generation speed, token usage, cache metrics, and errors. |
| Plugins and external API | Manage built-in plugins and expose model endpoints to other local applications. |
| Independent TAB settings | Public, official direct, and custom service options remain available. |

The application is free and open source; your model provider may charge for API usage. Requests go to the model service you select. A local gateway does not imply local model inference.

## Download and install

**[Download the latest release](https://github.com/haohaomin/cursor-byok/releases/latest)** and choose an **Asset** for your operating system and processor:

| Platform | Filename pattern | Installation |
| --- | --- | --- |
| macOS · Apple Silicon | `*_aarch64.app.tar.gz` | Extract and move the application to Applications. |
| macOS · Intel | `*_x64.app.tar.gz` | Extract and move the application to Applications. |
| Windows · x64 | `*_x64-setup.exe` | Run the installer; use `*-windows-amd64.zip` for the portable build. |
| Linux · x64 | `*_amd64.AppImage` / `*_amd64.deb` / `*.x86_64.rpm` | Choose for your distribution; AppImage needs execute permission. |

For a first installation, choose a desktop package from the table. `.sig` files are updater signatures. `latest.json`, `portable-latest.json`, and `update.json` are update manifests, not installers.

Installers use this repository's update source and signing identity. **Moving from an upstream build? Read [Upgrading and preserving data](#upgrading-and-preserving-data) first.** Upstream builds do not automatically switch to this repository's updates.

## Quick start

Have your model service's **endpoint, API key, and model ID** ready, then:

1. **Open Cursor BYOK.** Follow the in-app prompts for Cursor integration, local CA initialization, and certificate trust. Complete system authorization when prompted.
2. **Add a model.** Enter the service details in Model Settings, select the matching protocol, save, and run the connection test.
3. **Confirm the connection.** Enable Cursor's local integration as instructed and keep Cursor BYOK running.
4. **Restart Cursor and select the model.** Quit completely, reopen Cursor, start a new conversation, and select your configured model. **Auto uses official models; it does not select your BYOK models.**
5. **Verify a complete operation.** Send a simple message, then ask Agent to read a file. Confirm both text and tool results arrive correctly.

If integration stops working after a Cursor update, revisit the in-app configuration and restart Cursor completely. For remote development, continue with the [Remote SSH guide](docs/remote-ssh.md).

## Model configuration

Choose the protocol supported by your provider's endpoint, rather than inferring it from the model name:

| Provider endpoint | Protocol to select |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

The server address can be a base address or a complete request URL. Use the model ID accepted by the API; the display name only changes its label in Cursor. Set context limits, maximum output, custom headers, and extra parameters according to the provider's documentation. Headers and extra parameters must be JSON objects.

After the connection test, verify streaming tool calls too. Cache support, reasoning parameters, and pricing depend on the model service.

<details>
<summary>View the model settings interface</summary>

![Model settings](./images/en-model-1.png)

</details>

## Upgrading and preserving data

Check for updates in the application or download a build from this repository's [Releases](https://github.com/haohaomin/cursor-byok/releases). Desktop data lives in your home directory, separately from the application:

| System | Default data directory |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

**When updating or switching an existing v3 installation:**

1. Quit Cursor BYOK completely, including its background tray instance, and back up the entire data directory.
2. Install this repository's build. Keep the data directory and avoid uninstall options that erase application data.
3. Launch as the same OS user and check your models, settings, records, and Cursor integration. Run only one instance against the database.

Stop the application before backing up the entire directory so SQLite WAL data is included. Backups may contain API keys, plugin credentials, and a CA private key; keep them private. Use the matching pre-upgrade backup when rolling back. Compatibility with older data formats needs separate verification.

<details>
<summary>What the data directory contains</summary>

```text
.cursor-byok-v3/
├── cursor-byok.db   # Models, settings, and runtime records
├── ca/             # Local CA certificate and private key
├── plugins/        # Plugins, plugin data, and runtime
└── logs/           # Desktop runtime logs
```

These are the desktop defaults. A standalone server can override its database through `CURSOR_DATABASE_URL`.

</details>

## TAB and remote development

In **Settings → TAB Settings**, choose a public service, official direct connection, or custom service address. Public service availability depends on its operator; official direct access depends on your Cursor account's entitlements and quota. TAB is separate from Agent model channels: an Agent API key does not provide an official TAB allowance.

**Basic Agent use over Remote SSH has been verified in a real session.** Configure request routing per remote host using the [Remote SSH guide](docs/remote-ssh.md). Remote WebFetch, Semble, and approval handling have different implementation and validation statuses; see the [maintenance status](docs/maintenance-status.md) for their current scope.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Configured model missing in Cursor | Confirm the model and local integration are enabled, quit Cursor completely, and start a new conversation. |
| Connection test passes but Agent tools fail | Check streaming tool-call support for the selected protocol and inspect the call record's error. |
| 401 / 403 | Check credentials, endpoint, model permissions, or plugin login status. |
| 429 / quota exhausted | Check provider limits; wait for recovery or choose another model channel. |
| Background command finishes but the conversation stalls | Some cases still await validation. Ask Agent to read the existing terminal output and continue without rerunning the command; see [maintenance status](docs/maintenance-status.md). |
| Existing data missing after an upgrade | Check the OS user, data directory, and application copy before clearing any database. |
| TAB fails while Agent works | Check TAB mode, service availability, and account quota separately. |

## Maintenance and documentation

I continue investigating existing issues, validating community patches, and publishing updates. **Released, merged, in development, and awaiting validation are tracked separately.** A source or development-branch fix may not be in an installer yet. Consult each release's notes for its contents.

| What you need | Where to look |
| --- | --- |
| Changes in published builds | [Release notes](https://github.com/haohaomin/cursor-byok/releases) |
| Fixes and known limitations | [Maintenance status](docs/maintenance-status.md) |
| Completed work, remaining work, and development branches | [Work status](docs/work-status.md) |
| Issue and PR investigations with validation evidence | [Documentation index](docs/README.md) · [Issue checklist](docs/issues.md) · [PR checklist](docs/pull-requests.md) |

The detailed maintenance documents are currently in Chinese.

## Local development

Use Rust stable, Node.js 22, npm, Make, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
git clone https://github.com/haohaomin/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci
make dev-desktop
```

Alternatively, `make dev-web` starts the local server and web management interface. Isolate development instances from production data.

| Command | Purpose |
| --- | --- |
| `make check` | Rust formatting, Clippy, tests, and frontend checks. |
| `make build-web` | Build the frontend. |
| `make build-server` | Build the standalone Rust server. |
| `make build-desktop` | Build desktop packages with a local development signing key. |
| `make build-docker` | Build a Docker image; requires Docker. |

<details>
<summary>Repository structure and request flow</summary>

```text
cursor-byok/
├── apps/desktop/       # React UI and Tauri desktop lifecycle
├── server/            # Local gateway and Agent runtime
│   ├── src/cursor/    # Cursor protocol, request compilation, and presentation
│   ├── src/local_app/ # Local integration, proxy, and certificates
│   ├── src/run/       # Agent loop, tool rounds, and compaction
│   ├── src/provider/  # Model protocols and response adaptation
│   ├── src/store/     # SQLite persistence
│   ├── src/plugin/    # Plugin host and lifecycle
│   ├── plugins/       # Built-in plugins
│   ├── prompt/        # Prompts and runtime templates
│   └── migrations/    # Database schema changes
├── crates/semble-core/ # Code indexing and search
├── protocols/cursor/  # Cursor protobuf sources
├── docs/              # Guides, maintenance status, and historical evidence
├── support/           # Protocol extraction, debugging, and benchmarks
└── .github/workflows/ # CI and desktop releases
```

```text
Cursor → Local integration / compilation → Agent runtime ↔ SQLite session state
                                                ↕
                                        Provider adapter ↔ Model API
                                                ↓
                                    Text to Cursor / tool requests
                                                ↓
                                    Cursor executes tools → results to runtime
                                                            until done or cancelled
```

Extend model protocols in `server/src/provider/`. The host in `server/src/plugin/` manages plugins. The repository's release workflow produces published packages and update artifacts.

</details>

## Support and contributing

Report problems in [this repository's Issues](https://github.com/haohaomin/cursor-byok/issues) or submit improvements through [Pull Requests](https://github.com/haohaomin/cursor-byok/pulls). Include your **OS, Cursor/BYOK versions, model protocol, reproduction steps, and redacted errors**. Link an existing upstream report when relevant.

Read [AGENTS.md](./AGENTS.md) before contributing code. Keep changes focused, add regression coverage where needed, and run the relevant checks. For new features or substantial changes, open an issue to discuss the approach first. Do not publish credentials or private conversations.

## Credits and license

This independently maintained repository is based on [leookun/cursor-byok](https://github.com/leookun/cursor-byok). Thanks to [leookun](https://github.com/leookun) and the [original contributors](https://github.com/leookun/cursor-byok/graphs/contributors). Screenshots come from the original project; the installed version's interface may differ. Semble provenance is documented in [UPSTREAM.md](./UPSTREAM.md).

Distributed under the [MIT License](./LICENSE), retaining the original copyright notice. This project is not affiliated with Cursor.
