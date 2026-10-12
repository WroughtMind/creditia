// 官网构建：node site/build.ts [输出目录]
// 页面上的每一张图都是渲染器现画的：首屏的片尾字幕、二十六种风格的署名榜、画册里的作品、creditia 自己的署名。
// 演示用虚构的样例项目；creditia 自己那一块用仓库里的 credits.json。
import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync, cpSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { buildModel, STYLES, styleById, art, exporter } from '../packages/render/src/index.ts';
import type { Art } from '../packages/render/src/export.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF } from '../packages/render/src/style.ts';
import { TOOL_NAMES } from '../packages/render/src/data.ts';
import { mountHits, INTERACT_CSS } from '../packages/render/src/interact.ts';
import * as C from '../packages/render/src/classic/index.ts';
import { ORDERED_THEMES, themeDot, themeName, themeNote } from '../packages/render/src/classic/catalog.ts';

const ROOT = path.resolve(import.meta.dirname, '..');
const OUT = path.resolve(process.argv[2] ?? path.join(ROOT, '_site'));
rmSync(OUT, { recursive: true, force: true });
mkdirSync(path.join(OUT, 'assets'), { recursive: true });

const sample = JSON.parse(readFileSync(path.join(ROOT, 'packages/render/fixtures/lumen.json'), 'utf8'));
const own = existsSync(path.join(ROOT, 'credits.json')) ? JSON.parse(readFileSync(path.join(ROOT, 'credits.json'), 'utf8')) : null;
const LANGS = ['zh', 'en'] as const;

/** 写一张作品：文字转路径的 SVG + 尺寸与悬停区域 */
function put(dir: string, name: string, a: Art) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, `${name}.svg`), exporter.svg(a));
  writeFileSync(path.join(dir, `${name}.json`), JSON.stringify({ w: a.w, h: a.h, bg: a.bg, hits: a.hits }));
  return { w: a.w, h: a.h, bg: a.bg };
}

const meta: any = { styles: {}, album: [], roll: {}, self: null };

// creditia 自己的完整署名页：交给命令行生成。放在最前面跑，这时本进程还没载入字体，两个进程加起来不会太占内存
if (own) {
  for (const lang of LANGS) {
    const r = spawnSync(process.execPath, [path.join(ROOT, 'packages/cli/bin/creditia.mjs'), 'site', '--credits', path.join(ROOT, 'credits.json'), '--style', 'minimal', '--lang', lang, '--out', path.join(OUT, lang === 'zh' ? 'self' : 'self/en')], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error('creditia site 失败');
  }
  console.log('✓ self');
}

// 首屏：黄金时代风格的片尾字幕，整张长图在银幕里滚
for (const lang of LANGS) {
  const m = buildModel(sample, { lang });
  meta.roll[lang] = put(path.join(OUT, 'assets/roll'), lang, art('roll', m, styleById('cinema')!, { variant: 'scroll' })!);
}
console.log('✓ roll');

// 风格：原版的十五个主题（satori 排版）+ 新做的十一种（和原版重名、或原版已有对应的不重复）
const V2_SKIP = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const V2_STYLES = STYLES.filter((s) => !V2_SKIP.has(s.id));
for (const lang of LANGS) {
  const m = buildModel(sample, { lang });
  for (const st of V2_STYLES) meta.styles[st.id] = put(path.join(OUT, 'assets/styles', lang), st.id, art('tier', m, st)!);
  const cm = C.buildModel(sample, { director: false, lang });
  for (const t of ORDERED_THEMES) {
    const a = (await C.art('tier', cm, t, { lang }))!;
    mkdirSync(path.join(OUT, 'assets/styles', lang), { recursive: true });
    writeFileSync(path.join(OUT, 'assets/styles', lang, `${t.id}.svg`), a.svg);
    writeFileSync(path.join(OUT, 'assets/styles', lang, `${t.id}.json`), JSON.stringify({ w: a.w, h: a.h, bg: a.bg, hits: a.hits }));
  }
  console.log('✓ styles', lang);
}

// 画册：挑结构最不一样的一组
const ALBUM: [string, string, string?][] = [
  ['roll', 'woodblock', 'vertical'], ['tier', 'cyber', 'hud'], ['roll', 'meadow', 'ed'], ['hand', 'ink'],
  ['tier', 'manga', 'stats'], ['receipt', 'manuscript'], ['donut', 'grimoire'], ['tier', 'baroque', 'podium'],
  ['stream', 'meadow'], ['clock', 'cyber'], ['roll', 'cutout', 'billing'], ['waffle', 'pastel'],
];
for (const lang of LANGS) {
  const m = buildModel(sample, { lang });
  for (const [id, sid, v] of ALBUM) {
    const name = `${id}-${sid}${v ? '-' + v : ''}`;
    const d = put(path.join(OUT, 'assets/album', lang), name, art(id, m, styleById(sid)!, { variant: v })!);
    if (lang === 'zh') meta.album.push({ name, piece: id, style: sid, variant: v ?? '', ...d });
  }
  console.log('✓ album', lang);
}

// creditia 自己：一张真实数据的署名榜
if (own) for (const lang of LANGS) meta.self = put(path.join(OUT, 'assets/self'), lang, art('tier', buildModel(own, { lang }), styleById('minimal')!)!);

// 社交预览图
for (const lang of LANGS) writeFileSync(path.join(OUT, `assets/og-${lang}.png`), exporter.png(art('share', buildModel(sample, { lang }), styleById('cinema')!)!, 1));

const tools = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'copilot', 'kimi', 'grok', 'devin', 'jules', 'aider', 'amp'].map((k) => {
  const lg = LOGOS[LOGO_OF[k] ?? ''];
  const name = TOOL_NAMES[k] ?? k;
  return { name, svg: lg ? `<svg viewBox="${lg.vb}" aria-hidden="true"><path d="${lg.d}"/></svg>` : `<b>${name.split(/\s+/).map((w) => w[0]).join('').slice(0, 2)}</b>` };
});
const styles = [
  ...ORDERED_THEMES.map((t) => ({ id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'), note: themeNote(t, 'zh'), noteEn: themeNote(t, 'en'), dot: themeDot(t) })),
  ...V2_STYLES.map((s) => ({ id: s.id, name: s.name, en: s.en, note: s.note, noteEn: '', dot: s.dot() })),
];
const logo = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8').replace(/#0E0E0E/g, 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');

let html = readFileSync(path.join(import.meta.dirname, 'index.html'), 'utf8');
html = html
  .replace('/*__INTERACT_CSS__*/', INTERACT_CSS)
  .replace('/*__DATA__*/', `const DATA = ${JSON.stringify({ ...meta, styles, tools, styleMeta: meta.styles, hasSelf: !!own })};\n${mountHits.toString()}`)
  .replaceAll('<!--__LOGO__-->', logo);
writeFileSync(path.join(OUT, 'index.html'), html);
writeFileSync(path.join(OUT, 'assets/favicon.svg'), logo.replace(/currentColor/g, '#111'));
cpSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), path.join(OUT, 'assets/logo.svg'));
writeFileSync(path.join(OUT, '.nojekyll'), '');
console.log(`→ ${OUT}/index.html`);
