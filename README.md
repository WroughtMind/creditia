<p align="center"><img src="packages/render/assets/creditia/logo.svg" width="56" alt=""></p>

# creditia

> 为人和 AI 一起做出的作品署名。
> Credits for work made by people and agents together.

**状态：开发中。**

## 做什么

现在很多项目的代码是好几个 AI 工具、好几个模型一起写的。creditia 读你电脑上的 git 历史和这些工具留下的日志，算出每一行是哪个工具、哪个模型写的，再把结果做成能拿出去的东西：README 徽章、CREDITS.md、片尾字幕式的署名图、分享图、统计图，或者一个能放在项目网站上的署名页。

分析在本机完成，不上传代码，也不调用任何大模型。

## 结果靠得住吗

- 一行代码如果在某个工具的日志里能找到（工具确实把这一行写进了文件），就算有逐行证据。
- 没有日志的，看提交里的信息：提交邮箱、`Co-authored-by`、creditia 的提交盖章。
- 两样都没有的，默认会做两种推断：同一个提交里其余的行跟着有证据的那个工具走；`codex/` 这类工具自动建的分支算那个工具。推断单独记，在图上和档案里都能看出来。不想要推断，用 `--level strict`。
- 还是对不上的，记为「未确认」，不算到任何工具或人头上。

完整规则和已知的边界在 [ATTRIBUTION.md](ATTRIBUTION.md)。

## 人在哪里

creditia 主要回答「哪些 AI 工具和模型参与了、各写了多少」。人的工作没法用行数衡量，所以单独放几个数字：发给 AI 多少条消息、review 多少次、人写了多少行、AI 写的代码提交了多少、现在还剩多少。人名默认不写，要写名字或者落款，在设置里打开。

## 支持的工具

| 工具 | 读什么 | 逐行证据 | 模型 | 渠道 |
|---|---|:-:|:-:|:-:|
| Claude Code | `~/.claude/projects/*/*.jsonl` 的 Write / Edit / MultiEdit | ✓ | ✓ | Bedrock、Vertex、改写响应的中转站 |
| Codex | `~/.codex` 线程库 + rollout 的 FileChange | ✓ | ✓ | `model_provider` |
| Cursor | `ai-code-tracking.db` 逐行哈希（与 Cursor 自己的算法逐字节一致） | ✓ | ✓ | — |
| Gemini CLI / Qwen Code | `~/.gemini/tmp/<项目哈希>/chats`（成功的 write / replace） | ✓ | ✓ | — |
| OpenCode | `storage/session + message + part`（completed 的 edit / write） | ✓ | ✓ | `providerID`（含自定义中转站） |
| Cline / Roo Code / Kilo Code | 各 VS Code 系编辑器里的 `tasks/*/ui_messages.json` | ✓ | ✓ | `model_provider_id` |
| Grok、Kimi | 会话目录里的写文件调用 | ✓ | ✓ | — |
| GitHub Copilot、Devin、Jules、Aider、Cursor 云端、bot | 提交邮箱、Co-authored-by、作者名 | 声明 | — | — |
| 任何工具 | `creditia stamp` / 提交 hook 盖章（`Creditia-Harness` trailer） | 声明 | ✓ | — |

## 快速上手

```bash
cd 你的仓库
npx creditia analyze                # 分析（第一次会下载对应平台的分析程序）
npx creditia doc                    # 写 CREDITS.md
npx creditia export --piece tier --style ink --format png --out tier.png
npx creditia export --list          # 全部作品、版式、风格、格式
npx creditia hook install           # 以后每次提交自动盖章：日志被清理了也认得出是哪个工具写的
```

一般不用自己敲这些：让你的 agent 用 [skills/creditia/SKILL.md](skills/creditia/SKILL.md)，它会按步骤做，公开任何内容之前先问你。

一个项目在好几个地方（worktree、临时克隆、换过电脑、搬过家）：`creditia paths` 看登记过的路径，`creditia link <路径>` 补上。

## 放到 GitHub 上

CI 里看不到任何人的 agent 日志，所以分析在本机做，CI 只负责把结果做成徽章、CREDITS.md 和署名页：

```bash
npx creditia publish                # 把档案写到仓库根目录的 credits.json，并列出将公开的内容
git add credits.json && git commit -m "credits"
```

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

每次推送，署名页就跟着更新。也可以只生成徽章（`badge: assets/credits-badge.svg`）或 CREDITS.md（`doc: CREDITS.md`）。

## App

macOS 的 App 目前没有签名（还没有开发者证书）：第一次打开时右键 → 打开，或者在终端运行 `xattr -cr /Applications/creditia.app`。

一面展墙：顶部切换项目，左下角的小圆点切换风格（悬停看风格名，每个圆点就是这个风格的缩影），所有作品直接摆出来。悬停作品会出现虚线框——换版式、选格式导出；作品里的每一块都能悬停看细节；把作品拖到右下角的导出坞也能导出，拖的时候其余作品会虚化。

## 风格

极简、暗室、液态玻璃 · 水墨、浮世绘、古籍 · 黄金时代、巴洛克、魔典 · 手稿、对称、剪纸、夏日 · 漫画、赛博、终端。
每个风格是一套完整的视觉语言（底、框、标题、条形、标记、分隔），不是换色；有的作品在某些风格下有专属版式（古籍的竖排、漫画的能力面板、赛博的 HUD、动画片尾……）。

## 仓库结构

```
crates/
  core/       归因引擎、行哈希、模型目录（名称归一、厂商、渠道）、配置
  adapters/   git 与各家 agent 日志的读取
  store/      作品库：项目、路径、证据库、快照（SQLite）
  cli/        二进制 creditia-core
packages/
  schema/     credits.json v0 的 JSON Schema 与类型
  render/     作品渲染：credits.json → SVG 场景（风格 × 作品）→ PNG / SVG / PDF / GIF / HTML
  cli/        npm 包 creditia：档案、导出、终端名片
  embed/      <creditia-card> 网页组件
apps/desktop/ Tauri 2 App（展墙与导出）
skills/       给 agent 用的 skill
spike/        阶段 0 验证工程（保留作记录）
```

## 开发

```bash
cargo build --release && cargo test           # Rust 核心（端到端测试用临时 HOME 里的假日志）
cd packages/render && npm i && npm test       # 渲染（全部作品 × 全部风格 × 全部版式）
npm run gallery                               # 样张：gallery/index.html
cd apps/desktop && npm i && npm run fonts && npm run dev   # 浏览器里调 App（用 fixtures 里的样例档案）
npm run core && npm run tauri dev             # 原生 App（core 把分析核心放进 App 的打包位置）
```

## 协议

MIT，见 [LICENSE](LICENSE)。logo 来自 simple-icons（CC0），仅用于识别对应工具。
