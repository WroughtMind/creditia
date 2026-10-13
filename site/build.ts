// 官网构建：node site/build.ts [输出目录]
// 页面上的作品都是渲染器现画的（原版 satori 渲染器和新风格两套），演示用虚构的样例项目；
// creditia 自己的署名用仓库里的 credits.json。大图是文字已转路径的 SVG，缩略图是 PNG。
import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync, cpSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { buildModel, STYLES, styleById, art, exporter, raster, frames } from '../packages/render/src/index.ts';
import { PIECES } from '../packages/render/src/registry.ts';
import { makeExporter } from '../packages/render/src/export.ts';
import { creditsBlock, fetchSegs } from '../packages/render/src/text.ts';
import { deflateSync } from 'node:zlib';
import type { Art } from '../packages/render/src/export.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF } from '../packages/render/src/style.ts';
import { TOOL_NAMES } from '../packages/render/src/data.ts';
import * as C from '../packages/render/src/classic/index.ts';
import { ICONS } from './icons.ts';
import { ORDERED_THEMES, themeName, themeNote, themeDot, pieceName, variantName } from '../packages/render/src/classic/catalog.ts';

const ROOT = path.resolve(import.meta.dirname, '..');
const OUT = path.resolve(process.argv[2] ?? path.join(ROOT, '_site'));
rmSync(OUT, { recursive: true, force: true });
mkdirSync(path.join(OUT, 'assets'), { recursive: true });

const sample = JSON.parse(readFileSync(path.join(ROOT, 'packages/render/fixtures/lumen.json'), 'utf8'));
const own = existsSync(path.join(ROOT, 'credits.json')) ? JSON.parse(readFileSync(path.join(ROOT, 'credits.json'), 'utf8')) : null;
const LANGS = ['zh', 'en'] as const;
type Lang = (typeof LANGS)[number];

