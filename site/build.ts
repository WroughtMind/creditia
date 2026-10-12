// 官网构建：node site/build.ts [输出目录]
// 页面上的每一张作品都是渲染器现画的（原版 satori 渲染器和新风格两套），演示用虚构的样例项目；
// creditia 自己那一段用仓库里的 credits.json。作品都写成文字已转路径的 SVG，页面里用 <img> 引用。
import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync, cpSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { buildModel, STYLES, styleById, art, exporter } from '../packages/render/src/index.ts';
import type { Art } from '../packages/render/src/export.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF } from '../packages/render/src/style.ts';
import { TOOL_NAMES } from '../packages/render/src/data.ts';
import * as C from '../packages/render/src/classic/index.ts';
import { ORDERED_THEMES, themeDot, themeName, themeNote } from '../packages/render/src/classic/catalog.ts';

const ROOT = path.resolve(import.meta.dirname, '..');
const OUT = path.resolve(process.argv[2] ?? path.join(ROOT, '_site'));
rmSync(OUT, { recursive: true, force: true });
mkdirSync(path.join(OUT, 'assets'), { recursive: true });

const sample = JSON.parse(readFileSync(path.join(ROOT, 'packages/render/fixtures/lumen.json'), 'utf8'));
const own = existsSync(path.join(ROOT, 'credits.json')) ? JSON.parse(readFileSync(path.join(ROOT, 'credits.json'), 'utf8')) : null;
const LANGS = ['zh', 'en'] as const;
type Lang = (typeof LANGS)[number];

