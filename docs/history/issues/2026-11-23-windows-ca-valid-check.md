# Windows 有效 CA 与 HTTPS 验证

日期：2026-11-23（Asia/Shanghai）。基线：`0568285db1233d8ccaf5a9b452e89ebe208586eb`，CA 生产逻辑与 v1.2.1 相同。关联上游 PR #458、fork PR #43；接续 [存储生命周期初测](2026-11-21-windows-ca-check.md)。

## 当前结论

完整 CA 模块在 Windows 普通用户环境中通过五种状态对照：缺失 → Missing，过期 / 非 CA / 私钥不匹配 → Invalid，有效但未安装 → Untrusted。由生产 `CaManager::load()` 加载签发者后签出的 localhost 证书可通过显式 CA 的 HTTPS 验证；错误主机名被拒绝，Windows 默认 TLS 客户端在未信任时也拒绝握手。

**有效 CA 的安装、Ready 状态、Windows 系统信任 HTTPS 与移除闭环已通过。**用户从桌面手动运行后，certutil 退出码为 0；完整生产 CA 模块状态为 Untrusted → Ready → Untrusted。Windows 默认 TLS 客户端仅在安装期间成功连接，错误主机名始终被拒绝。整个测试以普通用户运行，未写入 LocalMachine Root。

真实 BYOK 安装包按钮、Tauri 终端启动与 Cursor 接管不在本次夹具覆盖内。没有发现需要新增生产补丁的证据。本轮只补充验收与修正文档。

## 测试结构与边界

```text
隔离 Rust 程序/                       # 编译并调用当前生产 CA 模块
├── main.rs                          # CLI、临时数据目录与 Error/CaState 适配
└── production_ca/                   # 当前仓库源码的原样副本
    ├── mod.rs                       # 文件、有效期、CA 约束、私钥匹配和状态
    └── windows.rs                   # Win32 CurrentUser Root 枚举与 DER 匹配
Windows 用户临时目录/                 # 与正式 BYOK 数据完全分离
├── cases/                           # 五组一次性证书夹具，仅在 Windows 生成私钥
├── https_probe.py                   # 仅监听 127.0.0.1 随机端口的 HTTPS 服务
└── acceptance.ps1                   # 普通用户对照、原生确认、信任验证与清理
```

Windows Rust 1.94.1 原生编译通过；测试为 Session 1、`admin=false`，不采用 SSH 的 Session 0 管理员身份作为普通用户证据。夹具 CA 为 RSA 3072，localhost 叶证书为 RSA 2048；生产 CA 模块未修改。自定义夹具名称、有效期及 CLI 适配不代表完整产品初始化/UI 已验收。

CA 名称：`Cursor BYOK isolated HTTPS acceptance 20261123`；SHA-1 指纹：`D3DB45BE3C51E9BCF0C77E90D8E4629BAF26F0EE`。有效期为 2026-11-23 13:26:55 至 2026-11-30 13:31:55 UTC。每次运行先检查当前有效期、指纹和两个 Root 存储均无该证书，避免覆盖已有信任。

## 已执行结果

| 检查 | 实际结果 | 能证明的范围 |
| --- | --- | --- |
| `CaManager::state()` 五种夹具 | 5/5 与预期一致 | 完整生产 CA 模块的校验及未安装状态 |
| .NET `SslStream.AuthenticateAsClient("localhost")`，未安装 CA | 证书验证失败 | Windows 默认 TLS 客户端没有意外信任隔离 CA |
| Python 默认验证上下文，显式指定隔离 CA | HTTPS 成功，响应随机标记一致；后续安装轮前置对照协商 TLS 1.3 | CA/叶证书签发、有效期、主机名和 HTTPS 服务可用；**不是系统信任已安装** |
| 同一显式 CA，错误主机名 | 验证错误 62：Hostname mismatch | 主机名校验仍然有效 |
| 远程启动普通用户 `certutil -user -addstore -f Root ...` | 180 秒未确认，测试超时退出；用户反馈窗口不可见 | 此次远程尝试未安装成功；后续手动成功结果见下表 |
| 超时后的独立复查 | CurrentUser/LocalMachine 均不存在本次指纹；无测试进程；四项临时计划任务已删除 | 本次远程尝试没有留下临时信任或后台服务 |

### 桌面手动运行后的闭环

用户通过桌面快捷方式运行同一隔离脚本。执行时间为 **2026-11-23 22:04:59–22:05:05（UTC+8）**，Session 1、`admin=false`。结果文件整体为 `passed`；没有用用户口头完成代替实际结果检查。

| 阶段 | CurrentUser / LocalMachine Root | `CaManager::state()` | Windows 默认 TLS 客户端 |
| --- | --- | --- | --- |
| 安装前 | 不存在 / 不存在 | Untrusted | localhost 证书验证失败 |
| 原生确认、certutil 退出码 0 后 | 存在 / 不存在 | Ready | localhost HTTPS 成功，响应随机标记一致；错误主机名验证失败 |
| 按本次精确指纹移除后 | 不存在 / 不存在 | Untrusted | localhost 再次证书验证失败 |

22:12（UTC+8）独立复查确认两个存储都没有本次指纹，测试进程和计划任务均为 0。随后删除 Windows 隔离测试目录（含全部临时私钥）及桌面快捷方式。正式 BYOK 数据、已有证书和系统权限策略未改动。certutil 的 stderr 有图片色彩配置警告，但退出码为 0，信任状态与实际 HTTPS 结果也通过；原始输出保留在证据中。

### 保留的 curl 失败

Windows 系统自带 curl 8.22.0（Schannel）在未安装时返回 exit 60、`SEC_E_UNTRUSTED_ROOT`，符合预期；加 `--cacert` 后仍返回 exit 60、`the revocation status is unknown`，因此首轮脚本整体结果为 failed，未覆盖或删去。

后续采用 .NET 单参数 `AuthenticateAsClient` 的默认行为：验证证书与主机名，不设置证书接受回调，不关闭证书验证。该 API 默认不检查吊销列表，见 [Microsoft 文档](https://learn.microsoft.com/en-us/dotnet/api/system.net.security.sslstream.authenticateasclient?view=netframework-4.8.1)。所以它和要求可确定吊销状态的 curl 不是同一验证策略；不能据后续对照通过宣称所有 Schannel 客户端都通过，也不能仅由这次 curl 失败认定 Cursor 存在同样故障。

## 剩余边界

- 真实 BYOK 安装包按钮、Tauri 终端启动和 Cursor HTTPS 接管仍需单独验收，不能由本夹具代替。
- 企业组策略环境未覆盖；严格吊销检查客户端的失败保留上述限制。
- 旧“有效 CA 的 Ready 与系统信任 HTTPS 尚未验证”待办已经完成。桌面手动入口及私钥已清理，无需再次运行原快捷方式。

结构化原始结果及生产源码摘要见 [证据](2026-11-23-windows-ca-valid-evidence.json)。本轮只新增验证与修正文档，没有更改生产代码。
