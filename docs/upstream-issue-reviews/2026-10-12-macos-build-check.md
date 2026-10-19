# macOS SQLx 宏加载阻断：本地构建配置

2026-10-12 已解除当前工作目录的构建阻断。直接执行以下命令，无需再在 shell 中临时设置 SDK 或 linker：

```sh
cargo test --locked -p cursor-server --test compaction -- --nocapture
```

结果：编译成功；6 项测试通过，0 失败、0 忽略、0 筛除，测试耗时 6.52 秒。该结果验证构建环境和既有压缩回归，不代表已经复现或解决 #297 的长对话性能问题。

## 原因与处理

本机为 macOS 27.0.1（26A434）、Command Line Tools linker 27037.1、Rust 1.94.1。默认 macOS 27.0 SDK 及系统 linker 生成的 `sqlx_macros` 库在编译期间被 dyld 拒绝，错误为 `mis-aligned LINKEDIT string pool`；后续 SQLx unresolved imports 是连带错误。

清理缓存、全新 target、禁用 fixup chains、移除符号以及 Rust 1.93.0 的对照都未解除阻断。仅换 Rust 自带 LLVM linker 时，它无法解析 macOS 27 SDK 的 `arm64e.x1` TAPI 架构。仓库 [9 月 29 日记录](../upstream-pr-reviews/2026-09-29-pr424-test.md) 已验证 LLVM linker 与已安装 macOS 26.5 SDK 的组合；之前只是命令级设置，本次将它保存为工作目录的本地默认配置。

```text
仓库根目录/
├── .cargo/config.toml  # 本机 SDKROOT 和 macOS Rust linker 参数
└── .git/info/exclude  # 排除上述机器专属配置，避免进入提交
```

配置内容：

```toml
[env]
SDKROOT = "/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk"

[target.'cfg(target_os = "macos")']
rustflags = ["-C", "link-arg=-fuse-ld=/Users/renhao/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/gcc-ld/ld64.lld"]
```

这是当前 checkout 的本地配置，不会随 Git 同步到其他机器或 worktree。系统默认 SDK 软链接仍指向 27.0，Rust 默认仍是 stable 1.94.1，应用最低部署版本仍为 macOS 11.0。没有更改 SQLx 依赖、数据库、生产代码或系统工具链安装。

## 产物核对与边界

- Cargo fingerprint 确认 `sqlx_macros-a1e6e500bf50565c` 使用上述 LLVM linker 参数。
- Mach-O `LC_BUILD_VERSION` 记录 SDK 26.5、linker tool 4 / version 21.1.8、minos 11.0。
- 原失败产物 `LC_SYMTAB.stroff=6481748`；成功产物 `stroff=6481588`，二者模 8 均为 4。因此不能仅根据偏移就宣称“LLVM 修复了 8 字节对齐”；这里证实的是所选 SDK/linker 组合可被加载并完成测试，未证明系统 linker 或 dyld 内部的唯一缺陷。
- 普通 Cargo 命令已通过；用户显式提供的 SDKROOT/RUSTFLAGS 或其他目标平台属于另外的构建配置。本轮未验证桌面打包或发布。
- 本次临时安装的 Rust 1.93.0 和独立试验目录已移除；保留通过验证的正常构建缓存。
