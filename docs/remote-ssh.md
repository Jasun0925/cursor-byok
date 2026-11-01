# Remote SSH 中使用 BYOK Agent

本地 Agent 正常、SSH 窗口报 `Failed to establish a socket connection to proxies: PROXY 127.0.0.1:端口` 时，让远程 Agent 的模型请求经过本机 Cursor。应用里的 **SSH** 按钮使用同样的步骤。[Cursor 官方说明](https://forum.cursor.com/t/remote-ssh-agent-stuck-on-reconnecting-after-cursor-agent-host-rollout/172620/8)

1. 在本机启动 Cursor BYOK，确认一个 BYOK 模型能正常对话。
2. 使用 Cursor 3.21.16 或更新版本连接 SSH 主机。
3. 打开编辑器设置，选择 **远程 [SSH: 主机]**，搜索 `cursorAgentHost.remoteInferenceRoute`，把值设为 `always`。如果进入的是 Cursor 自身设置页，先打开 **Editor Settings**。
4. 执行 **Developer: Reload Window**。
5. 新建 Agent 对话，选择 BYOK 模型，再读取一个远端文件并运行一条无副作用的命令。

也可以把下面的配置合并进该主机的远程 `settings.json`，保留已有设置：

```json
{
  "cursorAgentHost.remoteInferenceRoute": "always"
}
```

```text
远程 Agent ──模型请求──→ 本机 Cursor → 本机 BYOK → 已配置的模型服务
     │                     ↑                │
     └──远程文件和终端工具  └────响应─────────┘
```

每个 SSH 主机单独配置。重载远程窗口后才会生效。Cursor 的 Private Inference 远端会忽略 `always`。不需要为此公开 BYOK 端口或关闭证书校验。

本仓库 v1.0.18 及以后的发布包含 WebFetch 分页。远程仓库索引不在这套路由的验证范围内。
