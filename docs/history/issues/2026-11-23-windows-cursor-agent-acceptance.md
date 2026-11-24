# Windows Cursor Agent 真实验收记录（2026-11-23）

## 结论

v1.2.1 在 Windows 普通用户环境下已完成 BYOK 应用启动、CA 初始化、用户级信任和 Cursor 接管。真实 Cursor Agent 请求仍未完成验收，原因尚未确定。观察到账号接口 `401`、模型选择器未显示测试模型、provider 事件日志为空，但没有确认一次有效的界面提交，也没有在后续重试前重新确认 provider 存活。

这次结果不能证明 BYOK 模型路由正常或失败，也不能标记为 Issue #155 或远程 Agent 已解决。2026-11-24 复核更正：此前“账号功能门控阻断”“阻断点发生在请求之前”的结论证据不足；临时日志已清理，本次更正依据保留的执行记录，不能补齐完整请求链。

## 测试范围

- BYOK：v1.2.1，发布包 SHA-256 为 `6f30223414923d8193fce39b3ca39555bec35d4030cc81ea262ebb4f2fd78dcd`。
- Cursor：3.28.7，Windows，普通用户 `renha`，交互会话 1。
- CA：生产 `CaManager` 生成的 `Cursor BYOK Local CA`；初始化和 `Ready` 状态已在独立 CA 记录中验证。
- 模型：仅监听 `127.0.0.1` 的临时 OpenAI 兼容 provider，响应标记为 `WINDOWS_CLIENT_ACCEPTED_20261123`。
- 数据隔离：未向真实 Cursor 原账号数据库写入测试令牌，也未替换或删除该数据库；Agent 复测另使用临时 `--user-data-dir`，仅复制本地测试状态和代理设置。

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

## Agent 观察与证据边界

Cursor 日志出现以下状态：

- `molten_agents_window is off for the signed-in account`；
- `cursor_agent_host gate is disabled`，有效拓扑为 `disabled`；
- `GetMe`、`GetPlanInfo`、`GetEffectiveUserPlugins` 等请求经 BYOK 代理返回 `401 Unauthorized`；
- `AvailableModels` 请求未在 Cursor 选择器中显示临时 BYOK 模型；
- 临时 provider 的事件文件保持 0 行；该日志只记录 Chat Completions，不是完整入站追踪，不能据此判断 Responses 或 BYOK Agent Run 是否到达。

`cursor_agent_host` 关闭对应使用 legacy 路径，`molten_agents_window` 关闭对应 Agents 窗口功能；两者不能直接等同于 Agent 不可用。本项目也有禁用未支持本地 Agent 循环的配置，见 [PR #10 记录](../pull-requests/2026-11-04-pr10-check.md)。

界面输入和发送没有可靠确认；provider 启动任务还在后续重试期间退出（结果码 `3221225786`），重试前未重新检查健康状态。隔离账号令牌与生产生成结果也未做精确对照。因此目前不能区分账号认证、测试环境、界面提交和模型路由故障。生产代码未修改。

## 清理核验

- 用户级 CA 指纹已移除；LocalMachine 根证书中没有该临时证书。
- Cursor 原设置文件已恢复并与验收前哈希一致。
- 临时模型、provider、Cursor 隔离用户目录、BYOK 数据库、测试进程和计划任务已删除。
- 原 Cursor 账号数据库没有被替换、删除或写入测试令牌。

## 后续条件

后续先验证隔离 provider 存活、实际生效的路由与账号状态，再确认界面提示真正提交，同时记录 `AvailableModels`、BYOK Agent Run 入站与 provider 请求。根据断点决定是否修复，不能先把更换账号作为必要条件。20 分钟长任务不属于本轮范围。
