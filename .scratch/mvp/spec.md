# Smart Backup CLI MVP

Status: done

## Problem Statement

项目只有规格，没有可实际执行和验证的备份软件。用户要求配置 Matt skills 并快速交付第一版 CLI 原型，删除会引起误解的 BAT。

## Solution

Rust Core 与 CLI 提供创建 Job → 预览 → 不可变 7z 备份 → 验证 → 浏览 → 恢复 → 历史的真实闭环。复用 Task.md T01–T08，原型能力边界必须明确，不能把未完成的正式发行要求标为完成。

## User Stories

1. 用户可创建带 UUID 的多来源 Job，以别名组织归档。
2. 用户可查看、修改、启用、禁用和删除 Job，删除计划不删除备份。
3. 用户可在打包前预览输入、过滤和字节数。
4. 用户可创建独立归档及 SHA-256，不覆盖既有文件。
5. 用户可跳过未变化内容或强制备份。
6. 用户可验证归档并浏览其文件。
7. 用户可恢复到新的目录且来源不受影响。
8. 用户可查询成功、失败和跳过记录。
9. 自动化程序可使用稳定 JSON 和退出码。
10. 卷身份不符、输出递归、危险路径和未支持功能必须明确失败。

## Implementation Decisions

Rust workspace；Core 隐藏引擎/扫描/状态/提交细节，CLI 只解析和呈现。TOML Job，SQLite 历史和基线，JSONL 事件，7zz 子进程参数数组。仅 full 验证、7z、全量恢复。加密默认不可用；正式发行的完整能力以 Task.md 为准。

## Testing Decisions

已确认的唯一测试 seam 是 CLI。真实 7zz、真实临时文件、通过命令读取 Job/Run/归档，恢复后与已知原文比较。不 mock 内部模块。逐个用户行为红→绿，最后格式/lint/全套测试和双轴 review。

## Out of Scope

GUI、scheduler、加密、其他格式、分卷、保留删除、云备份和系统热快照。未通过三平台实测不宣称正式发行完成。

## Further Notes

评审基线 bb03e6a7002c124b12852897602f462ecd450c7f。任务依据 Task.md 第22节，用户已确认本地 tracker 和 CLI 路线。
