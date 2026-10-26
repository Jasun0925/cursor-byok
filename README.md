<div align="center">

# Cursor BYOK · Ad-free Fork

Use your own model APIs in Cursor with Agent, tool calling, Skills, and MCP workflows.

[中文说明](./README-CN.md) · [Download](https://github.com/haohaomin/cursor-byok/releases/latest) · [Releases](https://github.com/haohaomin/cursor-byok/releases) · [Issues](https://github.com/haohaomin/cursor-byok/issues) · [Fix status](./docs/maintenance-status.md) · [Pull Requests](https://github.com/haohaomin/cursor-byok/pulls) · [Upstream](https://github.com/leookun/cursor-byok)

[![Release](https://img.shields.io/github/v/release/haohaomin/cursor-byok?style=flat-square)](https://github.com/haohaomin/cursor-byok/releases/latest)
[![CI](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square)](https://github.com/haohaomin/cursor-byok/releases/latest)

</div>

![Cursor BYOK dashboard](./images/en-home-1.png)

## About this fork

This is a personal fork of [leookun/cursor-byok](https://github.com/leookun/cursor-byok), maintained by **haohaomin**. It runs a local gateway that translates Cursor Agent requests into requests to your configured model APIs.

This fork removes advertising components and uses its own GitHub Releases, update source, and updater signing identity. Public, direct, and custom TAB service options remain available. Upstream changes are integrated selectively after review and validation.

The `main` branch, experimental branches, and released installers may contain different changes. Install from this repository's [Latest Release](https://github.com/haohaomin/cursor-byok/releases/latest) and consult that version's release notes for its contents.

> Cursor BYOK is free and open source; your model provider may charge for API usage. This project is independent of Cursor. A local gateway still sends requests to the model service you choose; it does not imply local model inference.

## Maintenance after upstream archival

The original repository is archived. This independent fork continues reviewing its open issues and pull requests, with credited sources and reproducible tests. See the [fix status and remaining work](./docs/maintenance-status.md), including which changes are released and which exist only in source. The 2026-10-27 inventory covers 104 open issues and 51 open pull requests; collection does not mean every report is fixed.

Report reproducible problems in [this fork’s Issues](https://github.com/haohaomin/cursor-byok/issues), linking the original report when applicable. Archived upstream discussions cannot receive new comments.

Using Agent over Remote SSH? See the [per-host routing guide and verified limits](./docs/remote-ssh.md). Basic Agent requests can route through local BYOK. WebFetch paging is fixed and verified in source; v1.0.17 does not include that fix.

## Features

- **Your own model services:** Configure endpoints, API keys, model IDs, context limits, and generation parameters.
- **Three API protocols:** OpenAI Responses, OpenAI Chat Completions, and Anthropic Messages.
- **Model management and testing:** Add, duplicate, reorder, and batch-test configurations; inspect time to first token and generation speed.
- **Agent workflows:** Tool calls, subagents, Skills, MCP, multi-turn conversations, and context compaction, subject to model and API compatibility.
- **Call records:** Inspect request status, token usage, cache metrics, and errors.
- **Plugins and external API:** Manage built-in plugins and optionally expose model endpoints to other local applications through the external API settings.
- **Independent TAB settings:** Choose public, official direct, or custom services.
- **Desktop support:** macOS, Windows, and Linux.

### Fixes released in this fork

| Release | Changes |
| --- | --- |
| [v1.0.4](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.4) | Retain subagent execution after Steer and deliver completed results to the parent conversation. |
| [v1.0.5](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.5) | Prevent tool execution and automatic retries after an explicit Anthropic refusal; mark the error non-retryable in Cursor. |
| [v1.0.6](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.6) | Reconcile Responses text per output item and avoid duplicates in the covered cases; order tool completion before trailing narration so successful tools do not incorrectly appear as Skipped. |
| [v1.0.7](https://github.com/haohaomin/cursor-byok/releases/tag/v1.0.7) | Recover text and missing suffixes supplied only in the final Responses snapshot without repeating content already displayed. |

See [all releases](https://github.com/haohaomin/cursor-byok/releases) for subsequent changes. Fixes on experimental branches are not automatically included in released builds.

## Download and installation

Open the [Latest Release](https://github.com/haohaomin/cursor-byok/releases/latest) and choose an **Asset** matching your operating system and processor.

| Platform | Package |
| --- | --- |
| macOS · Apple Silicon | `aarch64.app.tar.gz`; extract it and move the application to Applications. |
| macOS · Intel | `x64.app.tar.gz`; extract it and move the application to Applications. |
| Windows · x64 | `x64-setup.exe` installer, or `windows-amd64.zip` for the portable build. |
| Linux · x64 | `.AppImage`, `.deb`, or `.rpm` as appropriate for your distribution; AppImage requires execute permission. |

Files ending in `.sig` are updater signatures. `latest.json`, `portable-latest.json`, and `update.json` are update manifests, not installers.

## Quick start

1. Install and open Cursor BYOK. Have your provider's endpoint, API key, and model ID ready.
2. Follow the in-app Cursor configuration prompts to initialize the local CA, establish certificate trust, and configure the local connection. Complete any required system authorization.
3. Add a model in **Model Settings**, choose a supported protocol, save, and run **Test**.
4. Enable Cursor's local integration as instructed by the application, confirm the connection status, and keep Cursor BYOK running.
5. Quit Cursor completely, restart it, and create a new conversation. Select your configured model explicitly; do not select **Auto** for a BYOK model.
6. Try a simple conversation, then a file read or another tool call whose result you can verify.

If integration stops working after a Cursor update, revisit the in-app Cursor configuration and restart Cursor completely. The [upstream user guide](https://docs.leokun.cn) covers general setup; use this fork's download and update links, and see the support section below for feedback.

## Model configuration

![Model settings](./images/en-model-1.png)

| Provider endpoint | Protocol to select |
| --- | --- |
| `/v1/responses` | OpenAI → Responses API |
| `/v1/chat/completions` | OpenAI → Chat Completions API |
| `/v1/messages` | Anthropic → Messages API |

Follow your provider's documentation rather than inferring protocol support from the model name. Caching, pricing, and reasoning parameters also depend on the service; choosing one protocol does not guarantee them.

- **Server address:** A base address or complete request URL is supported. Verify that the resulting path matches your provider's endpoint.
- **Model ID:** Use the identifier accepted by the API. The display name only changes how the model appears in Cursor.
- **Context and output limits:** Set these according to the actual model's capabilities.
- **Custom headers / extra parameters:** Supply JSON objects containing fields supported by your provider.
- **Tool calling:** Test it separately after the connection test; successful text generation alone does not establish tool compatibility.

## Upgrading and preserving data

This fork's installers use an independent update source. Check for updates in the application or download a new build from this repository's Releases. Installing the original upstream application does not automatically subscribe it to this fork's updates.

The desktop application stores runtime data in your home directory, separately from the application installation:

| System | Default data directory |
| --- | --- |
| macOS / Linux | `~/.cursor-byok-v3/` |
| Windows | `%USERPROFILE%\.cursor-byok-v3\` |

```text
.cursor-byok-v3/
├── cursor-byok.db   # SQLite models, settings, and runtime records
├── ca/              # Local CA certificate and private key
├── plugins/         # Installed plugins, plugin data, and runtime
└── logs/            # Desktop runtime logs
```

**To switch an existing v3 installation to this fork:**

1. Quit Cursor BYOK completely, including any background tray instance, and back up the entire data directory.
2. Install or replace the application with this fork's build. Keep the existing data directory and avoid uninstall options that erase application data.
3. Launch it as the same OS user. Check your models, settings, records, and Cursor integration status.
4. Run only one application instance against the database. To roll back, quit the application first and restore the matching pre-upgrade backup so an older binary does not read a newer database.

Stop the application before making a backup to avoid copying only the SQLite main file while missing data still in its WAL. Backups may contain API keys, plugin credentials, and a CA private key; keep them private. These paths describe the default desktop setup; a standalone server can override its database through `CURSOR_DATABASE_URL`. Do not assume that older data formats are compatible merely by copying their files.

## TAB and official accounts

Choose a mode in **Settings → TAB Settings**:

| Mode | Behavior |
| --- | --- |
| Public service | Retains the upstream public TAB service option; availability depends on its operator. |
| Direct | Uses the official TAB service for your current Cursor account, subject to its entitlements and quota. |
| Custom | Uses the TAB service address you supply. |

Removing advertising does not remove the public TAB option. TAB is separate from Agent model channels; an Agent API key does not provide an official TAB allowance.

You can use your own official Cursor account. **Auto selects official models, not your BYOK models.** Official features remain subject to account permissions and Cursor version support.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Configured model missing in Cursor | Confirm the model and local integration are enabled, quit Cursor completely, and start a new conversation. |
| Connection test passes but Agent tools fail | Check streaming tool-call support for the selected protocol and inspect the call record's error. |
| 401 / 403 | Check credentials, endpoint, model permissions, or plugin login status. |
| 429 / quota exhausted | Inspect provider limits; wait for recovery or choose another model channel. |
| Existing data missing after an upgrade | Check the OS user, data directory, and application copy before clearing any database. |
| TAB fails while Agent works | Check TAB mode, service availability, and official account quota separately. |

## Runtime and repository structure

```text
Cursor Agent requests / tool results
    ↓
Local integration and request compilation → Agent runtime ↔ SQLite state
    ↓                                          ↑
Provider adapter → Model API → Text / tool requests
                                    ↓
                              Cursor executes tools
                                    ↓
                              Results continue the turn
```

```text
cursor-byok/
├── apps/desktop/       # React UI and Tauri desktop lifecycle
├── server/
│   ├── src/
│   │   ├── cursor/    # Cursor protocol, request compilation, and presentation
│   │   ├── local_app/ # Local integration, proxy, and certificates
│   │   ├── run/       # Agent loop, tool rounds, and compaction
│   │   ├── provider/  # Model protocols and response adaptation
│   │   ├── store/     # SQLite persistence
│   │   ├── plugin/    # Plugin host and lifecycle
│   │   ├── api/       # HTTP and Connect routing
│   │   └── control/   # Management APIs used by the UI
│   ├── plugins/       # Built-in plugin implementations
│   ├── prompt/        # Prompts and runtime templates
│   └── migrations/    # Database schema changes
├── crates/semble-core/ # Local code indexing and search
├── protocols/cursor/  # Cursor protobuf sources
├── support/           # Protocol extraction, debugging, and benchmarks
└── .github/workflows/ # CI and desktop releases
```

## Local development

Install Rust stable, Node.js 22 with npm, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform. Make is required for the commands below; Docker is needed only for image builds.

```bash
git clone https://github.com/haohaomin/cursor-byok.git
cd cursor-byok
npm --prefix apps/desktop ci

make dev-web         # Local server and web management UI
make dev-desktop     # Tauri desktop development mode
```

Choose the development mode you need. Development instances also use local data; isolate testing from the installed application.

```bash
make check           # Rust formatting, Clippy, tests, and frontend checks
make build-web       # Build the frontend
make build-server    # Build the standalone Rust server
make build-desktop   # Build local desktop packages
make build-docker    # Build a Docker image
```

`make build-desktop` uses a local development signing key; it is not a release publication. The repository's release workflow produces official packages and updater artifacts for this fork.

## Support, contributing, and credits

Report problems in [this repository’s Issues](https://github.com/haohaomin/cursor-byok/issues) and submit code improvements through [pull requests](https://github.com/haohaomin/cursor-byok/pulls). Upstream is archived; reference its existing reports instead of trying to open a new upstream discussion. Include your OS, Cursor and BYOK versions, protocol, reproduction steps, and redacted errors. Do not attach credentials or complete private conversations.

Focused pull requests are welcome. Read [AGENTS.md](./AGENTS.md) and relevant directory instructions, add necessary regression coverage, and run `make check`. Keep general upstream fixes separate from this fork's release configuration and UI customizations.

Thanks to [leookun](https://github.com/leookun) and the [upstream contributors](https://github.com/leookun/cursor-byok/graphs/contributors). General documentation and community links are available from the [upstream repository](https://github.com/leookun/cursor-byok) and [project documentation](https://docs.leokun.cn). Screenshots are inherited from upstream; the installed version's UI may differ.

Distributed under the [MIT License](./LICENSE), retaining the original copyright notice.
