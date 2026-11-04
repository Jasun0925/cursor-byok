---
name: sync-upstream
description: 同步 Cursor BYOK 个人 fork 的上游更新，保留无广告定制、独立更新源和既有签名，处理合并冲突并验证；用户要求发版时衔接桌面发布流程。用于“同步上游”“合并原仓库更新”“更新我的改版”等任务。
---

# 同步个人 fork 上游

把原作者的更新合入用户维护的 Cursor BYOK，保持既有定制可用。先核对当前仓库，不把历史版本号、某次网络故障或个人 fork 的称呼当成当前事实；“个人仓库”不等于 GitHub 私有仓库。

## 任务范围

- “检查上游更新”：只获取、比较和报告，不合并、不提交。
- “同步/合并上游”：完成隔离分支上的合并、冲突处理和验证；按用户已有授权提交或推送代码。不自动推送版本标签、发布 Release 或修改 Secrets。
- “同步并发布”：先完成同步与验证，再按 [release 技能](../release/SKILL.md)执行用户已授权的发布。授权在本次任务中持续有效，不重复索要。
- “发布这个 skill 到 GitHub”：发布技能文件，不等同于发布桌面应用。

## 当前仓库关系

```text
upstream: leookun/cursor-byok              # 原作者源码，只作为同步来源
    │ fetch → merge
    ▼
codex/sync-upstream-<日期或主题>           # 合并、解决冲突、验证
    │ 经检查合入
    ▼
main → origin: haohaomin/cursor-byok  # 用户维护的源码
    │ 仅在发版已授权时推送新版本标签
    ▼
.github/workflows/release.yml             # 多平台构建、签名、发布
    │ latest.json / portable-latest.json
    ▼
用户仓库的 Release → 已安装的改版应用    # 下载并验证更新
```

仓库迁移时以用户明确指定的目标为准，先验证 `origin`、`upstream` 和 GitHub 仓库身份，不静默改写远程地址。永远不要把此 fork 的提交推到 `upstream`。

## 1. 检查并获取更新

1. 读取 `AGENTS.md`，检查工作区、分支、远程地址和相关技能。保护未提交修改；不要替用户提交不相关文件，也不要清理工作区来强行开始同步。必要时使用独立 worktree。
2. 获取 `origin` 默认分支和 `upstream` 目标分支。当前双方使用 `main`；每次重新核对。默认 `git fetch --no-tags upstream`，避免导入与个人发版重名的上游标签。
3. 对照 `git log main..upstream/main` 和 `git diff main...upstream/main`，说明新增提交、涉及模块、可能影响的定制和数据升级。没有新增提交时结束，不创建空合并或升版本。
4. 若用户指定上游版本/提交，使用该明确目标；不要擅自改为上游最新开发分支。

干净工作区且双方仍使用 `main` 时的参考命令：

```bash
git switch main
git fetch origin
git merge --ff-only origin/main
git fetch --no-tags upstream
git log --oneline main..upstream/main
git diff --stat main...upstream/main
```

`--ff-only` 失败表示历史分叉，应检查本地独有提交，不使用 `reset --hard` 或强推消除差异。

## 2. 合并并保留定制

从用户主分支创建 `codex/sync-upstream-<日期或主题>`，默认用 `git merge --no-ff --no-commit <已确认的上游引用>` 保留上游祖先关系。不默认 rebase 已发布主分支，也不把逐个 cherry-pick 作为长期同步策略。

先读取 [定制保留清单](references/fork-customizations.md)，再处理冲突。这个 merge 中 ours 是用户改版，theirs 是上游；不要整仓或整份共享配置统一选一边。即使没有冲突，也要检查上游新增路径是否恢复了广告或改回官方更新源。

- 用 `git diff --name-only --diff-filter=U` 列出冲突。对删除/修改冲突先确认单一职责；广告专用文件保持删除，共享文件保留上游新功能并移除广告部分。
- 先合并依赖声明，再整理锁文件；不要删除整个锁文件来逃避冲突或顺带升级全部依赖。
- 翻译源确认后通过 `npm --prefix apps/desktop run i18n:scan` 重新生成目录；补全新增语言条目，不手改生成目录冒充已解决。
- 涉及数据库时读取 database-schema 技能。不要改动已经应用过的迁移；需要升级验证时使用数据库副本。
- 合并尚未提交且需要放弃时使用 `git merge --abort`；只撤销本次操作，不覆盖用户原有工作。

## 3. 验证并交付代码

依赖/锁文件确认一致后运行 `npm --prefix apps/desktop ci`。针对上游变更使用对应技能；通常执行：

```bash
make check
npm --prefix apps/desktop run tauri:build -- --debug --no-bundle
node --test .github/scripts/generate-portable-update.test.mjs
git diff --check
```

若命令已被上游调整，按实际脚本选择等效检查，不调用已删除入口。验证无广告、页面导航、模型配置/连接、Cursor 对话及 TAB 模式；区别静态检查、演示数据验证和真实 Cursor 端到端验证，缺失项如实报告。

检查暂存区仅含本次同步文件、不含 `.tauri/` 或其他敏感材料，再提交合并。用户授权推送时，确认远程未新增提交，再将验证过的分支合入自己的主分支并推送 `origin`。远程前进则重新整合并验证，不强推。

按目录树报告实际新增、移动、删除的模块，列出上游目标提交、合并提交、保留的定制、验证结果和未完成项。如果推送了代码，跟进对应提交的 CI；推送 `main` 只做 CI，不表示桌面安装包已更新。

## 4. 仅在用户要求时发布应用

执行 [release 技能](../release/SKILL.md)，并保留以下 fork 特有要求：

- 读取当前清单、已发布版本和远程标签后选择递增且未占用的版本，不固定使用 `1.0.3`、不倒退到上游较小版本。普通源码同步不必升版本；若上游版本字段影响当前改版，先保持自身版本一致。
- 同步桌面 package.json、package-lock.json 的顶层/根包版本、Tauri 配置、桌面 Cargo.toml 和 Cargo.lock 中对应包的版本。不更改独立的 server 版本，也不全局替换版本字符串。
- 复用现有签名密钥和 Actions Secret。当前 `make build-desktop` 会使用另一套本地密钥，不拿该产物充当正式自动更新包。
- 版本提交必须已在用户的 `origin/main` 且 CI 通过。仅推送本次标签，不使用 `git push --tags`。
- 核验各平台产物、签名、三个更新清单、Latest 状态和用户仓库下载地址。至少验证一个真实更新包的完整签名；有条件时从已安装的改版执行更新，不把“首页可达”或“构建成功”称为自动更新端到端通过。

## 异常处理边界

- 网络超时：先查询远程 SHA、标签和运行记录，确认上次操作是否已成功，再有限重试；可临时用 `git -c http.version=HTTP/1.1 ...`。不要为此改全局 Git 配置。
- 标签存在但无运行：检查 fork 的 Actions 启用状态、workflow 触发条件和延迟。不要直接删/移动标签，也不要仅为重试就连续增加版本号。
- 发版失败：记录运行链接和具体失败步骤，先诊断。是否重跑遵循用户已有授权及 release 技能；代码需要更改时用新的发布版本，不覆盖已发布版本。
- 签名验证失败：检查公钥与现有私钥的配对，不禁用验证、不自动轮换密钥。私钥缺失时不能从 GitHub Secret 读回，寻找用户备份后再处理。
