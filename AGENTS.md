## Learned User Preferences

- 产品决策（平台、品牌、命名、视觉、范围）以用户为准；列出选项和利弊，不要自行写成默认方案。
- 不要把 creditia 的视觉或 logo 做成魏碑的姊妹款，除非用户要求。WroughtMind 是 GitHub 组织，不是视觉品牌家族。
- 展示要对标魏碑 Credits 面板的一眼能懂；不要做成带大量开关或多视图的数据看板。
- 不要把缺失的 agent 日志归因成工具清理；别的用户可能数据齐全。只有代码或文档里确认过的保留规则才能写成事实。
- 判断某数据源“无法归因模型”之前，先查全本地存储。Cursor 有 `~/.cursor/ai-tracking/ai-code-tracking.db`。
- 人作为主导者单独署名，不要混进 agent 的百分比条。
- 魏碑只是 creditia 的测试数据，两者没有产品关系：不要为 creditia 改魏碑的任何东西，也不要把魏碑当成 creditia 的案例。
- 发布（WroughtMind 开源、npm、Releases）等用户整体验收通过后由用户发起。

## Learned Workspace Facts

- creditia 是 WroughtMind 组织在魏碑之后的产品：结构化精力档案，加上多种一眼能懂的展示形式。
- 目标平台是 macOS、Windows、Linux；入口是 CLI、App 和 agent skill；协议 MIT。
- 核心产出是 `CREDITS.md` 与 `credits.json`。所有展示形式读同一份档案；每种形式只回答一个问题（tier、片尾字幕、精力小票、统计图等）。
- 展示用各家真实 logo 和熟悉品牌色便于识别；creditia 自己的 logo 独立于魏碑。
- 技术栈方向：Tauri 2 + Rust 核心 + TypeScript 渲染器；本机可以承担 Rust 开发。
