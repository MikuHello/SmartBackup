# CLI MVP 验收与双轴评审

日期：2026-10-03（Asia/Shanghai）。评审基线：`bb03e6a7002c124b12852897602f462ecd450c7f`。初始实现：`d59679d`；修复：`4cffe05`、`5830ca6`。

按项目安装的 Matt `code-review` 流程，由两个独立只读评审分别检查 Standards 和 Spec，再对修复定向复核。下文保留发现与关闭证据，不把未验收的正式发行目标算作已完成。

## Standards

初审 2 项 P2，均已关闭：

1. **来源名称被静默改写。** Unix 单个文件名 `a\b.txt` 被转换为 `a/b.txt`，破坏文件名与目录结构。修复为逐个校验本机路径组件，再用 `/` 连接归档路径；非法名称明确失败。
2. **来源根符号链接被展开。** 创建 Job 提前 canonicalize，导致默认不跟随链接的语义失效。原型明确拒绝来源根链接，仍保存来源内部链接。复核进一步发现末尾 `/` 绕过；修复共用的 source_candidate，在检查前移除末尾分隔符和 `.`，保留 `..` 语义，再检查最后一级链接。测试覆盖 `/`、`/.`、`//./`，返回 unsupported_source_symlink。

复核结果：两项均关闭；未发现本次定向复核范围内的新问题。

## Spec

初审 2 项 P2，均已关闭：

1. **非便携名称未明确拒绝。** 与 Standards 的名称改写问题相同；以公开 CLI 回归测试证明拒绝且不生成归档。
2. **CACHEDIR.TAG 覆盖了更高优先级规则。** 缓存目录被无条件跳过，`!cache/` / `!cache/**` 无法重新包含。修复为最低优先级内置决策；测试验证预览和实际归档都包含显式重新纳入的文件。

复核结果：两项均关闭；实际范围与已声明 CLI alpha 原型一致。

初审统计：Standards 2 项、最高 P2；Spec 2 项、最高 P2（其中一个问题重复）。最终两轴未关闭项均为 0。

## 实际验证

在 macOS Apple Silicon、Rust 1.99.0、官方 7-Zip 26.03 上执行：

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace --locked`：21 项 CLI 集成测试全部通过，使用真实引擎。
- `cargo build --release --locked`：成功。
- `scripts/install-test-engine.py`：官方 macOS 引擎下载 SHA-256 与锁定清单一致。
- `scripts/demo.py`：创建计划、预览、备份、完整验证、浏览、无变化跳过、恢复、历史全链路成功；恢复文本与原文逐字节相同。

测试还覆盖：多来源同名文件、中文和 @ 前缀名称、mtime、空目录、已有恢复目录拒绝、符号链接、绝对路径/盘符/父级穿越/大小写碰撞、引擎哈希变化、数据库损坏隔离、Dry Run 不重建损坏数据库、Ctrl+C 取消、计划修改保留注释、删除计划保留归档。

Windows/Linux 仅已编写平台适配与 CI，未在本次会话执行。故障注入、网络挂载、加密、GUI 和正式发行待办详见 `MVP.md`。
