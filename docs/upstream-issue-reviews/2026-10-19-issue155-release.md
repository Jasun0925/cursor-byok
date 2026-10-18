# v1.0.17 发布记录：#155 完成一半

用户已授权提交并发布已修复部分，#155 按“完成一半”记录，不关闭上游 Issue。

发布目标为 `haohaomin/cursor-byok`，当前 `origin`、GitHub 登录账号与仓库所有者一致，权限为 ADMIN。原 `renhao12356578/cursor-byok` 地址仍解析到同一仓库，应用现有更新入口与签名公钥保持不变。本次使用稳定版本 `v1.0.17`，GitHub Release 必须为普通 Latest Release。

## 本次范围

- 修复后台任务开关与 Bootstrap 配置身份，确保 Cursor 能加载后台通知配置。
- 进度和重复通知以 TurnEnded → EOF 确认；按单个任务去重，避免拆批、合批或重发重复唤醒。
- 保存检查点的 Cursor 状态，迟到完成通知保留已压缩历史；首次导入与较新历史继续保留。
- 新增数据库迁移 11。已有迁移未改写，回归覆盖升级前历史保留、外键约束和关闭后重开。

真实 Cursor 25 秒普通后台任务自动续答通过；完成通知四种重放去重通过。真实审批等待、压缩期间完成、最终补丁的输出通知及官方→BYOK 转入尚无完整实机覆盖。20 分钟测试按用户要求跳过。详见 [验收记录](2026-10-19-issue155-acceptance.md)。

## 发布前检查

从 `origin/main=8d74b25f3b09432d6d79736b2f6bcb0555f9229c` 建立隔离发布工作树，包含 #155 修复、关联测试/证据和五个版本文件；原工作区其他改动没有混入。版本文件统一为 1.0.17，独立服务端包版本未变。

`npm run check`、`npm run tauri:build -- --debug --no-bundle`、Clippy、格式与差异检查通过。隔离发布目录完整 Rust 回归 424 项通过、0 失败、0 忽略。测试数与原工作区 426 项不同，因为没有带入其他 Issue 的本地测试改动。

首轮默认环境及单独复核均触发既有 `request_transport_failure_is_one_failed_attempt` 断言；同一已编译测试程序为本地地址设置 NO_PROXY 后立即通过（0.03 秒）。因此完整回归只为测试进程设置 `NO_PROXY=127.0.0.1,localhost,::1`；系统代理和产品源码没有为此改动。

发布工作流验证为仅 `v*` 标签触发，使用 `tauri-action@v1` 的 `uploadUpdaterJson: true`，`prerelease: false`，最终设为 Latest。Proto 构建输入已跟踪。现有公钥与本地公钥匹配；Actions 存在签名密钥 Secret，无密码 Secret。没有读取或上传签名私钥。

本地构建没有替换已安装应用，也没有升级正式数据库。提交、标签和 CI 结果在发布后追加。
