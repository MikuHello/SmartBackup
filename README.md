# Smart Backup

Smart Backup 是一个面向个人用户和开发者的跨平台文件备份工具。它把一个或多个文件/目录保存为新的、不可变的标准压缩快照，并提供变化检测、校验、历史、调度和安全恢复。

> 当前状态：已实现 `0.1.0-alpha.1` CLI 原型，核心流程已在 macOS 实跑。历史 BAT 已移除。完整桌面版尚未实现；能力边界见 [MVP 使用说明](docs/MVP.md)。

完整需求、架构、安全边界、任务依赖和验收标准见 [`Task.md`](./Task.md)。

## 立即体验 CLI MVP

```sh
cargo build --release
./target/release/smart-backup --help
python3 scripts/demo.py
```

需要已安装的 7-Zip（`7zz` / `7z`）。演示只使用自动生成的示例资料，完整执行备份、验证和恢复。实际使用方法、支持范围与退出码见 [docs/MVP.md](docs/MVP.md)。

本项目已配置 [Matt Pocock skills](https://github.com/mattpocock/skills)，版本锁定与流程记录位于 [docs/agents/](docs/agents/workflow.md)。以下路线图保留正式发行的验收目标，不把原型能力等同于完整交付。

## 为什么做

- 产物是普通的 7z、ZIP 或 TAR 系列压缩包，脱离本软件仍可恢复；
- 每次 Run 创建独立快照，不原地更新旧归档；
- GUI、CLI 和调度器共享同一 Rust Core；
- 默认执行归档测试并生成 SHA-256；
- 持久化的加密秘密只进入操作系统钥匙串，不写入配置或日志；
- 输出目录、卷身份、保留删除和恢复路径都有明确安全校验。

## 目标技术栈

| 层 | 方案 |
| --- | --- |
| Core / CLI / Scheduler | Rust |
| Desktop UI | Tauri 2 + Vue 3 + TypeScript |
| 配置 | TOML |
| 状态与历史 | SQLite |
| 详细日志 | JSONL |
| 压缩引擎 | 随包内置并校验的官方 `7zz` |
| 密钥 | Windows Credential Manager / macOS Keychain / Linux Secret Service |

## 首个完整桌面版目标

- 多文件、多目录、跨卷来源，每个来源映射为唯一归档顶层别名；
- 正式创建 7z、ZIP、TAR、tar.gz、tar.xz、tar.bz2；
- metadata、hash、always 三种变化检测和强制运行；
- gitignore 语义过滤、规则解释与 Dry Run；
- 7z 文件名加密、ZIP AES-256 和显式弱兼容模式；
- 分卷、`7zz t`、SHA-256、内嵌 provenance manifest；
- 全量/部分恢复和跳过、覆盖、重命名冲突策略；
- 频率预设、五字段 cron、时区、错过执行和 DST 规则；
- 中文/英文 GUI、浅色/深色主题和键盘完整操作；
- Windows、macOS、Linux 桌面发行包。

## 安全底线

- 受管理任务永远不修改或删除来源；
- 输出目录不能位于任一来源内部；
- 路径存在但卷/挂载身份不匹配时不运行；
- 加密不可用时失败，不降级生成明文；
- 密码不进入 argv、环境变量、TOML、数据库、日志或 manifest；
- 只有新备份成功且验证通过后才可执行保留清理；
- 自动删除前必须从历史定位候选并核对归档 manifest 身份；
- 已有产物不覆盖，未知归属的 partial 不自动删除；
- 所有恢复输入都按不受信任归档处理。

## 路线图

以下清单按交付阶段组织；阶段内的精确依赖和验收以 [`Task.md` 第 22 节](./Task.md#22-实施工作分解) 为准。

### MVP — 0.1.0 CLI 垂直切片

- [ ] 建立 Rust workspace、三平台 CI、lint 和测试基线
- [ ] 定义 Global Config、Job、Manifest、Run、CLI JSON 和退出码 schema
- [ ] 实现安全路径、来源别名、卷身份和输出递归检查
- [ ] 集成锁定版本的 `7zz`、哈希校验、能力探测和安全 argv builder
- [ ] 实现 Filter Engine、扫描、空间估算和 Dry Run
- [ ] 实现 `.7z` 不可变快照流水线、partial、可恢复安全提交和取消
- [ ] 实现固定 `full` 验证（`7zz t` + SHA-256）与内嵌 manifest
- [ ] 实现 SQLite 摘要/安全重建、JSONL 日志和 metadata 变化检测
- [ ] 实现 Job CRUD、运行、历史、归档列表、验证和安全全量恢复 CLI
- [ ] 提供人类输出、`--json` 和稳定退出码

### 完整 Core 与安全能力

- [ ] 增加 ZIP、TAR、tar.gz、tar.xz、tar.bz2 driver
- [ ] 实现两阶段 TAR 临时空间和分卷成组生命周期
- [ ] 增加 `archive_test` / `off` 验证策略与显式验证状态
- [ ] 接入三平台钥匙串、7z 头部加密和 ZIP AES-256
- [ ] 实现 hash/always 变化检测与强制运行
- [ ] 实现最大数量/年龄保留及 manifest 身份核验
- [ ] 加固归档浏览、部分恢复和恶意路径防护
- [ ] 完成崩溃、掉电、写满和目标掉线的恢复加固

### Scheduler 与桌面 GUI

- [ ] 实现跨进程队列、同 Job 去重和可配置全局并发
- [ ] 实现 preset/cron、IANA 时区、missed-run、DST 和电源策略
- [ ] 配置当前用户级 scheduler 自启动
- [ ] 建立最小权限 Tauri shell、严格 CSP 和 typed Rust commands
- [ ] 实现 Job 卡片、可视化表单、高级 TOML 编辑和参数帮助
- [ ] 实现预览、实时进度、取消、历史和日志界面
- [ ] 实现归档浏览、验证、恢复、通知和诊断包
- [ ] 完成简体中文/英文、主题、缩放、键盘和 reduced-motion 支持

### 发行准备

- [ ] 实现 schema 迁移快照、只读修复、Job 导入导出和便携模式
- [ ] 完成 Windows x64 安装包与便携包
- [ ] 完成 macOS Apple Silicon/Intel 签名与公证 DMG
- [ ] 完成 Linux glibc x64 AppImage，可行时增加 `.deb`
- [ ] 发布 SBOM、第三方 notices、应用包 SHA-256 和变更日志
- [ ] 实现 GitHub Releases 版本检查与共享缓存
- [ ] 跑完格式、平台、安全、故障注入、辅助功能和本地化验收矩阵
- [ ] 按 MIT OR Apache-2.0 双许可发布

### 后续

- [ ] WebDAV、对象存储、rclone 和可选第二本地副本
- [ ] VSS、APFS、LVM/Btrfs 一致性快照
- [ ] Windows/Linux ARM64
- [ ] 签名自动更新
- [ ] 邮件和 webhook 通知
- [ ] 更细的 CPU、内存和 I/O 限速
- [ ] 更多正式写入格式与平台元数据能力

## 不在范围内

- 整机镜像、裸机恢复或操作系统灾难恢复；
- 数据库和虚拟机磁盘的一致性热备保证；
- 企业多租户、RBAC、管理员/root 常驻服务；
- 私有去重仓库或增量链恢复；
- 首版 pre/post hooks；
- 自动追踪用户手动移动、改名或删除的归档。

## 开始实施前

1. 通读 [`Task.md`](./Task.md) 的安全不变量、状态语义和验收场景；
2. 从 T01-T08 完成 `0.1.0` CLI 垂直切片；
3. 不在核心契约稳定前抢先搭建依赖模拟数据的完整 GUI；
4. 任何可能修改来源、泄漏秘密、覆盖产物或误删文件的改动先重新评审。