// creditia 自己的完整署名页：交给命令行生成。放在最前面跑，这时本进程还没载入字体，两个进程加起来不会太占内存
// 本地预览内存紧时可以设 SKIP_SELF_PAGE=1 跳过这一步
if (own && !process.env.SKIP_SELF_PAGE) {
  for (const lang of LANGS) {
    const r = spawnSync(process.execPath, [path.join(ROOT, 'packages/cli/bin/creditia.mjs'), 'site', '--credits', path.join(ROOT, 'credits.json'), '--style', 'minimal', '--lang', lang, '--out', path.join(OUT, lang === 'zh' ? 'self' : 'self/en')], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error('creditia site 失败');
  }
  console.log('✓ self page');
}

// 官网上的真 App：apps/desktop 的界面按网页打包，放进「桌面」演示里，用样例项目的档案。
// 需要 apps/desktop 装好依赖并生成过字体（npm ci && npm run fonts）；本地不想打包可以设 SKIP_APP=1
const DESK = path.join(ROOT, 'apps/desktop');
const hasApp = !process.env.SKIP_APP && existsSync(path.join(DESK, 'node_modules/vite')) && existsSync(path.join(DESK, 'src/fonts/manifest.json'));
if (hasApp) {
  const r = spawnSync(process.execPath, [path.join(DESK, 'node_modules/vite/bin/vite.js'), 'build', '--base', './', '--outDir', path.join(OUT, 'app'), '--emptyOutDir', '--logLevel', 'warn'], { cwd: DESK, stdio: 'inherit' });
  if (r.status !== 0) throw new Error('App 网页版打包失败');
  const demo = {
    projects: [{ id: 'lumen', name: 'lumen', path: '~/code/lumen', paths: ['~/code/lumen'], lastAnalyzed: Math.floor(Date.now() / 1000), evidence: 18234, exists: true, hasCredits: true }],
    credits: { lumen: sample },
    doctor: [{ id: 'claude-code', name: 'Claude Code', ok: true, detail: '' }, { id: 'codex', name: 'Codex', ok: true, detail: '' }, { id: 'cursor', name: 'Cursor', ok: true, detail: '' }],
  };
  const f = path.join(OUT, 'app/index.html');
  writeFileSync(f, readFileSync(f, 'utf8').replace('<head>', `<head>\n<script>window.__CREDITIA_DEMO__ = ${JSON.stringify(demo)};</script>`));
  cpSync(path.join(DESK, 'src-tauri/icons/128x128@2x.png'), path.join(OUT, 'assets/app-icon.png'));
  console.log('✓ app');
}

/** 和原版重名、或原版已有对应的新风格不重复 */
const V2_SKIP = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const V2_STYLES = STYLES.filter((s) => !V2_SKIP.has(s.id));
const isClassic = (id: string) => ORDERED_THEMES.some((t) => t.id === id);

const models = new Map<string, any>();
const modelOf = (raw: any, lang: Lang, kind: 'c' | 'v') => {
  const k = `${raw === sample ? 's' : 'o'}${lang}${kind}`;
  if (!models.has(k)) models.set(k, kind === 'c' ? C.buildModel(raw, { director: false, lang }) : buildModel(raw, { lang }));
  return models.get(k);
};
const write = (rel: string, data: string | Uint8Array) => { const p = path.join(OUT, 'assets', rel); mkdirSync(path.dirname(p), { recursive: true }); writeFileSync(p, data); };

/** 画一张作品，写 SVG 大图；thumb 给宽度时另写一张 PNG 缩略图。style 是原版主题 id 或新风格 id */
async function draw(rel: string, piece: string, style: string, lang: Lang, o: { variant?: string; raw?: any; thumb?: number; thumbH?: number; noSvg?: boolean } = {}) {
  const raw = o.raw ?? sample;
  if (isClassic(style)) {
    const a = (await C.art(piece, modelOf(raw, lang, 'c'), C.themeById(style)!, { lang, variant: o.variant }))!;
    if (!o.noSvg) write(`${rel}.svg`, a.svg);
    const tw = o.thumb ?? (o.thumbH ? Math.round((o.thumbH * a.w) / a.h) : 0);
    if (tw) write(`${rel}.png`, C.raster.png(a.svg, tw));
    return { w: a.w, h: a.h, bg: a.bg };
  }
  const a: Art = art(piece, modelOf(raw, lang, 'v'), styleById(style)!, { variant: o.variant })!;
  if (!o.noSvg) write(`${rel}.svg`, exporter.svg(a));
  const tw = o.thumb ?? (o.thumbH ? Math.round((o.thumbH * a.w) / a.h) : 0);
  if (tw) write(`${rel}.png`, exporter.png(a, tw / a.w));
  return { w: a.w, h: a.h, bg: a.bg };
}

const meta: any = { wall: {}, usage: {}, selfBadge: null };

// 首屏和命令行演示用的数字：样例项目 creditia fetch 的输出（各工具占比、模型）
meta.fetch = {}; meta.share = {};
for (const lang of LANGS) {
  const segs = fetchSegs(modelOf(sample, lang, 'v'));
  meta.fetch[lang] = segs;
  meta.share[lang] = segs.filter((l) => l.length >= 4 && l[0].color).map((l) => ({ key: l[0].color, name: l[0].text.trim(), pct: parseFloat(l[3].text), model: (l[4]?.text ?? '').trim() }));
}

// 风格墙：三条带子，图块大小不一；一列可以是一张，也可以上下叠两三张。
// 有的图块会动：live 是一整卷字幕在小窗里滚，morph 是同一张分享图在几种风格之间切换
const styles = [
  ...ORDERED_THEMES.map((t) => ({ id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'), note: themeNote(t, 'zh'), noteEn: themeNote(t, 'en'), dot: themeDot(t), bg: t.bg })),
  ...V2_STYLES.map((s) => ({ id: s.id, name: s.name, en: s.en, note: s.note, noteEn: '', dot: s.dot(), bg: s.c.bg })),
];
type Item = { style: string; piece: string; variant?: string; live?: boolean; morph?: string[] };
const it = (style: string, piece: string, variant?: string): Item => ({ style, piece, variant });
const live = (style: string): Item => ({ style, piece: 'roll', variant: 'scroll', live: true });
const morph = (piece: string, list: string[]): Item => ({ style: list[0], piece, morph: list });
const WALL: { h: number; cols: Item[][] }[] = [
  { h: 300, cols: [
    [it('cinema', 'roll', 'cast')], [it('ukiyoe', 'share'), it('neon', 'lanes')], [live('grimoire')], [it('woodblock', 'tier', 'vertical')],
    [it('film', 'fetch'), it('poster', 'ribbon')], [it('manga', 'tier', 'stats')], [morph('share', ['broadsheet', 'gala', 'arcade', 'poster', 'crt', 'film'])],
    [it('baroque', 'tier', 'podium')], [it('editorial', 'mosaic'), it('minimal-dark', 'clock')], [it('playbill', 'roll', 'production')],
    [it('pastel', 'receipt', 'slip')], [it('cyber', 'tier', 'hud')],
  ] },
  { h: 200, cols: [
    [it('meadow', 'roll', 'ed')], [it('terminal', 'fetch')], [it('glass', 'app')], [it('ink', 'calendar')], [it('gala', 'roll', 'crew')],
    [it('broadsheet', 'versions')], [it('cutout', 'roll', 'billing')], [it('crt', 'area')], [it('arcade', 'waffle')], [it('ukiyoe', 'roll', 'vertical')],
    [it('minimal-light', 'roll', 'slate')], [it('neon', 'donut')],
  ] },
  { h: 300, cols: [
    [live('manuscript')], [it('editorial', 'roll', 'opening')], [it('woodblock', 'stream'), it('baroque', 'survival')], [it('poster', 'tier')],
    [it('grimoire', 'receipt', 'ticket')], [it('mono', 'waffle'), it('film', 'settle')], [it('manga', 'roll', 'card')], [it('pastel', 'versions')],
    [live('meadow')], [it('cutout', 'tier', 'center')], [it('playbill', 'donut'), it('glass', 'lanes')], [it('minimal-dark', 'roll', 'end')], [it('cyber', 'clock')],
  ] },
];
const GAP = 12;          // 叠放时上下的间距
const LIVE_AR = 0.95;    // 滚动小窗的宽高比：比整卷宽，字幕才有得滚
const SCALE = 1.75;      // 缩略图按显示尺寸的倍数出图
const nameOf = (x: { style: string; piece: string; variant?: string }) => `${x.style}-${x.piece}${x.variant ? '-' + x.variant : ''}`;
const pieceNames = (style: string, piece: string): [string, string] => (isClassic(style) ? [pieceName(piece, 'zh'), pieceName(piece, 'en')] : [PIECES[piece].name, PIECES[piece].en]);

/** 画一张（同一张只画一次）：SVG 大图、尺寸、按宽度出 PNG */
const drawn = new Map<string, { svg: string; w: number; h: number; png: (tw: number) => Uint8Array }>();
async function render(lang: Lang, x: { style: string; piece: string; variant?: string }) {
  const k = `${lang}/${nameOf(x)}`;
  if (!drawn.has(k)) {
    if (isClassic(x.style)) {
      const a = (await C.art(x.piece, modelOf(sample, lang, 'c'), C.themeById(x.style)!, { lang, variant: x.variant }))!;
      drawn.set(k, { svg: a.svg, w: a.w, h: a.h, png: (tw) => C.raster.png(a.svg, tw) });
    } else {
      const a: Art = art(x.piece, modelOf(sample, lang, 'v'), styleById(x.style)!, { variant: x.variant })!;
      drawn.set(k, { svg: exporter.svg(a), w: a.w, h: a.h, png: (tw) => exporter.png(a, tw / a.w) });
    }
  }
  return drawn.get(k)!;
}

for (const lang of LANGS) {
  const rows = [];
  for (const row of WALL) {
    const cols = [];
    for (const col of row.cols) {
      // 先量尺寸，算出这一列的宽：叠放的几张同宽，高度加起来正好是带子高
      const dims: { w: number; h: number }[] = [];
      for (const x of col) {
        if (x.live) dims.push({ w: LIVE_AR, h: 1 });
        else if (x.morph) dims.push({ w: 1200, h: 630 });
        else dims.push(await render(lang, x));
      }
      const W = Math.round((row.h - GAP * (col.length - 1)) / dims.reduce((s, d) => s + d.h / d.w, 0));
      const items = [];
      for (const [i, x] of col.entries()) {
        const dh = Math.round((W * dims[i].h) / dims[i].w);
        const one = async (style: string) => {
          const y = { ...x, style };
          const d = await render(lang, y);
          write(`wall/${lang}/${nameOf(y)}.svg`, d.svg);
          write(`wall/${lang}/${nameOf(y)}.png`, d.png(Math.round(W * SCALE)));
          const [pz, pe] = pieceNames(style, x.piece);
          return { style, piece: x.piece, name: nameOf(y), w: d.w, h: d.h, pz, pe };
        };
        if (x.morph) { const list = []; for (const st of x.morph) list.push(await one(st)); items.push({ kind: 'morph', dh, list }); }
        else items.push({ ...(await one(x.style)), kind: x.live ? 'live' : 'still', dh });
      }
      cols.push({ w: W, items });
    }
    rows.push({ h: row.h, cols });
  }
  meta.wall[lang] = rows; // 英文版有的作品高度不同，布局按语言各算一份
  console.log('✓ wall', lang);
  drawn.clear();
}

// 用法 · 命令行：真的跑一遍 export，各种格式都是同一张署名榜（noir）；终端里的 → 文件名点了就是这些文件
const exp = makeExporter(raster, (d) => new Uint8Array(deflateSync(d)));
const kb = (n: number) => (n > 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);
meta.formats = {};
for (const lang of LANGS) {
  const vm = modelOf(sample, lang, 'v'), st = styleById('noir')!, f: Record<string, string> = {};
  const tierA = art('tier', vm, st)!;
  const out = (ext: string, d: string | Uint8Array) => { write(`out/${lang}/credits.${ext}`, d); f[ext] = kb(typeof d === 'string' ? Buffer.byteLength(d) : d.length); };
  out('png', exp.png(tierA, 1));
  out('svg', exp.svg(tierA));
  out('pdf', await exp.pdf(tierA, 1));
  out('gif', exp.gif(frames('tier', vm, st, {}, 20), { width: 400 }));
  out('html', exp.html(['tier', 'share', 'donut'].map((id) => art(id, vm, st)!).filter(Boolean), { title: 'lumen · credits', lang, bg: st.c.bg, note: 'lumen' }));
  out('md', creditsBlock(vm, 4));
  write(`out/${lang}/peek.png`, exporter.png(tierA, 520 / tierA.w)); // 悬停时的小预览
  meta.formats[lang] = { sizes: f, w: tierA.w, h: tierA.h };
}
console.log('✓ formats');

// 用法 · agent：对话里导出的那张分享图
// 用法 · GitHub：README 里的徽章（带风格的，不是空白条）和 Credits 段落里的署名条
const HERO_STYLES = ['cinema', 'woodblock', 'neon', 'pastel', 'editorial', 'cyber', 'manga', 'grimoire', 'broadsheet', 'baroque']; // 都是 1200×630，切换时画框不变形
meta.heroStyles = HERO_STYLES;
for (const lang of LANGS) {
  const vm = modelOf(sample, lang, 'v');
  const sh = art('share', vm, styleById('ukiyoe')!)!; write(`usage/${lang}-share.png`, exporter.png(sh, 640 / sh.w));
  // GitHub Action 那一栏：README 例子里用的是 ink 风格
  { const b = art('badge', vm, styleById('ink')!, { variant: 'flat' })!; write(`usage/${lang}-ink-badge.svg`, exporter.svg(b)); meta.usage.ink = { w: b.w, h: b.h }; }
  { const a = art('share', vm, styleById('ink')!)!; write(`usage/${lang}-ink-share.png`, exporter.png(a, 560 / a.w)); }
  for (const v of ['flat', 'stack']) { const a = art('badge', vm, styleById('ukiyoe')!, { variant: v })!; write(`usage/${lang}-badge-${v}.svg`, exporter.svg(a)); meta.usage[v] = { w: a.w, h: a.h }; }
  // 首屏：同一张分享图，在几种风格之间切换（SVG，文字已转路径，大屏也清楚）
  for (const id of HERO_STYLES) await draw(`hero/${lang}/${id}`, 'share', id, lang);
  if (own) meta.selfBadge = await draw(`usage/${lang}-self-badge`, 'badge', 'minimal-light', lang, { raw: own });
}
console.log('✓ usage');

// 社交预览图
for (const lang of LANGS) writeFileSync(path.join(OUT, `assets/og-${lang}.png`), exporter.png(art('share', buildModel(sample, { lang }), styleById('cinema')!)!, 1));

const tools = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'copilot', 'kimi', 'grok', 'devin', 'jules', 'aider', 'amp'].map((k) => {
  const lg = LOGOS[LOGO_OF[k] ?? ''];
  return { name: TOOL_NAMES[k] ?? k, svg: lg ? `<svg viewBox="${lg.vb}" aria-hidden="true"><path d="${lg.d}"${lg.rule ? ` fill-rule="${lg.rule}"` : ''}/></svg>` : '' };
});
const logo = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8').replace(/#0E0E0E/g, 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');

let html = readFileSync(path.join(import.meta.dirname, 'index.html'), 'utf8');
html = html
  .replace('/*__DATA__*/', `const DATA = ${JSON.stringify({ ...meta, styles, tools, hasSelf: !!own, hasApp, icons: ICONS, version: JSON.parse(readFileSync(path.join(ROOT, 'packages/cli/package.json'), 'utf8')).version })};`)
  .replaceAll('<!--__LOGO__-->', logo);
writeFileSync(path.join(OUT, 'index.html'), html);
writeFileSync(path.join(OUT, 'assets/favicon.svg'), logo.replace(/currentColor/g, '#111'));
cpSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), path.join(OUT, 'assets/logo.svg'));
writeFileSync(path.join(OUT, '.nojekyll'), '');
console.log(`→ ${OUT}/index.html`);
