<p align="center"><img src="packages/render/assets/creditia/logo.svg" width="56" alt=""></p>

# creditia

> 为人和 AI 一起做出的作品署名。
> Credits for work made by people and agents together.

**状态：开发中。**

## 做什么

读本机的 git 历史和各家 agent 的日志，**逐行**弄清楚：哪段代码是哪个工具、哪个模型、在哪次会话里写下的，经由哪个渠道（官方、云厂商、聚合平台、中转站）。产出一份档案（`credits.json` + `CREDITS.md`），再从同一份档案做出一整面「作品墙」：署名榜、片尾字幕、精力小票、分享图、徽章、人的部分，以及各种统计图，十六种风格，导出 PNG / SVG / PDF / GIF / 可交互的 HTML 单文件。

全程在本机完成，**不调用任何大模型**：归因靠行哈希、时间窗口、提交元数据这些可复查的证据，每一个数字都能追到具体的提交。默认口径会做两种推断（同一提交内的延伸、分支前缀），它们单独记为「推断」；只要有证据的结论，用 `--level strict`。

## 理念

**代码大多是 agent 写的；写什么、留下什么，是人决定的。**

- **agent 按行署名。** 谁写了多少行、多少至今仍在、多少有逐行证据，分工具、分模型列清楚。
- **人不和 agent 抢百分比。** 人的贡献不是行数，是方向和取舍：发出了多少条指令、做了多少次审阅、亲手写了多少行、agent 生成的代码里留下了多少。这些单独成一块「人的部分」。
- **不封头衔。** 没有「由某某主导」。人名默认不出现——要不要写名字、要不要落款，由你决定。
- **诚实。** 证据不够的归「未确认」，不并进任何人名下；每个结论都标明依据是逐行证据、明确声明，还是推断。推断不冒充证据，`strict` 口径下完全不做推断。
- **开发者是作者，也是编辑。** 你决定口径（strict / standard / loose）、决定公开哪些内容（项目名、PR 标题、文件名、人名），决定用哪种样子拿出去。creditia 只负责把事实整理好、做得好看。

归因的完整规则、各数据源的边界、多路径项目和各种边缘情况，见 [ATTRIBUTION.md](ATTRIBUTION.md)。

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
creditia init                       # 写 .creditia/config.json（识别你的提交身份、主分支、路径）
creditia analyze --github           # 分析，写 .creditia/credits.json
creditia doc                        # 写 CREDITS.md（只改标记区间）
creditia export --piece tier --style ink --format png --out tier.png
creditia export --piece roll --style cinema --variant scroll --format html --out credits.html
creditia board --style glass --out credits.html   # 全部作品放进一个可交互的页面
creditia hook install               # 之后每次提交自动盖章，不再依赖日志
```

一个项目在好几个地方（worktree、临时克隆、换过电脑、搬过家）：

```bash
creditia paths                      # 看本项目登记过的全部路径
creditia link /old/machine/path     # 关联一个路径（目录不在了也行）
```

让 agent 代你操作时，所有命令都支持 `--json`、非交互；agent 可以先 `creditia whoami` 认出自己，再 `creditia stamp` 给自己的提交盖章。见 [skills/creditia/SKILL.md](skills/creditia/SKILL.md)。

## App

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
