---
name: creditia
description: 给人和 AI agent 一起做出的作品署名。生成 CREDITS.md 和 credits.json，导出 tier、片尾字幕、徽章等展示；需要用户决定的事（口径、风格、公开哪些内容）必须停下来问。
---

# creditia

用户说"给 X 做个 credits""加个徽章""更新一下署名"时用这个 skill。

## 原则

- 全程用 CLI，每步都加 `--json`，读返回值再决定下一步。
- **公开任何内容前必须让用户确认**：写入 `CREDITS.md`、改官网、改 README、提交之前，列出将要公开的字段（项目名、PR 标题、人名、文件名），用户说可以才做。`--yes` 只跳过确认提示，不跳过隐私过滤。
- 不替用户选口径、风格、是否显示主导者。列出选项，让用户选。
- 不修改 `~/.codex`、`~/.claude`、`~/.cursor` 等任何 agent 数据。

## 流程

1. **找项目**：`creditia discover --json`。用户没指定项目时，把列表给用户选；指定了就直接用。
2. **初始化**（仓库里还没有 `.creditia/config.json` 时）：`creditia init --json`。把识别出的主导者和邮箱念给用户确认，不对就让用户改配置文件。
3. **分析**：`creditia analyze --github --json`。汇报：各方占比、未确认比例、证据覆盖（哪段时间只有 git 信号）。未确认超过 20% 时，告诉用户原因（通常是日志缺失或 PR 没有分支名），问要不要先补 `gh auth login` 或接受现状。
4. **问用户要决定的事**：
   - 口径：按工具（默认）/ 按模型
   - 风格：Minimal / Film Credits / Terminal / Glass / Editorial / Mono
   - 是否显示主导者署名（默认不显示）
   - 私有仓库：是否公开 PR 标题、文件名
5. **预览**：`creditia app open --form tier --theme <风格>`，让用户在 App 里看；没有 App 时 `creditia export --form tier --format png --out /tmp/tier.png` 给用户看图。
6. **产出**：
   - 档案：`creditia doc --json`（写 `CREDITS.md`，只改标记区间）
   - 嵌入：`creditia export --form tier --format embed --json` 得到 `<creditia-card>` 片段
   - 徽章：`creditia export --form badge --format svg --out assets/creditia-badge.svg`
   - 其他形式按需：`roll`、`receipt`、`share`、`waffle` 等，`creditia export --list` 看全部
7. **接入代码**：改官网 / README 时，先 diff 给用户看，确认后再写。
8. **确认后提交**：列出将公开的内容，用户同意后再 `git add` / `git commit`。
9. **可选**：`creditia hook install`，以后每次提交自动盖章，不再依赖日志。

## 常用命令

```
creditia discover --json
creditia init --json
creditia analyze --github --json
creditia update --json
creditia doc --json
creditia release-notes v1.0.0
creditia export --list
creditia export --form tier --theme minimal --format embed
creditia export --form roll --theme film --format png --out roll.png
creditia blame src/foo.ts:42 --json
creditia doctor --json
creditia hook install
creditia stamp --harness <当前工具> --model <模型>
```

## 自己也要盖章

你作为 agent 给仓库提交时，提交前运行 `creditia stamp --harness <你是哪个工具> --model <你的模型>`，这样这次提交会带上 `Creditia-Harness` trailer。
