# SmartBackup Review Handoff

把下面的提示词交给在项目目录 `/Users/mikuhello/project/dev/SmartBackup` 打开的新会话。主线提交已按工作流、旧原型移除、CLI 垂直切片、CI、各项修复及其回归测试、文档、品牌改名分开；旧历史保留在本地 `backup/pre-smartbackup-atomic-history`。

```text
请对 SmartBackup 当前实现执行完整的双轴 code review。

工作目录：/Users/mikuhello/project/dev/SmartBackup
固定基线：bb03e6a7002c124b12852897602f462ecd450c7f
范围：基线之后到本轮开始时 HEAD 的全部提交，不只审查最后一次改名。
规格：Task.md 是完整目标；.scratch/mvp/spec.md 与 .scratch/mvp/issues/ 是已交付 CLI alpha 的任务范围；docs/MVP.md 声明原型支持与限制。

1. 读取 AGENTS.md、.agents/skills/code-review/SKILL.md、docs/agents/workflow.md、docs/agents/issue-tracker.md、GLOSSARY.md 与 docs/adr/。记录 HEAD SHA，确认固定基线存在、工作区状态。若有未提交改动，单独列明，不混进已提交范围。
2. 使用以下完整命令建立审查输入：
   git diff bb03e6a7002c124b12852897602f462ecd450c7f...HEAD
   git log --reverse --oneline bb03e6a7002c124b12852897602f462ecd450c7f..HEAD
   确认 diff 非空。docs/REVIEW.md 只作为既往发现与验证记录，逐项复核证据，不能替代本轮独立审查。
3. 明确授权按 code-review skill 启动恰好两个独立、只读子智能体：Standards 与 Spec。分别传递完整 diff 命令、提交列表和对应标准/规格；Standards 还须携带 skill 的完整 smell baseline。除这两个评审外，不额外委派。
4. 重点核查来源只读、不可变 Artifact、失败关闭的加密行为、恢复路径 containment、符号链接/碰撞/穿越、输出递归/卷身份、引擎哈希、过滤优先级、Dry Run 副作用、取消和中断提交。核查代码、文档及公开 CLI 的行为是否一致。
5. 区分三类结果：alpha 已承诺但实现错误或缺失；已声明的正式版待办；可能使安全不变量失效的限制。已声明限制不能自动豁免安全缺陷，也不能把全部正式版待办混报为 alpha 回归。保持 Task.md 要求原样，不静默降低标准。
6. 在隔离临时样例上验证，使用真实 7-Zip 和公开 CLI JSON，禁止使用个人文件或删除现有归档。先检查 scripts/test-engines.lock.json 和 scripts/install-test-engine.py；已有 .ci/engine/7zz 时验证其 SHA-256 与当前平台锁定值一致，否则运行安装脚本取得锁定引擎。在当前 macOS 环境执行：
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   SMART_BACKUP_7ZZ="$PWD/.ci/engine/7zz" cargo test --workspace --locked
   cargo build --release --locked
   SMART_BACKUP_7ZZ="$PWD/.ci/engine/7zz" python3 scripts/demo.py
   记录实际执行的平台、命令及结果；依赖或环境阻塞须明确报告。核查来源样例字节在备份和恢复后不变。不要把其他平台的 CI 配置当作已通过的实测。
7. 分别呈现 Standards / Spec 报告，不合并两轴排名。每条可执行发现给出严重度、文件和行号、触发条件、影响、规格或标准依据、已有复现证据或建议复现步骤。代码味道明确标为判断建议。每轴报告遵守 skill 字数限制，并分别统计发现数量与最高严重度。

本轮任务是审查和报告。保持实现与原有评审记录不变，不自动修复、提交、推送或发布。给出通过的检查、未验证范围和仍需主人决策的事项；没有发现时明确说明审查范围与剩余风险。
```
