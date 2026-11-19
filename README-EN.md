<div align="center">

# Cursor BYOK

**Use your own model APIs in Cursor, with the Agent workflow you already know.**

Free and open source · No ads · macOS / Windows / Linux

[中文说明](./README.md) · [Русский](./README-RU.md) · [Download](https://github.com/haohaomin/cursor-byok/releases/latest) · [Quick start](#quick-start) · [Maintenance](#ongoing-maintenance-and-upstream-work) · [Docs](docs/README.md) · [Disclaimer](#disclaimer) · [Report an issue](https://github.com/haohaomin/cursor-byok/issues)

[![Release](https://img.shields.io/github/v/release/haohaomin/cursor-byok?style=flat-square)](https://github.com/haohaomin/cursor-byok/releases/latest)
[![CI](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml/badge.svg)](https://github.com/haohaomin/cursor-byok/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)

</div>

> [!IMPORTANT]
> **Upstream maintenance has stopped; this repository is personally maintained by [haohaomin](https://github.com/haohaomin).** The original [leookun/cursor-byok](https://github.com/leookun/cursor-byok) project is archived. I am continuing to investigate and fix unresolved upstream issues, evaluate unmerged pull requests, and maintain future releases. Please use this repository for downloads, updates, issues, and contributions. Thanks to the original author and all contributors.

## Ongoing maintenance and upstream work

**My current focus is resolving upstream issues and bringing validated community improvements into this fork.** I prioritize work by impact, reproducibility, and the time I can contribute, with public records of completed fixes, open problems, and validation limits.

| Maintenance area | Approach and progress |
| --- | --- |
| Upstream issues | Reproduce, investigate, fix, and run regression checks. Reports without enough evidence or pending real-world validation remain open in the checklist. See the [issue checklist](docs/issues.md). |
| Upstream pull requests | Review suitability and risk, then adopt, adapt, complete, or reimplement changes while retaining contributor attribution and validation records. A listed PR is not necessarily merged. See the [PR checklist](docs/pull-requests.md). |
| Releases and compatibility | Fix problems in this fork, follow Cursor changes, and publish updates. Source merges, automated checks, real-world validation, and packaged releases are tracked separately. See [maintenance status](docs/maintenance-status.md) and [release notes](https://github.com/haohaomin/cursor-byok/releases). |

Work so far covers Responses output, subagent execution and result delivery, SQLite, OAuth renewal, and Remote SSH. Background tasks, real accounts, and client approvals still have unresolved or unverified cases. **This does not mean every upstream issue is fixed or every PR will be merged.** See [work status](docs/work-status.md) for completed and remaining work, and the [documentation index](docs/README.md) for supporting records. Detailed maintenance records are currently in Chinese.

**Source fixes may not be included in an installer yet. Check the release notes for the version you download.**

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
| Linux · x64 | `*_amd64.AppImage` / `*_amd64.deb` / `*.x86_64.rpm` | Choose for your distribution; AppImage needs execute permission. See the [Linux installation guide](docs/linux.md) for dependencies and compatibility boundaries. |

For a first installation, choose a desktop package from the table. `.sig` files are updater signatures. `latest.json`, `portable-latest.json`, and `update.json` are update manifests, not installers.

Installers use this repository's update source and signing identity. **Moving from an upstream build? Read [Upgrading and preserving data](#upgrading-and-preserving-data) first.** Upstream builds do not automatically switch to this repository's updates.

## Quick start

Have your model service's **endpoint, API key, and model ID** ready, then:

1. **Open Cursor BYOK.** Follow the in-app prompts for Cursor integration, local CA initialization, and certificate trust. If your managed device cannot install certificates, read the [CA and certificate guide](docs/certificates.md) first. Complete system authorization when prompted.
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

Report problems in [this repository's Issues](https://github.com/haohaomin/cursor-byok/issues) or submit improvements through [Pull Requests](https://github.com/haohaomin/cursor-byok/pulls). Include your **OS, Cursor/BYOK versions, model protocol, reproduction steps, and redacted errors**. For an existing upstream issue or PR, check the maintenance lists and include its original link. Reproduction cases, redacted logs, and validation on other platforms are welcome.

Read [AGENTS.md](./AGENTS.md) before contributing code. Keep changes focused, add regression coverage where needed, and run the relevant checks. For new features or substantial changes, open an issue to discuss the approach first. Do not publish credentials or private conversations.

## Disclaimer

- **Independent project:** This is a personally maintained community fork. It is not affiliated with, partnered with, or endorsed by Cursor or model service providers, and does not claim to be an official successor designated by the original author. Names and trademarks belong to their respective owners.
- **Compatibility and maintenance:** Availability depends on Cursor versions, model capabilities, API protocols, and third-party services. The maintainer does not guarantee ongoing compatibility with every feature, account, or environment, or a deadline for fixing each issue. Passing tests establish only the documented validation scope.
- **Accounts, data, and costs:** Use only accounts, credentials, and services you are authorized to access, and follow applicable service terms and laws. You are responsible for choosing and assessing providers, securing credentials, backing up important data, reviewing Agent file operations and commands, and paying for the services you use. A local gateway does not mean offline processing; do not send sensitive content to a service that is not authorized to receive it.
- **No warranty and limited liability:** The software is provided “as is” under the [MIT License](./LICENSE), without express or implied warranties. To the extent permitted by applicable law, the authors and copyright holders are not liable for claims, damages, or other liability arising from use of the software. The license text governs the full permission, warranty, and liability terms.

## Credits and license

This independently maintained repository is based on [leookun/cursor-byok](https://github.com/leookun/cursor-byok). Thanks to [leookun](https://github.com/leookun) and the [original contributors](https://github.com/leookun/cursor-byok/graphs/contributors). Screenshots come from the original project; the installed version's interface may differ. Semble provenance is documented in [UPSTREAM.md](./UPSTREAM.md).

Distributed under the [MIT License](./LICENSE), retaining the original copyright notice and contributor attribution.
