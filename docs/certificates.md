# CA、证书与无证书使用方式

Cursor BYOK 的本地接管通过本机 HTTPS 代理读取 Cursor 请求。**在 Cursor 接管模式下，系统必须信任 BYOK 生成的本地 CA**；这是 HTTPS 代理链路的一部分，不是可选的模型配置。

## 公司设备禁止安装证书时

不能通过关闭 TLS 校验、信任所有证书或替换 Cursor 安装文件来绕过这一步。这会降低连接安全，也不属于本项目支持的运行模式。

可以选择以下路径：

1. 请管理员把本地 CA 安装到设备允许的信任范围，再在 BYOK 中刷新 CA 状态。
2. 不接管 Cursor，改用 BYOK 的**外部 API**。在设置页打开外部 API 并设置密钥，使用显示的本机地址（例如 `http://127.0.0.1:<端口>/byok/v1`）调用已配置的模型。该路径由本机应用直接提供 HTTP 接口，不需要 Cursor 接管或本地 CA。
3. 使用组织批准的其他模型网关或官方模型；这不经过 BYOK 的 Cursor 接管链路。

外部 API 仍要求 BYOK 进程保持运行，并且每个请求都要带设置中的 Bearer API Key 或 `x-api-key`。它只监听本机地址，不应把端口暴露到公网。

## 证书存放与备份

CA 证书和私钥只保存在本机数据目录。私钥等同于本地代理的敏感凭据；不要上传到 Issue、聊天或日志中。备份整个数据目录或模型配置导出文件前，先确认其中是否包含密钥材料并妥善保存。

旧版本可以继续运行多久取决于 Cursor 协议、系统信任策略和本仓库更新；项目不承诺旧版本永久兼容，也不建议通过长期停留旧版本规避证书政策。

## 排查顺序

```text
BYOK 运行
  ↓
设置页显示 CA ready
  ↓
系统信任本地 CA
  ↓
开启 Cursor 接管
  ↓
重启 Cursor 并选择 BYOK 模型
```

若企业策略禁止安装 CA，停在第二步时应切换到外部 API 或组织批准的其他入口；不要继续尝试关闭 TLS 校验。

## English summary

Cursor takeover requires the local CA to be trusted because BYOK inspects Cursor HTTPS traffic through a local proxy. If a managed device cannot install certificates, do not disable TLS verification. Use the local external API instead, or use an organization-approved gateway. The external API is bound to `127.0.0.1`, requires its configured key, and does not require Cursor takeover or the local CA.
