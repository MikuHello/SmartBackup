# 独立快照与 CLI 优先

沿用 Task.md 的既定决策：Rust Core 生成可独立恢复的标准 7z Artifact，CLI 先于 GUI。每次完整快照增加空间和 I/O 成本，但避免增量链依赖并支持脱离应用恢复。历史 BAT 已删除，不继承原地更新归档行为。
