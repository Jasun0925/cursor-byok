# Windows Cursor Agent 真实验收记录（2026-11-23）

## 结论

v1.2.1 在 Windows 普通用户环境下已完成 BYOK 应用启动、CA 初始化、用户级信任和 Cursor 接管。真实 Cursor Agent 请求仍未完成验收：Cursor 3.28.7 的账号功能门控处于关闭状态，云端接口持续返回 `401`，本地测试 provider 收到 0 个请求。

这次结果不能作为 BYOK 模型路由失败的证据，也不能标记为 Issue #155 或远程 Agent 已解决。阻断点发生在 Cursor Agent 启动请求之前。

## 测试范围

- BYOK：v1.2.1，发布包 SHA-256 为 `6f30223414923d8193fce39b3ca39555bec35d4030cc81ea262ebb4f2fd78dcd`。
- Cursor：3.28.7，Windows，普通用户 `renha`，交互会话 1。
- CA：生产 `CaManager` 生成的 `Cursor BYOK Local CA`；初始化和 `Ready` 状态已在独立 CA 记录中验证。
- 模型：仅监听 `127.0.0.1` 的临时 OpenAI 兼容 provider，响应标记为 `WINDOWS_CLIENT_ACCEPTED_20261123`。
- 数据隔离：真实 Cursor 原账号数据库未修改；Agent 复测另使用临时 `--user-data-dir`，仅复制本地测试状态和代理设置。

## 已通过的链路

```text
BYOK 启动
  -> 初始化生产 CA
  -> 显示用户级 certutil 信任命令
  -> 普通用户证书安装
  -> 刷新为 ca=ready
  -> 显示“Cursor 已接管”
```

设置文件在清理后恢复到验收前 SHA-256：
`9AFAF3C8ECAF87257174D6801265E7EAB0F28CD6A0F9CF5796FAE96C40FB5355`。

## Agent 阻断证据

Cursor 日志出现以下状态：

- `molten_agents_window is off for the signed-in account`；
- `cursor_agent_host gate is disabled`，有效拓扑为 `disabled`；
- `GetMe`、`GetPlanInfo`、`GetEffectiveUserPlugins` 等请求经 BYOK 代理返回 `401 Unauthorized`；
- `AvailableModels` 请求未在 Cursor 选择器中显示临时 BYOK 模型；
- 临时 provider 的事件文件保持 0 行，没有收到 Chat Completions、Responses 或 Agent Run 请求。

因此当前可以确定：Cursor 没有进入本地模型请求阶段。没有修改生产代码，也没有把这次环境阻断误记为模型协议或代理路由缺陷。

## 清理核验

- 用户级 CA 指纹已移除；LocalMachine 根证书中没有该临时证书。
- Cursor 原设置文件已恢复并与验收前哈希一致。
- 临时模型、provider、Cursor 隔离用户目录、BYOK 数据库、测试进程和计划任务已删除。
- 原 Cursor 账号数据库没有被替换、删除或写入测试令牌。

## 后续条件

要完成真实 Agent 验收，需要 Cursor 账号处于可用状态并允许 Agents/Agent Exec 功能；随后重复同一隔离 provider 测试，重点观察 `AvailableModels`、Agent Run 和 provider 入站事件。20 分钟长任务不属于本轮范围。
