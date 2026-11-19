# 文档导航

维护：2026-11-02。先看当前状态，需要复核时再进入历史证据。

| 你要找什么 | 入口 |
| --- | --- |
| 哪些做好、哪些没做；代码有没有提交 | [工作状态](work-status.md) |
| 安装包已经包含哪些修复 | [维护状态](maintenance-status.md) |
| 所有 Issue 的进度与下一步 | [Issue 清单](issues.md) |
| 上游 PR 能否采用、风险是什么 | [PR 审查清单](pull-requests.md) |
| Linux 安装包依赖和发行版边界 | [Linux 安装指南](linux.md) |
| CA、证书和无证书使用方式 | [CA 与证书说明](certificates.md) |
| Remote SSH 如何配置 | [远程连接指南](remote-ssh.md) |
| 复现、失败基线、验收、截图与旧发布 | [历史资料](history/README.md) |
| 安装、功能和使用说明 | [中文 README](../README.md) / [English README](../README-EN.md) |
| Semble 来源与基准测试 | [来源](../UPSTREAM.md) / [基准说明](../support/benchmarks/semble/README.md) |

```text
docs/
├── README.md              # 文档导航
├── work-status.md         # 开发交付状态
├── maintenance-status.md  # 用户可用范围
├── issues.md              # Issue 清单
├── pull-requests.md       # PR 审查清单
├── linux.md               # Linux 安装依赖与兼容边界
├── certificates.md        # Cursor 接管的 CA 要求与外部 API 替代路径
├── remote-ssh.md          # 当前使用指南
└── history/               # 按日期保留的证据
```

维护时更新稳定入口，新的复现材料放入 history 的对应分类。发布、合入、自动化通过、真实 UI 验收分别记录。保留失败证据和来源署名；已完成计划、重复全文及旧版架构草稿不继续作为当前文档保留。

本轮清理：10 份发布笔记合为一份历史记录；删除已被最终结果替代的 PR424 计划、冗余模型回答全文及根目录旧架构草稿 `cursor.md`；去除主清单内重复的滚动进度叙述。旧内容可从 Git 历史找回。
