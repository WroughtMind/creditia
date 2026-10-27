---
name: creditia
description: 分析一个仓库的代码是哪些 AI 工具和模型写的：读本机 agent 日志与 git 历史（不调用大模型），生成 CREDITS.md、credits.json，导出署名榜、片尾字幕、徽章、统计图等（PNG / SVG / PDF / GIF / 可交互 HTML）。公开任何内容前必须让用户确认。 Find out which AI tools and models wrote a repo's code from local agent logs and git history (no model calls), then write CREDITS.md / credits.json and export credits, end credits, badges and charts. Always confirm with the user before publishing anything.
---

# creditia

用户说「给 X 做个 credits」「加个徽章」「更新署名」「这个项目哪些是 AI 写的」时用这个 skill；英文里是 "make credits for X"、"add a credits badge"、"update the credits"、"which AI wrote this repo"。

## 原则

- 全程用 CLI，每步加 `--json`，读返回值再决定下一步。退出码 0 成功、1 失败、2 有警告（警告要转告用户）。
- **公开前必须确认**：写 `CREDITS.md`、改官网 / README、提交之前，列出将公开的内容（项目名、PR 标题、文件名），用户同意才做。
- **人名**：作品里写工具和模型；别人的提交默认写他们的 git 名字。用户不想写名字：这次加 `--no-names`，一直不写就在配置里设 `display.hideNames`，名字合成一行「其他贡献者」。不要替用户给人加头衔，也不要说哪些行是谁手写的：没有 AI 记录的行只记「未确认」。
- 口径（strict / standard / loose）、风格、公开范围都是用户的决定：列出选项和区别，让用户选。
- 只读各工具的数据，不修改 `~/.codex`、`~/.claude`、`~/.cursor`、`~/.gemini` 等任何目录。

## 流程

1. **找项目**：`creditia discover --json`。用户没指定就把列表给用户选。
2. **初始化**（还没有 `.creditia/config.json` 时）：`creditia init --json`。把识别出的「你的提交身份」（邮箱）念给用户确认——它只用来区分「用户自己的提交」和「别人的提交」。
3. **多处路径**：项目如果在别的目录、别的电脑上也做过（worktree、临时克隆、旧路径），`creditia paths --json` 看已登记的，`creditia link <路径> --json` 补上。别的电脑、云端会话里写的代码，日志不在这台电脑上：在那台电脑上 `creditia evidence export --out ev.json`，拿过来 `creditia evidence import ev.json --json` 再分析；或者在那台电脑上也开提交盖章（见第 5 步）。`analyze` 提示有提交是在别处写的时，把这两种办法告诉用户。
4. **分析**：`creditia analyze --github --json`。汇报各工具占比、未确认比例、逐行证据覆盖。有未确认贡献时，用 `creditia doctor --json` 和 `creditia agents --json` 核对实际来源与读取范围；只说明有证据的原因，不把缺失记录猜成日志被清理，也不靠切换口径掩盖证据不足。
5. **问两项接入**（`init` / `analyze` 的返回里有 `ask` 时必须做，不能替用户决定，也不能跳过）：`ask` 里列的是这个项目还没决定的项。逐项用一两句话讲清楚它会改什么，问用户要不要：
   - `hook`（提交盖章）：在仓库的 git `prepare-commit-msg` hook 里加一行。之后在 agent 里提交时，提交信息末尾自动加 `Creditia-Harness` / `Creditia-Model`；有 AI 代码时再加 `Creditia-Lines`（各工具行数，不含代码）。写入 Git 提交，无需依赖本地日志，PR 评论和 Release 说明也读它。只影响这个仓库。要：`creditia hook install --json`；不要：`creditia hook uninstall --json`
   - `release`（跟着发版）：写 `.github/workflows/creditia.yml`，需要用户自己提交。发 Release 时把这一版的署名写进说明，PR 上评论这个 PR 的署名；读的是提交盖章记下的行数，仓库里有 `credits.json` 时优先用它，所以最好和提交盖章一起开。要：`creditia release install --json`；不要：`creditia release uninstall --json`

   两种回答都要执行对应命令：答案会记进 `.creditia/config.json`，以后不再问。随时可以改，`creditia integrations --json` 看现状；App 里每个项目也有这两个开关。

   **定时收记录**：`creditia schedule status --json`。没开时告诉用户：工具会自动清理旧日志（Claude Code 默认 30 天），清理前没收进作品库的证据就没了；开了以后每小时把各项目的新记录收进来。它在这台电脑上装一个定时任务（macOS launchd、Linux systemd 用户定时器、Windows 计划任务），管所有项目，不只这一个仓库。用户同意才 `creditia schedule install --json`，返回里的 `notes`（没有 systemd 时改用的 cron 行、服务器上要 `loginctl enable-linger`）转告用户；关掉是 `creditia schedule uninstall --json`。
6. **选样子**：`creditia export --list --json` 看全部卡片、版式、风格。推荐几种给用户，用户选。
7. **预览**：导出 PNG 给用户看，例如 `creditia export --piece tier --style ink --format png --out /tmp/tier.png --json`。
8. **产出**（用户确认后）：
   - 档案：`creditia doc --json`
   - 徽章：`creditia export --piece badge --format svg --out assets/credits-badge.svg`
   - 分享图：`creditia export --piece share --style glass --format png --size og --out assets/credits.png`
   - 片尾字幕：`creditia export --piece roll --variant slate --format png --out credits.png`（版式见 `--list`；`--format gif` 出动画，`--list` 里标了哪些卡片会动）
   - 全部卡片一页：`creditia board --style minimal --out credits.html`
   - 放到 GitHub：先 `creditia publish --check --json`（只检查不写）：把 `categories` 里将公开的每一类（标题、文件路径、人名、工作时间、仓库地址，带样例）和 `risks` 里看起来不该公开的内容念给用户，逐类问要不要公开。再按回答执行 `creditia publish --hide <不公开的类别，逗号分隔> --json`；`risks` 用户看过、确认没问题时才加 `--yes`。之后按 README「放到 GitHub 上」加 workflow
9. **接入**：改官网 / README 时先给用户看 diff，确认后再写。

## 给自己盖章

你作为 agent 提交代码前：

```
creditia agents --json                     # 查工具 ID 与历史读取状态
creditia whoami --json                     # 确认当前工具身份
creditia stamp --agent <工具 ID>             # 模型没有明确记录时不填写
```

提交盖章 hook 已安装时，下一次提交会带上 `Creditia-Harness`；只有明确知道本次执行模型时才附加 `--model <模型>`。这是提交声明，不能冒充逐行编辑证据，也不能反推出过去使用的模型。`stamp` 本身不安装 hook。

未被环境自动识别的命令行 agent，可用 `creditia run --agent <工具 ID> -- <原启动命令>` 启动，子进程会继承明确身份。该命令不调用模型、不安装 hook，不为历史补造贡献。支持 ACP 的工具还可使用 `creditia capture --agent <工具 ID> -- <原 ACP 启动命令>` 接入客户端；仅有成功编辑差分的后续会话才产生逐行证据。

## 常用命令

用户用英文时，每条命令加 `--lang en`（或设 `CREDITIA_LANG=en`），提醒和摘要就都是英文。

```
creditia discover --json
creditia init --json
creditia analyze --github --json
creditia analyze --level strict --json
creditia doc --json
creditia doc --by contributor --json
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
creditia schedule install|uninstall|status --json
creditia evidence export --out ev.json --json
creditia evidence import ev.json --json
```
