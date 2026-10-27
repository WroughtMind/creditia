# @creditia/render

`credits.json` → 展示模型 → **SVG 场景**（风格 × 作品）→ PNG / SVG / PDF / GIF / HTML。纯本地，无网络请求。CLI、App、网页组件、样张都走这里。

## 用法

```bash
cd packages/render && npm install
npm test                         # 全部作品 × 全部版式 × 全部风格
npm run gallery                  # 样张 → gallery/index.html（fixtures/catnap.json 是虚构的样例档案）
node src/gallery.ts my/credits.json out ink,cinema   # 指定档案、输出目录、风格
```

需要 Node ≥ 22.18（直接运行 .ts）。

```ts
import { buildModel, styleById, art, frames, exporter } from '@creditia/render/src/index.ts';
const m = buildModel(credits, { lang: 'zh' });   // named: false 不写其他贡献者的名字
const a = art('roll', m, styleById('ukiyoe')!, { variant: 'vertical' });   // { w, h, svg, hits, bg, title }
writeFileSync('roll.png', exporter.png(a, 2));
writeFileSync('roll.svg', exporter.svg(a));                              // 文字已转成路径
writeFileSync('tier.gif', exporter.gif(frames('tier', m, styleById('cyber')!)));
writeFileSync('roll.html', exporter.html([a], { title: a.title }));      // 单文件，悬停看细节
```

## 结构

| 文件 | 作用 |
|---|---|
| `src/data.ts` | `credits.json` → 展示模型：agent 按工具归组（含模型、渠道、留存、逐行证据比例）；人的部分（指令、审阅、亲手写的、取舍）；不再有「主导者」 |
| `src/svg.ts` | SVG 场景：文字度量（`metrics.json`）、竖排、交互区域（hits）、基础图形 |
| `src/style.ts` | 风格接口：底、卡片面、标题、条形、标记、分隔、斜纹、配色；`reserve` 声明底图占用的边距 |
| `src/styles/*.ts` | 16 个风格：极简、暗室、液态玻璃、水墨、浮世绘、古籍、黄金时代、巴洛克、魔典、手稿、对称、剪纸、夏日、漫画、赛博、终端 |
| `src/pieces/*.ts` | 作品：署名榜（6 种版式）、片尾字幕（7 种）、分享图、人的部分、小票（2 种）、徽章（3 种）、署名行、9 种统计图 |
| `src/registry.ts` | 作品清单、版式名、`art()` / `frames()` |
| `src/export.ts` | 导出：PNG、扁平 SVG、PDF（无依赖的写入器）、GIF（gifenc）、HTML 单文件、常用画幅 |
| `src/interact.ts` | 悬停交互（App 与导出的 HTML 共用一份代码） |
| `src/text.ts` | CREDITS.md、版本说明、PR 评论、嵌入代码、终端名片 |
| `src/fonts.ts` | 字体清单；Node 端按目录查找 .ttf（不依赖文件名大小写） |
| `tools/` | 生成度量表、logo 模块、样例档案 |

## 加一个风格

在 `src/styles/` 里 `defineStyle({...})`：给出配色、字体、`dot()`（App 左下角的小圆点）和需要的钩子（`ground`、`plate`、`title`、`bar`、`mark`、`rule`……），再加进 `styles/index.ts`。字体加进 `src/fonts.ts` 后运行 `npm run metrics`。`npm test` 会把它和每一种作品、每一种版式都画一遍。
