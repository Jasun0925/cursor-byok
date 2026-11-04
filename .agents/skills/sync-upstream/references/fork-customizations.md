# Cursor BYOK 改版保留清单

这是合并审查清单，不是禁止这些文件发生变化。每次根据实际代码和用户新指令调整；目录移动后追踪职责，不依赖旧路径作为唯一判断依据。

## 无广告

| 位置 | 必须保留的行为 |
| --- | --- |
| `apps/desktop/src/shell/ads/` | 广告入口、浮层、样式、类型及浏览器缓存代码已删除 |
| `apps/desktop/src/shell/AppLayout.tsx` | 不恢复广告状态、焦点刷新、关闭反馈或广告入口；保留正常导航和使用教程 |
| `apps/desktop/src/shared/api.ts`、`src/demo/api.ts` | 不恢复 `/promotions` 客户端和演示接口 |
| `server/src/control/ads.rs`、`control/mod.rs`、`control/service.rs` | 不恢复广告拉取、图片缓存、关闭反馈和相关路由 |
| `server/src/store/settings.rs` | 不恢复广告专用 installation ID 读写；其他设置仍保留 |
| 桌面 package.json / package-lock.json | 不恢复仅供广告使用的 `react-css-marquee` |
| `apps/desktop/src/i18n/` | 扫描生成翻译目录，移除广告独占文案，不删除共享文案 |

可用搜索起点：

```bash
rg -n -i '广告|advertisement|promotions|AdMenu|FloatingAd|react-css-marquee|installation_id' apps/desktop/src server/src apps/desktop/package.json apps/desktop/package-lock.json
```

无匹配不是自动通过；还需检查上游新文件和新端点。协议定义中出现广告相关 RPC 不等于应用广告功能，不为消除关键词而破坏协议来源。

## 独立更新源与签名

| 位置 | 改版要求 |
| --- | --- |
| `apps/desktop/src-tauri/tauri.conf.json` | `plugins.updater.endpoints` 指向 `https://github.com/renhao12356578/cursor-byok/releases/latest/download/latest.json`；保留用户当前公钥 |
| `apps/desktop/src-tauri/src/update/mod.rs` | Windows 便携更新指向同仓库的 `portable-latest.json` |
| `README.md`、`README-EN.md` | 默认中文，顶部 English 链接进入英文版；下载链接和版本徽章指向用户仓库，保留原作者署名、协议来源及上游链接 |
| `.github/workflows/release.yml`、`.github/scripts/` | 使用当前 GitHub 仓库生成产物 URL，发布所有平台成功后才公开 Release |
| `.tauri/cursor-byok.key` | 既有私钥，仅本地与用户仓库的 `TAURI_SIGNING_PRIVATE_KEY` Secret 持有；目录继续被忽略 |
| `.tauri/cursor-byok.key.pub` | 本地公钥，应与应用配置一致；不要把公钥值硬编码进技能，避免形成多个来源 |
| `.agents/skills/release/SKILL.md` | 发布身份与 fork 仓库所有者一致，不恢复写死原作者 `leookun` 的授权约束 |

合并前记录应用公钥和两个端点，合并后比较；公钥可比较，不读取或展示私钥正文。`git check-ignore .tauri/cursor-byok.key` 应确认私钥继续被忽略；另外检查暂存文件列表，忽略规则不保护已经被跟踪的文件。

## 保留公益 TAB 服务

`server/src/store/settings.rs` 的 `PUBLIC_TAB_SERVICE_URL = "https://tab.leokun.cn"` 仍是有效的功能配置。TAB 请求由 `server/src/cursor/services/tab.rs` 转发，与删除的广告功能独立；不能全局删除或替换所有 `leokun` / `leookun` 引用。

## 发版产物验证

- `latest.json`：Tauri 各平台的版本、URL 和签名。
- `portable-latest.json`：Windows 便携包的版本、URL 和签名。
- `update.json`：当前发布流程仍生成的旧客户端清单；同步任务不顺便重新设计该流程。
- 确认各清单引用的文件实际存在，Release 为公开、非 prerelease、Latest。
- 下载真实产物，用应用内置公钥验证内容签名，并与 GitHub 提供的大小/摘要对照。比较签名 key ID 只能证明标识一致，不能代替完整签名验证。
- 首次从官方版本切换到改版需手动安装；以后从已有改版升级应验证检测、下载、签名、安装重启及版本号。失败时区分网络、清单、签名和安装步骤。
