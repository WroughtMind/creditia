---
name: creditia
description: 分析一个仓库的代码是哪些 AI 工具和模型写的：读本机 agent 日志与 git 历史（不调用大模型），生成 CREDITS.md、credits.json，导出署名榜、片尾字幕、徽章、统计图等（PNG / SVG / PDF / GIF / 可交互 HTML）。公开任何内容前必须让用户确认。 Find out which AI tools and models wrote a repo's code from local agent logs and git history (no model calls), then write CREDITS.md / credits.json and export credits, end credits, badges and charts. Always confirm with the user before publishing anything.
---

# creditia

用户说「给 X 做个 credits」「加个徽章」「更新署名」「这个项目哪些是 AI 写的」时用这个 skill；英文里是 "make credits for X"、"add a credits badge"、"update the credits"、"which AI wrote this repo"。

## 原则

- 全程用 CLI，每步加 `--json`，读返回值再决定下一步。退出码 0 成功、1 失败、2 有警告（警告要转告用户）。
- **公开前必须确认**：写 `CREDITS.md`、改官网 / README、提交之前，列出将公开的内容（项目名、PR 标题、文件名），用户同意才做。
- **只写 AI**：作品里只有工具和模型，不写人名，也不要替用户给人加头衔。
- 口径（strict / standard / loose）、风格、公开范围都是用户的决定：列出选项和区别，让用户选。
- 只读各工具的数据，不修改 `~/.codex`、`~/.claude`、`~/.cursor`、`~/.gemini` 等任何目录。

## 流程

1. **找项目**：`creditia discover --json`。用户没指定就把列表给用户选。
2. **初始化**（还没有 `.creditia/config.json` 时）：`creditia init --json`。把识别出的「你的提交身份」（邮箱）念给用户确认——它只用来区分「用户自己的提交」和「别人的提交」。
3. **多处路径**：项目如果在别的目录、别的电脑上也做过（worktree、临时克隆、旧路径），`creditia paths --json` 看已登记的，`creditia link <路径> --json` 补上。
4. **分析**：`creditia analyze --github --json`。汇报各工具占比、未确认比例、逐行证据覆盖。未确认超过 20% 时说明原因（日志被清理、工具不在支持范围、提交里没有声明），问用户要不要换口径或接受现状。
5. **问两项接入**（`init` / `analyze` 的返回里有 `ask` 时必须做，不能替用户决定，也不能跳过）：`ask` 里列的是这个项目还没决定的项。逐项用一两句话讲清楚它会改什么，问用户要不要：
   - `hook`（提交盖章）：在仓库的 git `prepare-commit-msg` hook 里加一行。之后在 agent 里提交时，提交信息末尾自动加 `Creditia-Harness` / `Creditia-Model`；提交里有 AI 写的行时再加 `Creditia-Lines`（各工具写了几行，不含代码）。本机日志清理后也认得出，PR 评论和 Release 说明也读它。只影响这个仓库。要：`creditia hook install --json`；不要：`creditia hook uninstall --json`
   - `release`（跟着发版）：写 `.github/workflows/creditia.yml`，需要用户自己提交。发 Release 时把这一版的署名写进说明，PR 上评论这个 PR 的署名；读的是提交盖章记下的行数，仓库里有 `credits.json` 时优先用它，所以最好和提交盖章一起开。要：`creditia release install --json`；不要：`creditia release uninstall --json`

   两种回答都要执行对应命令：答案会记进 `.creditia/config.json`，以后不再问。随时可以改，`creditia integrations --json` 看现状；App 里每个项目也有这两个开关。
6. **选样子**：`creditia export --list --json` 看全部作品、版式、风格。推荐几种给用户，用户选。
7. **预览**：导出 PNG 给用户看，例如 `creditia export --piece tier --style ink --format png --out /tmp/tier.png --json`。
8. **产出**（用户确认后）：
   - 档案：`creditia doc --json`
   - 徽章：`creditia export --piece badge --format svg --out assets/credits-badge.svg`
   - 分享图：`creditia export --piece share --style glass --format png --size og --out assets/credits.png`
   - 片尾字幕：`creditia export --piece roll --variant slate --format png --out credits.png`（版式见 `--list`；`--format gif` 出动画，`--list` 里标了哪些作品会动）
   - 全部作品一页：`creditia board --style minimal --out credits.html`
   - 放到 GitHub：先 `creditia publish --check --json`（只检查不写）：把 `categories` 里将公开的每一类（标题、文件路径、人名、工作时间、仓库地址，带样例）和 `risks` 里看起来不该公开的内容念给用户，逐类问要不要公开。再按回答执行 `creditia publish --hide <不公开的类别，逗号分隔> --json`；`risks` 用户看过、确认没问题时才加 `--yes`。之后按 README「放到 GitHub 上」加 workflow
9. **接入**：改官网 / README 时先给用户看 diff，确认后再写。

## 给自己盖章

你作为 agent 提交代码前：

```
creditia whoami --json                      # 确认自己是哪个工具
creditia stamp --harness <工具> --model <模型>
```

这样这次提交会带上 `Creditia-Harness` / `Creditia-Model`，以后即使日志被清理也能确认是你写的。

## 常用命令

用户用英文时，每条命令加 `--lang en`（或设 `CREDITIA_LANG=en`），提醒和摘要就都是英文。

```
creditia discover --json
creditia init --json
creditia analyze --github --json
creditia analyze --level strict --json
creditia doc --json
creditia export --list --json
creditia export --piece roll --style ukiyoe --variant vertical --format png --out roll.png
creditia export --piece tier --style cyber --format gif --out tier.gif
creditia board --style glass --out credits.html
creditia blame src/foo.ts:42 --json
creditia paths --json
creditia link /path/on/another/machine --json
creditia doctor --json
creditia integrations --json
creditia hook install|uninstall|status --json
creditia release install|uninstall|status --json
```
