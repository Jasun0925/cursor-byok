# Issue #511：清理统计数据后回收 SQLite 文件空间

**已复现并修复一个独立缺口：设置中的两种统计清理都会删除记录，却不会收缩数据库文件。原作者 19.09 GB 主库的具体组成仍未知，不能把这一修复等同于 #511 的所有增长原因已解决。** 独立补丁提交为 [`ae6d8f3`](https://github.com/haohaomin/cursor-byok/commit/ae6d8f36b41abfa49e26a58aab8159262d045930)，合入状态见 [PR #1](https://github.com/haohaomin/cursor-byok/pull/1)；未发布安装包。

## 原报告与取证范围

- [上游 #511](https://github.com/leookun/cursor-byok/issues/511)：2026-10-27 再次读取，OPEN、0 条评论，正文只有截图。浏览器查看[原附件](https://github.com/user-attachments/assets/7b0a5baa-9719-42f1-8948-4313e0de22ba)，其中主库显示 **19.09 GB**、WAL **16.7 MB**、SHM **33 KB**。截图不能证明主库中是有效会话、诊断内容还是空闲页，也没有说明是否执行过清理。
- 本轮基线：`main=23e1a7b8c7a2e38d694786af8e653ed595b3c123`。使用独立工作区；没有把尚待审批验收的远程 Semble 分支混入本补丁。
- 正式库只执行只读 PRAGMA 和文件大小读取：主库 13,438,976 字节，WAL 0，SHM 32,768；页大小 4,096、总页数 3,281、空闲页 0、`auto_vacuum=0`、WAL 模式。未读取正文、未清理正式数据、未运行正式库 VACUUM。
- 清理与故障注入全部使用临时合成数据库，没有复制凭据或真实会话。

## 失败基线

给真实 Store 写入各 4 MiB 的模型请求、模型响应和 Cursor 追踪附件，先将 WAL 回写，再调用现有两种清理方法。测试要求文件变小，并保持连接池开启，避免把最后一个连接关闭时的隐式 checkpoint 当作清理功能。

| 模式 | 清理前主库 | 原代码清理后主库 | 原代码清理后 WAL | 空闲页 |
| --- | ---: | ---: | ---: | ---: |
| 仅详细记录 | 12,828,672 | 12,828,672 | 140,112 | 3,075 |
| 全部统计 | 12,828,672 | 12,828,672 | 131,872 | 3,075 |

两个“主库应缩小”的断言均失败。记录删除和历史保留断言已通过，问题发生在物理空间回收阶段。日志：`/tmp/cursor-byok-issue511-before.log`。

SQLite 在关闭自动收缩时会保留删除后的空闲页供后续写入复用，`VACUUM` 才会重建紧凑文件；WAL 还需要完成 checkpoint 才能截断。[SQLite VACUUM 文档](https://sqlite.org/lang_vacuum.html)、[WAL checkpoint 文档](https://www.sqlite.org/pragma.html#pragma_wal_checkpoint)。

## 实现与生命周期

现有路径保留，没有新增表、保留期限或自动删除规则。增加事务提交后的空间回收，并把两种范围收敛到同一清理生命周期。

```text
server/
├── src/control/settings.rs       # 接收既有 details/all 清理请求
└── src/store/
    ├── storage.rs                # 范围删除、后台任务、VACUUM 与 WAL 截断；158 行
    ├── storage/tests.rs          # 文件、历史、并发及失败恢复验证；465 行
    └── writer.rs                 # 既有应用写入串行锁；未改
.github/workflows/ci.yml           # Windows/macOS 也运行存储测试
```

行数含注释和空行；测试单列，没有生成代码。没有移动、删除模块或修改历史投影。

```text
设置确认清理 → 既有 HTTP DELETE → Store 持有独立任务
                                    ↓
                              获取应用写入锁
                                    ↓
                       删除选定统计数据 → 提交事务
                                    ↓
                        事务外 VACUUM main
                                    ↓
                       WAL checkpoint(TRUNCATE)
                                    ↓
                        返回统计数量 → 释放写入锁
                                    ↓
                         排队的会话/追踪写入继续
```

- 删除阶段失败：事务回滚，详细内容保留，后续写入仍可用。
- 回收阶段失败：明确报告“统计记录已清理，但空间回收未完成”。`wal_checkpoint` 返回 busy 时视为未完成，不能把 SQL 执行成功当作 WAL 已截断。
- 现有读连接继续读取原快照，不强制关连接。读连接在 SQLite busy timeout 内释放后，清理继续；持续占用则返回上述部分完成错误，之后可重试。
- HTTP 调用者断开后独立任务继续持有锁，完成文件回收；这不保证应用进程退出后的任务继续执行。
- 仅清理详细记录仍保留调用摘要、统计、配置和会话。全部统计仍只扩大到调用/追踪摘要；未增加会话、模型或 checkpoint 的删除范围。
- 清理后仍在运行的任务可以继续产生新记录；补丁没有关闭详细日志，也没有新增定期自动清理。

## 验证

修复后的六项测试使用真实 SQLite WAL 文件，连接池在测量时保持开启：

1. 详细清理释放文件，保留调用/追踪摘要、模型配置、设置、canonical checkpoint、Cursor 展示状态及被共享的 blob；重启 Store 后恢复历史并继续写入。
2. 全部统计清理释放文件且可重复，保留会话、配置和设置。
3. 活跃读快照不受破坏；排队的会话写入在清理后继续。
4. 持续占用的读连接产生明确部分完成错误，后续 Store 写入健康，读连接释放后重试回收成功。
5. 删除已提交后取消调用者，文件回收仍完成，写入锁正常释放。
6. 删除中注入错误，模型请求、响应和追踪附件全部回滚保留；移除故障后重试成功。

六项初轮验证通过；清理后主库 **233,472 字节（228 KiB）**、WAL **0**、空闲页 **0**，完整性和外键检查通过。加入模型配置保留及恢复后续写的最终版本，全工作区 `cargo test --workspace --all-targets` **475 通过、0 失败、0 忽略**；日志 `/tmp/cursor-byok-issue511-workspace-final.log`。

补丁提交 `ae6d8f3` 的[四组 CI](https://github.com/haohaomin/cursor-byok/actions/runs/40448321156)全部成功：Linux 全工作区 473 项，Windows/macOS 各 6 项原生存储测试，以及前端检查。本机的 475 项和 Linux 的 473 项分别按实际日志计数。CI 原始公开日志保存在 `/tmp/cursor-byok-issue511-ci.log`。本地合成库、跨平台原生库测试与原作者 19 GB 环境验收分别计入，后者仍未覆盖。

全工作区 Clippy（`-D warnings`）、格式和差异检查通过；Clippy 日志 `/tmp/cursor-byok-issue511-clippy.log`。复跑入口：

```sh
cargo test -p cursor-server --lib store::storage::tests -- --nocapture
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

本机全量构建首次被旧 `sqlx-macros` dylib 的 `mis-aligned LINKEDIT string pool` 阻止，未运行到测试；只清理该包的构建缓存后恢复。随后新增模型保留夹具使用了不支持的端点字符串，已改用项目的 `OPENAI_CHAT_ENDPOINT` 常量；这次失败属于测试夹具，不作为产品复现。各次日志独立保留。

## 边界与下一步

- 这是手动清理后的回收修复，不是对所有数据库增长的大小上限承诺。会话、checkpoint、共享 blob 和保留的统计数据仍占空间。
- VACUUM 会暂缓应用写入，耗时随保留数据规模变化，并需要额外临时磁盘空间；没有用 19 GB 原库验证耗时、磁盘满或原作者环境。
- 已检查源码中依赖的主键/外键及 checkpoint 恢复；没有修改会话内容或 provider history。隐式 rowid 不对外作为持久身份使用；现有上下文用量排序回归由完整工作区测试覆盖。
- 原作者仍需提供不含正文的表占用、空闲页和日志开关状态，才能判断主要增长来源。不要删除整个数据库来处理此报告。
- 未操作正式应用的清理按钮，未发布安装包，未关闭上游 Issue。上游归档只读，未重复尝试发送评论。
