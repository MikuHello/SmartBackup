# 本次 Matt skills 执行记录

来源与锁定版本见 skills-lock.json。使用 setup-matt-pocock-skills、to-spec、to-tickets、implement、tdd、domain-modeling 和 code-review 的约定。用户已确认本地 Markdown tracker、AGENTS.md、单一术语表、CLI 测试 seam 和评审基线；复用原有 Task.md T01–T08，不进行重复长访谈。

规格：`.scratch/mvp/spec.md`。纵向任务：`.scratch/mvp/issues/`。本次功能边界：`docs/MVP.md`。

已观察到的红→绿循环：

1. Job 创建/读取：缺少 name 数据 → 创建、序列化、名称查询通过。
2. 真实快照/验证：缺少 engine 命令 → 引擎配置、归档、manifest、校验通过。
3. 变化检测/历史：无变化仍返回成功 → skipped/force/旧包不变/历史通过。
4. 浏览/恢复：缺少 archive 命令 → 字节一致、空目录和已有目标拒绝通过。
5. 过滤预览：缺少 edit 命令 → 排除、重新包含、预览不记历史通过。
6. JSON 参数错误：stdout 为空 → 稳定 JSON invalid_arguments 通过。
7. 取消：进程被信号直接结束 → cancelled 历史和不交付产物通过。
8. 安全相对链接：恢复拒绝所有链接 → 保留安全链接、拒绝逃逸目标。

最终复核：格式、Clippy、全套 CLI 测试和 Standards / Spec 两个独立评审。不要把三个操作系统的 CI 配置当作三平台实测结果。

交付记录：`docs/REVIEW.md`。21 项公开 CLI 集成测试、release 构建和真实演示通过；双轴评审发现均修复并关闭。
