# Windows 当前用户 CA 存储验证

日期：2026-11-21（Asia/Shanghai）。基线：main `68c4dfbc99bcba8f5e56d6e0535b4f2c92a584de`，包含 fork PR #43。上游来源是 [PR #458](https://github.com/leookun/cursor-byok/pull/458)，不是同编号 Issue。

## 结论

真实 Windows 普通用户下，现有证书检测代码能区分已信任和未信任证书，并拒绝非法 PEM。尚未完成用户确认后的实际安装，不能将 PR #43 标记为端到端验收通过；未发现需要据此新增生产补丁的证据。

上轮“Windows 证书测试”只覆盖安装命令字符串等自动化，本次补充运行真实 Win32 检测实现。SSH 连接正常，其进程处于 Session 0；已登录桌面是 Session 1。通过一次性计划任务、Interactive 登录类型和 Limited 权限运行测试，确认测试进程 SessionId=1、管理员身份=false。

## 方法与结果

用隔离的小型 Rust 可执行程序直接编译当前仓库的 `server/src/local_app/ca/windows.rs`，未改写其检测算法。仅提供 CLI、Error/Result 适配与现有依赖 `pem 3`、`windows-sys 0.61`，不启动正式 BYOK，不读取正式配置或私钥。

复制到 Windows 的源文件 SHA-256 与本地文件一致：

```text
2a524903932350ea96cc4bd0c1503014456aa81cc64bcbcedeb6000811d78c41
```

| 场景 | 实际结果 | 证明范围 |
| --- | --- | --- |
| 读取当前用户 Root 逻辑存储中的一个现有公共根证书，传入检测函数 | `installed=true` | 真实 Win32 枚举和 DER 匹配路径 |
| 传入新生成、未导入的隔离自签名证书 | `installed=false` | 不把任意合法 PEM 当作已信任 |
| 传入非法 PEM | `parse CA PEM: malformedframing`，退出码 1 | 解析失败显式报错 |
| 普通用户桌面运行 `certutil -user -addstore -f Root <临时证书>` | 输出“签名与公钥匹配”，180 秒未结束，测试终止进程 | 未收到交互确认；不算安装成功 |
| 安装尝试后的检测 | `installed=false`；LocalMachine Root 中也不存在 | 没有留下临时信任，未修改机器级根存储 |

隔离证书名称为 `Cursor BYOK isolated acceptance 20261121`，指纹 `F351CD5AABBCDA1C1B98096FDD2CC11232E33111`。测试只上传公共证书，私钥留在本地临时目录；收尾删除临时私钥、测试目录和一次性计划任务。未改动正式 BYOK 数据、Cursor 设置、已有信任证书或 Windows 永久权限策略。

## 仍缺的验收

- 在普通用户桌面确认本次根证书安装后，验证 certutil 成功退出、同一证书的检测结果变为 true，随后移除并确认 false。
- 从真实 BYOK 安装包点击安装入口，验证终端窗口、证书确认和 CA 状态刷新完整链路。此次测试启动了真实 certutil，但没有从 Tauri 按钮启动，不代替该验收。
- 当前用户 Root 是 Windows 逻辑存储；检测已有根证书的结果不证明其物理注册表来源，也不等于企业组策略环境已覆盖。

## 同轮合并状态核对

2026-11-19 的八项合并（#13、#43–#49）均已存在于远端 main。GitHub 最新公开版本仍为 2026-11-04 发布的 v1.1.0；这八项合并不在该版本中。已更新工作状态、维护状态、Issue 和 PR 清单，移除当前描述中错误的“审批候选未提交”“#49 等待合入”等表述，保留真实 UI 未验收边界。

上轮测试日志仍可核对：模型接口 18 通过；最终服务端单测 301 通过、0 失败、14 忽略；前端检查、四语言扫描、macOS 桌面构建通过。此处记录上轮结果，不声称本轮重跑这些套件。