// creditia 自己的完整署名页：交给命令行生成。放在最前面跑，这时本进程还没载入字体，两个进程加起来不会太占内存
if (own) {
  for (const lang of LANGS) {
    const r = spawnSync(process.execPath, [path.join(ROOT, 'packages/cli/bin/creditia.mjs'), 'site', '--credits', path.join(ROOT, 'credits.json'), '--style', 'minimal', '--lang', lang, '--out', path.join(OUT, lang === 'zh' ? 'self' : 'self/en')], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error('creditia site 失败');
  }
  console.log('✓ self page');
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

/** 画一张作品：style 是原版主题 id 或新风格 id。写到 assets/<dir>/<name>.svg，返回尺寸和底色 */
async function draw(dir: string, name: string, piece: string, style: string, lang: Lang, variant?: string, raw = sample) {
  let svg: string, w: number, h: number, bg: string;
  if (isClassic(style)) {
    const a = (await C.art(piece, modelOf(raw, lang, 'c'), C.themeById(style)!, { lang, variant }))!;
    ({ svg, w, h, bg } = a);
  } else {
    const a: Art = art(piece, modelOf(raw, lang, 'v'), styleById(style)!, { variant })!;
    svg = exporter.svg(a); ({ w, h, bg } = a);
  }
  mkdirSync(path.join(OUT, 'assets', dir), { recursive: true });
  writeFileSync(path.join(OUT, 'assets', dir, `${name}.svg`), svg);
  return { w, h, bg };
}

const meta: any = { hero: null, wall: [], styleArt: {}, self: null, selfStats: null };

// 首屏：一组成品叠在一起——片尾字幕、分享图、README 徽章
for (const lang of LANGS) {
  meta.hero = {
    roll: await draw(`hero/${lang}`, 'roll', 'roll', 'cinema', lang, 'classic'),
    share: await draw(`hero/${lang}`, 'share', 'share', 'ukiyoe', lang),
    badge: await draw(`hero/${lang}`, 'badge', 'badge', 'minimal-light', lang),
  };
}
console.log('✓ hero');

// 作品墙：形式、风格都尽量不一样
const WALL: [string, string, string?][] = [
  ['roll', 'cinema', 'cast'], ['tier', 'ukiyoe'], ['receipt', 'film', 'stub'], ['tier', 'cyber', 'hud'],
  ['roll', 'woodblock', 'vertical'], ['donut', 'gala'], ['tier', 'glass'], ['tier', 'manga', 'stats'],
  ['roll', 'playbill', 'program'], ['versions', 'editorial'], ['receipt', 'manuscript'], ['fetch', 'terminal'],
  ['roll', 'meadow', 'ed'], ['share', 'broadsheet'], ['roll', 'minimal-light', 'slate'], ['roll', 'gala', 'crew'],
];
for (const lang of LANGS) {
  for (const [piece, style, v] of WALL) {
    const name = `${piece}-${style}${v ? '-' + v : ''}`;
    const d = await draw(`wall/${lang}`, name, piece, style, lang, v);
    if (lang === 'zh') meta.wall.push({ name, piece, style, variant: v ?? '', ...d });
  }
  console.log('✓ wall', lang);
}

// 风格：每种一张署名榜、一张片尾字幕
const styles = [
  ...ORDERED_THEMES.map((t) => ({ id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'), note: themeNote(t, 'zh'), noteEn: themeNote(t, 'en'), dot: themeDot(t) })),
  ...V2_STYLES.map((s) => ({ id: s.id, name: s.name, en: s.en, note: s.note, noteEn: '', dot: s.dot() })),
];
for (const lang of LANGS) {
  for (const s of styles) {
    const tier = await draw(`styles/${lang}`, `${s.id}-tier`, 'tier', s.id, lang);
    const roll = await draw(`styles/${lang}`, `${s.id}-roll`, 'roll', s.id, lang);
    meta.styleArt[s.id] = { tier, roll };
  }
  console.log('✓ styles', lang);
}

// creditia 自己：署名榜 + 徽章，用真实数据
if (own) {
  for (const lang of LANGS) {
    meta.self = {
      tier: await draw(`self/${lang}`, 'tier', 'tier', 'minimal-light', lang, undefined, own),
      badge: await draw(`self/${lang}`, 'badge', 'badge', 'minimal-light', lang, undefined, own),
    };
  }
  const m = modelOf(own, 'zh', 'c');
  meta.selfStats = { commits: m.commits, lines: m.linesAdded };
  console.log('✓ self');
}

// 社交预览图
for (const lang of LANGS) writeFileSync(path.join(OUT, `assets/og-${lang}.png`), exporter.png(art('share', buildModel(sample, { lang }), styleById('cinema')!)!, 1));

const tools = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'copilot', 'kimi', 'grok', 'devin', 'jules', 'aider', 'amp'].map((k) => {
  const lg = LOGOS[LOGO_OF[k] ?? ''];
  const name = TOOL_NAMES[k] ?? k;
  return { name, svg: lg ? `<svg viewBox="${lg.vb}" aria-hidden="true"><path d="${lg.d}"/></svg>` : `<b>${name.split(/\s+/).map((w) => w[0]).join('').slice(0, 2)}</b>` };
});
const logo = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8').replace(/#0E0E0E/g, 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');

let html = readFileSync(path.join(import.meta.dirname, 'index.html'), 'utf8');
html = html
  .replace('/*__DATA__*/', `const DATA = ${JSON.stringify({ ...meta, styles, tools, hasSelf: !!own })};`)
  .replaceAll('<!--__LOGO__-->', logo);
writeFileSync(path.join(OUT, 'index.html'), html);
writeFileSync(path.join(OUT, 'assets/favicon.svg'), logo.replace(/currentColor/g, '#111'));
cpSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), path.join(OUT, 'assets/logo.svg'));
// App 截图（展墙，样例数据），随仓库提交
for (const f of ['app-light.webp', 'app-dark.webp']) if (existsSync(path.join(import.meta.dirname, f))) cpSync(path.join(import.meta.dirname, f), path.join(OUT, 'assets', f));
writeFileSync(path.join(OUT, '.nojekyll'), '');
console.log(`→ ${OUT}/index.html`);
