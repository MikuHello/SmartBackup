# Smart Backup 0.1.0-alpha.1 CLI 原型

这是可运行的第一版 CLI MVP，不是 Task.md 中三平台正式发行的完成声明。历史 BAT 已删除。

## 运行

环境需要 Rust（支持 edition 2024 和 let-chains；建议当前 stable）和 7-Zip。引擎必须显式选定，程序会固定其 SHA-256，每次加载时验证；升级 7-Zip 后需要重新配置。

```sh
cargo build --release
./target/release/smart-backup --help
python3 scripts/demo.py
```

`demo.py` 只在项目 `.demo/run-*/` 下生成示例文件、状态和归档，实跑预览、备份、校验、浏览、无变化跳过、恢复和历史。每次使用新目录，不会覆盖上次演示。

实际备份示例（输出目录须先存在，以避免把脱落的挂载盘冒认为普通目录）：

```sh
./target/release/smart-backup engine configure --path /opt/homebrew/bin/7zz
./target/release/smart-backup job create Documents --source "Documents=/absolute/source" --output /absolute/existing/backup-directory
./target/release/smart-backup run Documents --dry-run
./target/release/smart-backup run Documents
./target/release/smart-backup history list
./target/release/smart-backup verify /absolute/archive.7z
./target/release/smart-backup restore /absolute/archive.7z --output /absolute/new-restore-directory
```

Windows 使用 `target\release\smart-backup.exe`，引擎路径通常是 `C:\Program Files\7-Zip\7z.exe`。本机只实测 macOS；Windows/Linux 的适配和 CI 已编写，尚未在对应系统执行。

默认状态目录：Windows `%LOCALAPPDATA%/SmartBackup`；其他平台 `$HOME/.local/share/smart-backup`。全局 `--home PATH` 或 `SMART_BACKUP_HOME` 可覆盖。

## 已实现

- Rust Core / CLI workspace；公共 TOML/JSON schema_version 1。
- Job create/list/show/edit/enable/disable/delete；UUID 稳定；Unicode NFKC casefold 名称/别名比较；编辑保留注释并备份 TOML。删除 Job 不删除归档。
- 多来源文件/目录与独立顶层别名；输出递归检查；绑定卷身份并在运行前核对。
- 内置杂项过滤、CACHEDIR.TAG、可选动态 `.gitignore`、规则文件、内联规则与 `!` 重新包含；预览提供过滤原因。
- `metadata` 变化检测，`--force` 与 `--dry-run`。
- 只读捕获来源到输出卷私有暂存目录，7z 独立全量快照、内嵌 manifest、`7zz t`、SHA-256。
- 不覆盖发布：hard-link/exclusive-create；提交 journal；完整且可证明归属的中断提交可补记交付。
- SQLite 历史；损坏副本隔离重建；JSONL 阶段事件；同状态目录跨进程排他锁。
- 归档浏览、全文/子串搜索、完整验证、向新目录安全恢复；拒绝绝对路径、父级穿越、盘符、路径碰撞和链接父目录。
- Unix 平台保存来源目录内部的符号链接，恢复安全的相对链接；来源根本身为符号链接时明确拒绝，不隐式展开；绝对或含 `..` 的链接目标拒绝恢复。Windows 的符号链接能力尚未交付。
- Ctrl+C/终止信号取消；进入提交后完成安全交付，不因取消留下假成功。

## 原型边界

这些限制是显式收窄，Task.md 仍是正式版目标，不把缺项标记为已完成：

- 尚无 GUI、scheduler、保留删除、增量/去重、加密、分卷、ZIP/TAR 创建和云端存储。
- 采用用户提供并固定哈希的外部 7-Zip；未交付三平台内置官方 sidecar、完整参数能力矩阵或原生参数代理。
- macOS 使用文件系统挂载点与 diskutil VolumeUUID；Linux 需要可取得 UUID 的本地文件系统；Windows 使用 Get-Volume。网络卷、容器 overlay 和无稳定身份的卷明确拒绝。辅助 ID 真值表、完整重绑定 UI 待后续。
- 为保持来源别名与过滤一致，先完整复制内容再压缩；预检保守估算约两倍输入字节加余量，不适合超大、高频数据集。扫描和目录结构保存在内存。
- 默认 metadata 检测无法发现“内容变化但大小/修改时间不变”。读取前后检查可捕捉部分并发修改，但不是 VSS/APFS 热快照；检测到变化时本原型失败重试，不交付部分成功包。
- 恢复逐条从 stdout 写入私有目录，安全性优先；固实归档的大量文件恢复较慢。只支持新目标，不提供覆盖、重命名冲突策略或选择恢复。
- 未取得完整可证明归属的中断暂存/提交文件保留等待人工检查，不自动删除、不扫描归档重建备份库。完整掉电与磁盘故障注入矩阵尚未跑完。
- 源目录、输出目录被其他进程恶意重命名/替换的完整 TOCTOU 加固尚未验收；不要把本原型当成针对恶意本地进程的安全边界。
- 不支持非 UTF-8 名称、跨平台非法名称、特殊设备文件；Unix 权限/ACL/扩展属性没有完整恢复保证。
- 尚无 Job 导入导出、版本迁移 UI、细粒度队列、日志自动清理、签名安装包。
- 同 home 的查询也使用排他锁，运行时并发调用会返回 `busy`。

## JSON 与退出码

加 `--json`，stdout 返回单个对象：`schema_version`、`status`、`reason_code`、`data` 或 `message`。帮助/版本请求使用普通 CLI 输出。不要解析人类文案来决定流程。

| 退出码 | 含义 |
|---|---|
| 0 | 成功（含预览和查询） |
| 1 | 有可用产物但存在警告 |
| 2 | 跳过，例如 no_changes |
| 3 | 已取消 |
| 4 | 参数/计划配置错误或不支持的能力 |
| 5 | 环境、路径、卷或引擎依赖错误 |
| 6 | 运行/验证/恢复失败 |
| 7 | 预留内部错误 |

## 开发验证

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

测试从 CLI 接口观察行为，使用真实 7-Zip 和隔离临时文件。Windows/Linux CI 是待执行的构建配置，不能据此声称已在三平台通过。
