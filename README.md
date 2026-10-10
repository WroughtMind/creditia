# creditia

> 为你和 AI 一起做出的作品署名。
> Credits for work made with agents.

**状态：MVP 开发中。** 方案见 [`docs/plans/2026-10-08-creditia-plan.md`](docs/plans/2026-10-08-creditia-plan.md)，阶段 0 验证见 [`docs/validation/`](docs/validation/)。

## 做什么

读 git 历史和本机 agent 日志（Codex、Claude Code、Cursor、Grok、Kimi），逐行归因：哪段代码是哪个工具、哪个模型在哪次会话里写的。产出一份档案（`credits.json` + `CREDITS.md`），再从同一份档案画出多种展示：tier 榜单、片尾字幕、精力小票、徽章、统计图。

同一形式在十四个版式家族下有不同结构（极简 / 电影 / 终端 / 玻璃 / 杂志 / 等宽 / 黄金时代片尾 / 剧院节目单 / 颁奖礼 / 老报纸 / CRT 磷光 / 街机高分榜 / 霓虹招牌 / 海报）：片尾字幕十一种版式（卡司表、双栏滚屏、卡串、终卡、billing block、节目单……）、精力小票七种版式（电影票、账簿、长卷纸……），十五套主题跟着家族换肤。人不和 agent 排同一个百分比，而是单独署名。

- **本地优先**：扫描和计算都在本机；只存行哈希和元数据，不存代码和 prompt。
- **诚实**：证据不够就归入「未确认」；每段数字标出证据覆盖情况；每条归因都能回查到 commit。
- **人单独署名**：人不和 agent 排代码行百分比，而是作为主导者单独署名（可关闭）。

## 仓库结构

```
crates/
  core/       归因引擎、行哈希、时间、配置
  adapters/   git、Codex、Claude Code、Cursor、Grok、Kimi 的日志读取
  store/      作品库：项目、证据库、快照（SQLite）
  cli/        二进制 creditia-core
packages/
  schema/     credits.json v0 的 JSON Schema 与类型
  render/     展示渲染器：credits.json → SVG / PNG（Satori + resvg）
  cli/        npm 包 creditia：档案、导出、终端名片
  embed/      <creditia-card> Web Component
apps/desktop/ Tauri 2 App（预览与导出中心）
skills/       给 Claude Code / Codex / Cursor 的 agent skill
spike/        阶段 0 验证工程（保留作记录）
```

## 开发

```bash
# Rust 核心
cargo build --release         # 产出 target/release/creditia-core
cargo test                    # 单元 + 端到端（用临时 HOME 的假日志）

# 渲染器
cd packages/render && npm i
node src/gallery.ts ../../spike/out gallery    # 全部形式 × 全部风格

# CLI（Node 侧）
cd packages/cli && node bin/creditia.mjs --help

# App（需要 Rust + tauri 依赖；图标与预览字体子集：npm run icon / npm run fonts）
cd apps/desktop && npm i && npm run tauri dev
```

## 快速上手

```bash
cd 你的仓库
creditia init                  # 写 .creditia/config.json
creditia analyze --github      # 分析，写 .creditia/credits.json
creditia doc                   # 写 CREDITS.md（只改标记区间）
creditia export --form tier --theme minimal-light --format png --out tier.png
creditia hook install          # 之后每次提交自动盖章，不再依赖日志
```

## 协议

MIT
