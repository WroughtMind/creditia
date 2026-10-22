<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/logo-dark.svg">
  <img src=".github/readme/logo-light.svg" width="52" alt="">
</picture>

# creditia

**你的代码，是哪些 AI 写的。**

读本机的 git 历史和 agent 日志，把每一行对上写它的工具和模型，<br>
再做成徽章、CREDITS.md、片尾字幕和分享图。全程本地，不调用任何模型。

[![built with](https://wroughtmind.github.io/creditia/assets/usage/zh-self-badge.svg)](https://wroughtmind.github.io/creditia/self/)

[官网](https://wroughtmind.github.io/creditia/) · [安装](#安装) · [归因规则](ATTRIBUTION.md) · [English](README.en.md)

<a href="https://wroughtmind.github.io/creditia/promo.mp4"><img src="site/promo.jpg" width="720" alt="播放 creditia 宣传片（52 秒，有声）"></a>

</div>

<br>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/map-dark.svg">
  <img src=".github/readme/map-light.svg" alt="样例项目 catnap 的代码地图：一个文件一段，一行一道线，颜色是写它的 AI">
</picture>

## 一行命令

```text
$ creditia analyze && creditia fetch
catnap  2026-05-11 → 2026-09-28
────────────────────────────────
Claude Code     █████████████░░░░░░░░░░░░  50.9%  Claude Opus 5.5
Codex           ████████░░░░░░░░░░░░░░░░░  30.8%  GPT-5.6 Sol
Cursor          ██░░░░░░░░░░░░░░░░░░░░░░░   9.1%  Composer 2
Kimi            █░░░░░░░░░░░░░░░░░░░░░░░░   4.7%  Kimi K3
Grok            █░░░░░░░░░░░░░░░░░░░░░░░░   2.1%  Grok 5 Code
未确认          █░░░░░░░░░░░░░░░░░░░░░░░░   2.5%

提交 734  行 50,190  token 447M
```

分析完，结果可以拿去用：

```bash
creditia doc                                  # 写 CREDITS.md（只改标记区间）
creditia export --piece roll --out roll.png   # 片尾字幕
creditia export --list                        # 全部卡片、版式、风格、格式
creditia hook install                         # 以后每次提交自动盖章：写入 Git 提交，无需依赖本地日志
creditia release install                      # 跟着发版：Release 说明、PR 评论里写上署名
creditia blame src/core/analyze.ts:12         # 这一行是哪个工具、哪个模型写的
```

一个项目在好几个地方（worktree、临时克隆、换过电脑）：`creditia paths` 看登记过的路径，`creditia link <路径>` 补上。

作品默认按工具分行；一个工具写了绝大部分、下面又换过几个模型时，自动改成按模型分。`--group tool` / `--group model` 指定，App 顶栏也能切换、随时切回自动。一个工具里有几个模型时，按模型分的那几行会挨在一起、用同一品牌色的深浅区分；另外每类作品都有把工具和它的模型分层画出来的版式：条形图「分段」「分组」、环形图「内外环」、片尾字幕「分部」、小票「明细」、华夫饼「分块」、徽章「分段」、署名行「带模型」（App 里这些版式旁边有个小点）。

提交盖章和跟着发版都会改你的仓库，所以每个项目由你决定：`init` / `analyze` 在终端里会问一次（答案记进 `.creditia/config.json`，不再问），App 里每个项目的作品墙最上面也有这两个开关，随时能拨回去。`creditia integrations` 看现状。

## 每一行，都有出处

- **日志里对得上**　工具日志里有这一行的写入：Claude Code 的 Write / Edit、Codex 的 FileChange、Cursor 的逐行哈希……
- **提交里写明了**　提交邮箱、`Co-authored-by`、`Assisted-by`、creditia 的提交盖章
- **推断**　同一段改动里大半有某个工具的逐行证据，其余的行跟着它走；`codex/` 这类工具自动建的分支。单独记，`--level strict` 关掉
- **未确认**　以上都对不上的行。不算给任何工具

creditia 只回答「哪些 AI 工具和模型写了多少」：作品里不出现任何名字，百分比只在 AI 和「未确认」的行之间算。完整规则和已知的边界在 [ATTRIBUTION.md](ATTRIBUTION.md)。

## 二十六种样子

<img src=".github/readme/styles.png" alt="同一份署名，不同风格：影院片尾、浮世绘分享图、古籍竖排、漫画能力面板、赛博 HUD、霓虹环形图、报纸版本柱、魔典电影票、终端名片、巴洛克领奖台">

同一份档案，二十多种作品（署名榜、分享图、片尾字幕、代码地图、出处、板块、小票、徽章、时间织带、作息图……）× 二十六个风格，出 PNG、SVG、PDF、GIF、HTML。每个风格是一整套视觉语言，不是换色；有的作品在某些风格下有专属版式：古籍的竖排、漫画的能力面板、赛博的 HUD、片尾字幕的十几种排法。

> 极简 · 极简暗 · 玻璃 · 胶片 · 影院 · 杂志 · 报纸 · 节目单 · 颁奖礼 · 单色 · 海报 · 终端 · 磷光屏 · 霓虹 · 街机<br>
> 水墨 · 浮世绘 · 古籍 · 巴洛克 · 魔典 · 手稿 · 对称 · 剪纸 · 夏日 · 漫画 · 赛博

全部摆在一起看：[官网的风格墙](https://wroughtmind.github.io/creditia/#looks)。

## 四种用法

**命令行**　就是上面那些。所有命令都支持 `--json`。

**交给 agent**　把 [skills/creditia](skills/creditia/SKILL.md) 装进你的 agent，跟它说「看看这个仓库是哪些 AI 写的」。它会按步骤分析、出图，公开或提交任何东西之前先问你。

**跨项目汇总**　本地项目库里有几个项目时，App 项目菜单最上面多一项「全部项目」：各项目的档案并成一份，整面墙照常画。命令行是 `creditia summary`（印一张名片，`--out all.json` 写成一份档案），任何命令加 `--all-projects` 也一样，比如 `creditia export --all-projects --piece tier`。行数相加；版本是各项目自己的，不合并；文件和 PR 前面带上项目名。

**公开前的隐私检查**　`creditia publish` 写出 credits.json 之前，先列出会公开的每一类（PR / 提交标题、文件路径、人名、工作时间、仓库地址）和几个样例，并找出看起来不该公开的东西（像密钥的字符串、邮箱、本机路径、内网地址、带账号密码的地址）。在终端里逐类问你；有可疑内容的那一类默认不公开。`--check` 只检查不写，`--hide titles,paths` 之类直接隐藏；在脚本里遇到可疑内容时不写，要 `--yes` 确认。隐藏只改公开的这一份，本机档案不动。

**GitHub Action**　CI 里看不到任何 agent 的日志，所以分析在本机做：`creditia publish` 把档案写到仓库根目录的 `credits.json`，提交上去；之后每次推送，Action 用它重画署名页，也可以顺带生成徽章（`badge:`）、星号（`asterisk:`，各家份额长成的 *）和 CREDITS.md（`doc:`）。发 Release 时把这一版的署名写进说明（`release-notes: true`）、在 PR 上评论这个 PR 的署名并随推送更新（`pr-comment: true`）：`creditia release install` 直接写好这份 workflow。这两项不需要 credits.json：打开提交盖章后，每次提交都会带上一行 `Creditia-Lines`（各工具写了几行，在本机提交时算好，不含代码），CI 读它就够了。

**动态徽章**　署名网站（`site:`）里总会带一份 `badge.json`；发布到 GitHub Pages 后，README 里放 `![credits](https://img.shields.io/endpoint?url=https://<用户>.github.io/<仓库>/badge.json)`，每次推送徽章跟着变。也可以单独生成：`creditia badge-json --out badge.json`，Action 里是 `badge-json:`；没有 credits.json 时它汇总全部提交上的盖章。

```yaml
# .github/workflows/credits.yml
on: { push: { branches: [main] } }
permissions: { contents: read, pages: write, id-token: write }
jobs:
  credits:
    runs-on: ubuntu-latest
    environment: github-pages
    steps:
      - uses: actions/checkout@v4
      - uses: WroughtMind/creditia@main
        with: { site: _site, style: ink }
      - uses: actions/upload-pages-artifact@v3
        with: { path: _site }
      - uses: actions/deploy-pages@v4
```

**App**　一面展墙：顶部切换项目，左下角的圆点切换风格，所有作品直接摆出来。悬停作品换版式、选格式导出，作品里的每一块都能悬停看细节；把作品拖到右下角的导出坞也能导出。[在官网上直接试](https://wroughtmind.github.io/creditia/#use)。

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="site/app-dark.webp">
  <img src="site/app-light.webp" alt="creditia App：展墙上摆着样例项目的全部作品">
</picture>

## 支持的工具

| | 工具 | 读什么 | 逐行 | 模型 |
|:-:|---|---|:-:|:-:|
| <img src=".github/readme/logo/claude-code.svg" width="18"> | Claude Code | `~/.claude/projects/*/*.jsonl` 的 Write / Edit / MultiEdit；渠道含 Bedrock、Vertex、中转站 | ✓ | ✓ |
| <img src=".github/readme/logo/codex.svg" width="18"> | Codex | `~/.codex` 线程库 + rollout 的 FileChange；渠道看 `model_provider` | ✓ | ✓ |
| <img src=".github/readme/logo/cursor.svg" width="18"> | Cursor | `ai-code-tracking.db` 逐行哈希，与 Cursor 自己的算法逐字节一致 | ✓ | ✓ |
| <img src=".github/readme/logo/gemini.svg" width="18"> <img src=".github/readme/logo/qwen.svg" width="18"> | Gemini CLI · Qwen Code | `~/.gemini/tmp/<项目>/chats`（`.json` 与新版 `.jsonl`）里成功的 write / replace；新版 Qwen Code 的 `~/.qwen/projects/*/chats/*.jsonl` | ✓ | ✓ |
| <img src=".github/readme/logo/opencode.svg" width="18"> | OpenCode | `opencode.db`（新版的 SQLite，v1、v2 两代表结构都读）和老版的 `storage/*.json` 里 completed 的 edit / write / patch；渠道看 `providerID` | ✓ | ✓ |
| <img src=".github/readme/logo/cline.svg" width="18"> <img src=".github/readme/logo/roo.svg" width="18"> <img src=".github/readme/logo/kilo.svg" width="18"> | Cline · Roo Code · Kilo Code | 各 VS Code 系编辑器里的 `tasks/*/ui_messages.json`；Kilo 命令行的 `kilo.db`（OpenCode 的分支） | ✓ | ✓ |
| <img src=".github/readme/logo/grok.svg" width="18"> <img src=".github/readme/logo/kimi.svg" width="18"> | Grok · Kimi | 会话目录里的写文件调用 | ✓ | ✓ |
| <img src=".github/readme/logo/copilot.svg" width="18"> | GitHub Copilot | VS Code 的 `chatSessions` 与 agent 记录里成功的编辑；Copilot 命令行的 `~/.copilot/session-state` | ✓ | ✓ |
| <img src=".github/readme/logo/aider.svg" width="18"> | Aider | 仓库里的 `.aider.chat.history.md`：只算「Applied edit」确认过的文件，时间取它的提交 | ✓ | ✓ |
| | Amp · TRAE 命令行 | Amp 本地的 `threads/*.json`（完成的编辑）；TRAE CLI 的会话 rollout | ✓ | ✓ |
| | Goose | `~/.local/share/goose/sessions/sessions.db`：成功的 write / edit / text_editor（配上同 id 的工具结果）和 shell 命令；模型取每次调用的用量记录 | ✓ | ✓ |
| | Amazon Q 命令行 | `amazon-q/data.sqlite3` 的 conversations 表：结果为 Success 的 fs_write 与命令；每个目录只存最近一次对话 | ✓ | ✓ |
| | Antigravity 命令行 | `agy` 的 `~/.gemini/antigravity-cli/brain/*/…/transcript_full.jsonl`：结果为 DONE 的 write_to_file / replace_file_content 与命令（闭源，按第三方观察读） | ✓ | ✓ |
| | Zed | agent 面板的 `threads/threads.db`：edit_file / write_file 成功的结果里带着改前改后的完整内容，两者之差；terminal 命令 | ✓ | ✓ |
| | Continue | `~/.continue/sessions/*.json`（编辑器扩展和 `cn` 命令行共用）：状态为 done 的编辑与命令；被拒绝的 diff 不算 | ✓ | ✓ |
| | Crush | 项目里的 `.crush/crush.db`：`files` 表逐版存的文件内容，相邻两版多出来的行；提交里的 `Assisted-by: Crush:模型` | ✓ | ✓ |
| <img src=".github/readme/logo/devin.svg" width="18"> <img src=".github/readme/logo/jules.svg" width="18"> | Copilot 云端 · Devin · Jules · Cursor 云端 · bot | 提交邮箱、`Co-authored-by`、`Assisted-by`、作者名 | 声明 | — |
| | 任何工具 | `creditia stamp` / 提交 hook 盖章（`Creditia-Harness` trailer） | 声明 | ✓ |

## 安装

需要 Node 22+。npm、Homebrew、一行脚本和 App 安装包即将推出，现在从源码装：

```bash
git clone https://github.com/WroughtMind/creditia && cd creditia/packages/cli && npm i && npm link
# 或者只要 Rust 分析核心
cargo install --git https://github.com/WroughtMind/creditia creditia-cli
```

Agent skill（Claude Code）：

```bash
git clone --depth 1 https://github.com/WroughtMind/creditia /tmp/creditia && cp -r /tmp/creditia/skills/creditia ~/.claude/skills/
```

GitHub Action 直接用 `WroughtMind/creditia@main`，不用装。

命令行默认说中文；加 `--lang en` 或设 `CREDITIA_LANG=en` 改说英文。

大仓库第一次分析要逐个文件追溯现在的每一行，比较慢：一万多个提交、九千个文件的仓库，单核机器上要几十分钟，多核快得多。终端和 App 里都能看到走到哪一步；中途取消，下次接着跑；之后每次只算新提交。

<details>
<summary><b>仓库结构</b></summary>

```
crates/
  core/       归因引擎、行哈希、模型目录（名称归一、厂商、渠道）、配置
  adapters/   git 与各家 agent 日志的读取
  store/      本地项目库：项目、路径、证据库、快照（SQLite）
  cli/        二进制 creditia-core
packages/
  schema/     credits.json v0 的 JSON Schema 与类型
  render/     卡片渲染：credits.json → SVG 场景（风格 × 卡片）→ PNG / SVG / PDF / GIF / HTML
  cli/        npm 包 creditia：档案、导出、终端名片
  embed/      <creditia-card> 网页组件
apps/desktop/ Tauri 2 App（展墙与导出）
site/         官网（build.ts）和这份 README 的图（readme.ts）
skills/       给 agent 用的 skill
spike/        阶段 0 验证工程（保留作记录）
```

</details>

<details>
<summary><b>开发</b></summary>

```bash
cargo build --release && cargo test           # Rust 核心（端到端测试用临时 HOME 里的假日志）
cd packages/render && npm i && npm test       # 渲染（全部作品 × 全部风格 × 全部版式）
npm run gallery                               # 样张：gallery/index.html
cd apps/desktop && npm i && npm run fonts && npm run dev   # 浏览器里调 App（用 fixtures 里的样例档案）
npm run core && npm run tauri dev             # 原生 App（core 把分析核心放进 App 的打包位置）
node site/build.ts                            # 官网 → _site/
node site/readme.ts                           # README 的图 → .github/readme/
```

macOS 的 App 目前没有 Apple 签名：下载的 App 第一次打开可能提示「已损坏，无法打开」。拖进「应用程序」后运行一次 `xattr -dr com.apple.quarantine /Applications/creditia.app`，或者到「系统设置 → 隐私与安全性」里点「仍要打开」。用 Homebrew 装的话，同一行接上就行：`brew install --cask wroughtmind/tap/creditia && xattr -dr com.apple.quarantine /Applications/creditia.app`。Windows 上 SmartScreen 拦下时，点「更多信息」→「仍要运行」。

</details>

## 协议

MIT，见 [LICENSE](LICENSE)。logo 来自 simple-icons（CC0），仅用于识别对应工具。
