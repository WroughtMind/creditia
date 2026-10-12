// 官网构建：node site/build.ts [输出目录]
// 页面上的作品都是渲染器现画的（原版 satori 渲染器和新风格两套），演示用虚构的样例项目；
// creditia 自己的署名用仓库里的 credits.json。作品都写成文字已转路径的 SVG，页面里用 <img> 引用。
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
// 本地预览内存紧时可以设 SKIP_SELF_PAGE=1 跳过这一步
if (own && !process.env.SKIP_SELF_PAGE) {
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

const meta: any = { reel: null, styleArt: {}, badge: null, selfBadge: null };

// 首屏：一整卷片尾字幕，在银幕里往上滚（新风格里黄金时代的长卷；原版的 cinema 主题同名，这里直接用新渲染器）
mkdirSync(path.join(OUT, 'assets/reel'), { recursive: true });
for (const lang of LANGS) {
  const a = art('roll', modelOf(sample, lang, 'v'), styleById('cinema')!, { variant: 'scroll' })!;
  writeFileSync(path.join(OUT, 'assets/reel', `${lang}.svg`), exporter.svg(a));
  meta.reel = { w: a.w, h: a.h, bg: a.bg };
}
console.log('✓ reel');

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

// 示例徽章（放在「开始用」里）和 creditia 自己的徽章（真实数据，页脚）
for (const lang of LANGS) {
  meta.badge = await draw('badge', lang, 'badge', 'minimal-light', lang);
  if (own) meta.selfBadge = await draw('badge', `self-${lang}`, 'badge', 'minimal-light', lang, undefined, own);
}
console.log('✓ badges');

// 社交预览图
for (const lang of LANGS) writeFileSync(path.join(OUT, `assets/og-${lang}.png`), exporter.png(art('share', buildModel(sample, { lang }), styleById('cinema')!)!, 1));

const tools = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'copilot', 'kimi', 'grok', 'devin', 'jules', 'aider', 'amp'].map((k) => {
  const lg = LOGOS[LOGO_OF[k] ?? ''];
  return { name: TOOL_NAMES[k] ?? k, svg: lg ? `<svg viewBox="${lg.vb}" aria-hidden="true"><path d="${lg.d}"${lg.rule ? ` fill-rule="${lg.rule}"` : ''}/></svg>` : '' };
});
const logo = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8').replace(/#0E0E0E/g, 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');

let html = readFileSync(path.join(import.meta.dirname, 'index.html'), 'utf8');
html = html
  .replace('/*__DATA__*/', `const DATA = ${JSON.stringify({ ...meta, styles, tools, hasSelf: !!own })};`)
  .replaceAll('<!--__LOGO__-->', logo);
writeFileSync(path.join(OUT, 'index.html'), html);
writeFileSync(path.join(OUT, 'assets/favicon.svg'), logo.replace(/currentColor/g, '#111'));
cpSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), path.join(OUT, 'assets/logo.svg'));
writeFileSync(path.join(OUT, '.nojekyll'), '');
console.log(`→ ${OUT}/index.html`);
