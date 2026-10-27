# Remote SSH 中使用 BYOK Agent

Cursor 3.21.16 起提供远程 Agent 推理路由选项。若本地 Agent 正常、SSH 窗口报 `Failed to establish a socket connection to proxies: PROXY 127.0.0.1:端口`，先让远程 Agent 的模型请求经过本机 Cursor。[Cursor 官方说明](https://forum.cursor.com/t/remote-ssh-agent-stuck-on-reconnecting-after-cursor-agent-host-rollout/172620/8)

1. 在本机启动 BYOK，开启 Cursor 接管，确认一个 BYOK 模型能正常使用。
2. 使用 Cursor 3.21.16 或更新版本连接 SSH 主机。
3. 打开**编辑器设置**，选择 **远程 [SSH: 主机]**，搜索 `cursorAgentHost.remoteInferenceRoute`，将值设为 `always`。若进入的是 Cursor 自身设置页，先点击 **Editor Settings → Open**。
4. 执行 **Developer: Reload Window**。
5. 新建 Agent 对话，明确选择带供应商标记的 BYOK 模型，读取一个远端临时文件并运行一个无副作用的命令验证。

也可在该主机的远程 `settings.json` 中合入以下配置，保留已有其他设置：

```json
{
  "cursorAgentHost.remoteInferenceRoute": "always"
}
```

```text
远程 Agent ──模型请求──→ 本机 Cursor → 本机 BYOK → 已配置的模型服务
     │                     ↑                │
     └──远程文件/终端工具    └────响应─────────┘
```

这个设置需要在远程窗口重载后生效。每个 SSH 主机独立配置；本机接管状态不能用来判断远端是否已配置。Cursor 的 Private Inference 远端会忽略 `always`，不在本次验证范围内。无需为此公开 BYOK 监听端口或关闭证书校验；若出现证书错误，应记录报错进程和完整错误，另行定位。

2026-10-27 使用 Cursor 3.27.14、BYOK v1.0.17、macOS → Linux、OpenAI Chat 模型实测：`auto` 出现上述代理连接错误；改为 `always` 后，流式回复、Read、Shell、文件写入、续聊、取消后继续、窗口重载后新对话通过。[详细证据与限制](upstream-issue-reviews/2026-10-27-issue469-check.md)

**源码中的 WebFetch 分页已通过真实 SSH 验收。** 长网页通过同一 WebFetch 工具逐页返回，同会话续读及跨会话隔离也通过。[修复与验收证据](upstream-issue-reviews/2026-10-27-issue469-webfetch-fix.md)。**v1.0.17 安装包仍使用旧缓存引用，不包含此修复。**

**远程 Semble 仓库索引仍待修复。** 当前源码在 BYOK 本机读取仓库路径，远端独有路径已复现 `source does not exist`；候选快照补丁存在启动、忽略规则和索引来源问题，尚未采用。[定位与候选审查](upstream-issue-reviews/2026-10-27-issue469-semble-check.md)。共享索引层的 `.cursorignore` 和热缓存已[独立修复](upstream-issue-reviews/2026-10-27-semble-cursorignore-fix.md)，但未实现远端采集；[真实原生工具实验](upstream-issue-reviews/2026-10-27-issue469-native-files-check.md)还发现 Grep 会跟随内部符号链接；后续 [Ls 实测](upstream-issue-reviews/2026-10-27-issue469-inventory-check.md)确认清单会截断且排除项不区分大小写。元数据探针已在 Linux 验证，完整过滤与 Cursor 采集尚未接通。

MCP、Skills、Tab、提交信息和其他模型协议尚未完成本轮远程验收，不能据此宣称所有远程能力均已支持。

源码中的 Cursor 页面新增 **SSH** 指南按钮，使用相同配置步骤；该界面尚未发布安装包。v1.0.17 用户现在即可在 Cursor 中手动设置并按上述范围复测。
