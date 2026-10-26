# Issue #469 / PR #498：WebFetch 远程分页修复与实机验收

**WebFetch 的本机缓存引用问题已在源码修复，并通过真实 Cursor Remote SSH 验收。** 长网页经 WebFetch 分页返回，同会话可以续读，其他会话不能读取该缓存。此结论仅覆盖 WebFetch；#469 的其他远程能力仍按清单逐项验证。**v1.0.17 安装包不含此补丁，本轮没有发布新版。**

## 来源与失败基线

- 来源：[sunrisegm 的 PR #498](https://github.com/leookun/cursor-byok/pull/498)，固定 head `c7114878c33076340c0c2f1e77ce4e7512d1960e`。本轮重新查询仍为 OPEN、未合并；上游仓库仍归档只读。
- 本 fork 源码基线：`45744732700020e09a9686457e9fb1a3e62d0d32`。此前真实 Cursor 3.27.14 + v1.0.17 已出现模型读取本机缓存失败；同一个缓存 URL 本机 HTTP 200，Linux 远端连接拒绝。[原始对照](2026-10-27-issue469-check.md)
- 生产改动前新增 `web_fetch_remote_client_needs_no_host_local_reference`，旧实现因模型结果包含 `127.0.0.1` 失败（1 failed，退出 101）。修复后该用例通过，并检查 Cursor 卡片没有本机 `output_location`。
- 选择性移植原 PR 的缓存、分页参数、原生审批衔接和基础测试；保留作者贡献。没有整体合入 PR，没有采用 Semble 远程快照及其代理/设置改动。

## 最终执行路径

```text
server/
├── prompt/cursor/tools.json                         # WebFetch 的 URL、content_id、offset、limit 契约
├── src/search/
│   ├── fetch.rs                                    # 公网抓取与已批准缓存读取；保留 URL/SSRF 校验
│   └── cache.rs                                    # 会话归属、持久化、UTF-8 分页、容量与到期清理
├── src/cursor/tools/
│   ├── runtime.rs                                  # 在待审批操作中保存真实会话 ID
│   ├── tool_call_dispatch/interaction.rs            # 参数检查 → Cursor 审批 → 抓取或缓存读取
│   ├── tool_call_dispatch/interaction/tests.rs      # HTTP/重定向、审批、隔离及缓存重建验证
│   └── tool_call_result/interaction.rs              # 同一有界内容进入模型结果和 Cursor 工具卡片
└── tests/
    ├── tool_round.rs                               # 非法分页参数成为工具错误，不终止模型运行
    └── prefix_stability.rs                         # 工具契约摘要及历史前缀回归
```

```text
远端 Cursor Agent 请求 WebFetch(url)
  → Cursor 原生 URL 审批
  → 本机 BYOK 抓取公网网页，保存 {真实会话 ID、最终 URL、正文}
  → 返回至多 16 KiB 正文 + content_id + 下一字节偏移
  → 模型再次请求 WebFetch(url, content_id, offset)
  → Cursor 原生审批 → 核验会话和 URL → 直接读缓存，不重新请求网页
  → 新工具结果追加到当前历史；早先结果保持不变
```

首次下载仍保留 5 MiB 网络响应上限；缓存正文单项最多 16 MiB，总正文最多 64 MiB、256 项、24 小时。页面边界不会切断 UTF-8 字符，最小 limit 为 4 字节。持久化记录原子写入，重建缓存时同时恢复正文与归属；清理过期、破损、未完成和旧 UUID `.txt` 文件。

删除旧 `/web-cache` 下载路由、服务地址注入和本机路径返回；没有保留双重访问方式。未改写已存在的对话结果，旧对话中已经返回的旧链接不会自动变成新缓存 ID，需要重新 WebFetch 原 URL。

相对候选补丁的调整：移除可选的无缓存返回分支、TransportRegistry 的重复缓存字段和未使用方法；不在增量工具渲染时严格解析完整参数，避免半截/非法参数导致整轮协议失败。正式调度仍在预留审批之前校验。工具定义只修改 WebFetch，原 Auto-review 字段、工具顺序及其他工具不变；相应模式的固定摘要经核对后更新。

## 自动化证据

| 检查 | 结果与覆盖范围 |
| --- | --- |
| WebFetch 定向测试 | 9 通过；参数与 schema、URL 匹配、原有公网限制、模型/UI 内容边界、审批与会话隔离 |
| 缓存定向测试 | 5 通过；多字节分页、缓存重建、所有者、偏移/limit、EOF、容量、过期与旧文件清理、超限写入不破坏旧内容 |
| tool_round | 22 通过；包括非法分页参数只返回工具错误且不遗留待审批操作 |
| 真实本地 HTTP 夹具 | 公网字面 IP 经隔离自定义代理返回 302 与 91,011 字节多字节正文；审批前零请求，下载及重定向共 2 请求；重建缓存实例后全部分页逐字节一致且不再联网 |
| `cargo test --locked --workspace --all-targets` | 464 通过、0 失败、0 忽略；包含前缀、检查点、压缩、中断、桌面及 Semble 回归 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过 |
| 格式与前端 | `cargo fmt --all -- --check`、`git diff --check`；指南文案同步中/英/葡翻译；`npm run check` 与 `npm run tauri:build -- --debug --no-bundle` 均通过 |

HTTP 夹具不是远程 UI 测试；缓存重建是新实例读取持久化文件，不冒充真实 Cursor 下的服务进程重启。Clippy 首轮发现的布尔表达式和测试模块位置问题已修正，没有关闭 lint。

## 真实 Cursor + SSH + 供应商

- Cursor 3.27.14；macOS → Linux SSH；远程主机临时设置 `cursorAgentHost.remoteInferenceRoute=always`。
- 从当前源码装配生产 ProviderRouter、TransportRegistry、Cursor HTTP router 和 CursorHarness。使用 SQLite backup API 创建数据库副本、独立监听端口及 `WebCache::at` 临时目录；只在副本开启详细请求记录。正式 v1.0.17 应用没有被替换。
- 明确选择带供应商标记的 gpt-6 模型，实际供应商协议为 OpenAI Chat，模型 ID 为 `gpt-6-astra`。没有 FakeProvider，没有合成客户端审批结果。
- 新的远端临时工作区含独有随机 marker；先以 Read 核对远端文件，再要求仅以 WebFetch 读完整公开 RFC 9457。用户提示未提供缓存 ID、分页偏移或预期尾部答案。

| 实机步骤 | 结果 |
| --- | --- |
| 首次全文读取 | 1 次远端 Read + 3 次 WebFetch；偏移依次为 0、16384、32768，最后一页 1335 字节；无需补发用户消息，回复 `WEBFETCH_REMOTE_ACCEPTED` |
| 内容校验 | 三页重建的 34,103 字节与独立下载并去除 UTF-8 BOM 的原文完全一致；原始传输文件为 34,106 字节。末尾非空行也一致 |
| 同会话新一轮续读 | 1 次带同一 content_id 的 WebFetch，返回末尾 1335 字节，回复 `CACHE_FOLLOWUP_OK`；没有重新下载原网页 |
| 独立新会话读取相同 ID | 工具仅返回缓存不可用错误，回复 `CACHE_ISOLATION_OK`；没有正文、没有重抓网页 |
| 供应商及历史 | 两个会话共 9 次调用全部 completed；所属会话 7 次请求逐一验证前一请求消息是后一请求的精确结构前缀，tools 完全一致 |

所有 WebFetch 正文都走工具返回，模型没有使用 Shell、Read 缓存 URL 或本机文件路径。实机使用 Cursor 当前审批配置，未专门触发手动等待/拒绝弹窗；拒绝与审批前取消仅有自动化覆盖。没有把源码验证写成安装包已经更新。

[结构化证据](2026-10-27-issue469-webfetch-evidence.json)记录调用 ID、各页摘要、内容哈希和验证边界，不含凭据或无关会话。原始本机日志、截图和临时装配源码位于 `/tmp/cursor-byok-webfetch-20261027-kMz3dk/`。

## 还原与未完成项

临时测试后端已退出，Cursor 本机 settings.json 按原始字节还原，正式代理仍监听、健康端点 HTTP 204；Cursor 已关闭远程连接并重载窗口。远端设置确认仅含本次配置后恢复为原先文件不存在状态，临时工作区通过 marker 和内容清单核验后删除。生产数据库、模型和 CA 没有修改。

本次解决 WebFetch 缓存的主机边界问题。真实服务重启后的会话恢复、证书错误、其他平台/模型协议、Semble、MCP、Skills、Tab 和提交生成仍需各自验收，#469 与 PR #498 不标记为整体完成。上游归档阻止评论投递，更新本 fork 的维护入口及回复草稿，未重复发送失败请求。
