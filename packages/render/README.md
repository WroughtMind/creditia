# @creditia/render

`credits.json` → 对象树 → Satori → SVG → @resvg/resvg-js → PNG。纯本地，无网络请求。CLI 导出、App 预览、批量样张都走这里。

## 用法

```bash
cd packages/render && npm install
node src/gallery.ts ../../spike/out gallery     # 全部形式 × 全部风格 → gallery/index.html
FORMS=tier,roll THEMES=minimal-light,film node src/gallery.ts ../../spike/out gallery
PEOPLE=1 node src/gallery.ts ...              # 显示用户确认的人员署名（默认不显示）
```

需要 Node ≥ 22.18（直接运行 .ts）。`gallery/` 已被忽略。

程序里用：

```ts
import { buildModel, renderSvg, svgToPng, themeById, FORMS } from '@creditia/render/src/index.ts';
const m = buildModel(credits, { people: false, lang: 'zh' });
const svg = await renderSvg('tier', m, themeById('minimal-light')!, { lang: 'zh' });
```

## 结构

| 文件 | 作用 |
|---|---|
| `src/index.ts` | 入口：`FORMS` 清单、`renderSvg` / `renderSized` / `svgToPng`、字体加载 |
| `src/data.ts` | `credits.json` → 展示模型：按工具聚合、保留各贡献方和未确认项、主要模型按新增行排序 |
| `src/themes.ts` | 原有 15 套主题 |
| `src/brand.ts` | 各家 logo（`assets/logos/`）与熟悉的品牌色 |
| `src/ui.ts` | Satori 对象树的最小工具：标签、条、logo、背景、面板 |
| `src/forms.ts` | 18 种形式的版式 |
| `src/text.ts` | 文本形式：`CREDITS.md`（只改标记区间）、Release notes、PR 评论、终端名片、逐行溯源 |
| `src/gallery.ts` | 样张总览页 |
| `src/export.ts` | 共享 PNG、SVG、PDF、GIF 和自包含 HTML 编码 |

## 形式

一眼看懂：tier、share（分享图）、byline（署名行）、badge（徽章，原生 SVG）
有情绪：roll（片尾字幕）、receipt（精力小票）、ribbon（时间织带）、mosaic（代码马赛克）、profile（个人履历卡）、fetch（终端名片）
融进工作流：CREDITS.md、PR 评论、Release notes、blame（逐行溯源）、app（App 内致谢页）
统计图：waffle、donut、area、lanes、clock、versions、settle

## 收敛规则

- 一张图里，每条信息只出现一次。
- 不写凑数的话；不放占位（字母 logo、"暂无数据"、水印都不出现）。
- 每种形式只回答它那一个问题：占比只在 tier、华夫饼、环形图里出现；小票只列投入；片尾字幕只写角色和名字。
- 数据不可信就不画。
- 人员署名默认不显示（`people: true` 才出现）。

## 字体（均为 npm 包内 .ttf，Satori 可用，SIL OFL 1.1）

| 用途 | 字体 | 包 |
|---|---|---|
| 正文、标签 | Inter | `@expo-google-fonts/inter` |
| 中文 | Noto Sans SC / Noto Serif SC | `@expo-google-fonts/noto-sans-sc`、`noto-serif-sc` |
| 数字、等宽 | JetBrains Mono | `@expo-google-fonts/jetbrains-mono` |
| Film Credits 大字 | Instrument Serif | `@expo-google-fonts/instrument-serif` |
| Editorial 大字 | Newsreader | `@expo-google-fonts/newsreader` |

Noto SC 每个字重 10 MB 以上，正式发布前要子集化。

## Logo

单色 SVG 放在 `assets/logos/`，来源见 `assets/logos/SOURCES.md`。只用于识别对应工具，各家使用规范发布前逐家核对。暗色底下 Grok（X）、Kimi 用浅色。
