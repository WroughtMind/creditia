// 官网构建：node site/build.ts [输出目录]
// 首页的主图、作品墙都是用真实的渲染器现做的；首页演示用虚构的样例项目，creditia 自己的署名页用仓库里的 credits.json。
import { readFileSync, writeFileSync, mkdirSync, rmSync, existsSync, cpSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { buildModel, STYLES, styleById, art, exporter } from '../packages/render/src/index.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF, BRAND } from '../packages/render/src/style.ts';
import { TOOL_NAMES, fmt } from '../packages/render/src/data.ts';

const ROOT = path.resolve(import.meta.dirname, '..');
const OUT = path.resolve(process.argv[2] ?? path.join(ROOT, '_site'));
rmSync(OUT, { recursive: true, force: true });
mkdirSync(path.join(OUT, 'assets'), { recursive: true });

const sample = JSON.parse(readFileSync(path.join(ROOT, 'packages/render/fixtures/lumen.json'), 'utf8'));
const own = existsSync(path.join(ROOT, 'credits.json')) ? JSON.parse(readFileSync(path.join(ROOT, 'credits.json'), 'utf8')) : null;
const langs = ['zh', 'en'] as const;

// ── 1. 主图：分享图 × 16 种风格 × 2 种语言，文字转路径的 SVG + 悬停区域 ──
const hero: Record<string, Record<string, { w: number; h: number }>> = { zh: {}, en: {} };
for (const lang of langs) {
  const m = buildModel(sample, { lang });
  const dir = path.join(OUT, 'assets/hero', lang);
  mkdirSync(dir, { recursive: true });
  for (const st of STYLES) {
    const a = art('share', m, st)!;
    writeFileSync(path.join(dir, `${st.id}.svg`), exporter.svg(a));
    writeFileSync(path.join(dir, `${st.id}.json`), JSON.stringify({ w: a.w, h: a.h, bg: a.bg, hits: a.hits }));
    hero[lang][st.id] = { w: a.w, h: a.h };
  }
  console.log('✓ hero', lang);
}

// ── 2. 作品墙：挑一组最能说明「同一份数据，很多种样子」的作品 ──
const GALLERY: [string, string, string?][] = [
  ['roll', 'cinema', 'classic'], ['tier', 'ukiyoe'], ['tier', 'manga', 'stats'], ['roll', 'woodblock', 'vertical'],
  ['tier', 'cyber', 'hud'], ['receipt', 'manuscript'], ['hand', 'ink'], ['stream', 'glass'], ['tier', 'baroque', 'podium'],
  ['roll', 'meadow', 'ed'], ['waffle', 'pastel'], ['receipt', 'terminal', 'ticket'], ['donut', 'grimoire'], ['tier', 'cutout'],
  ['calendar', 'noir'], ['badge', 'minimal', 'stack'],
];
const gallery: { file: string; piece: string; style: string; variant: string; w: number; h: number }[] = [];
for (const lang of langs) {
  const m = buildModel(sample, { lang });
  const dir = path.join(OUT, 'assets/gallery', lang);
  mkdirSync(dir, { recursive: true });
  for (const [id, sid, v] of GALLERY) {
    const a = art(id, m, styleById(sid)!, { variant: v })!;
    const file = `${id}-${sid}${v ? '-' + v : ''}.png`;
    writeFileSync(path.join(dir, file), exporter.png(a, Math.min(2, 1100 / a.w)));
    if (lang === 'zh') gallery.push({ file, piece: id, style: sid, variant: v ?? '', w: a.w, h: a.h });
  }
  console.log('✓ gallery', lang);
}

// ── 3. creditia 自己的署名页（真实数据） ──
let ownStats: any = null;
if (own) {
  for (const lang of langs) {
    const r = spawnSync(process.execPath, [path.join(ROOT, 'packages/cli/bin/creditia.mjs'), 'site', '--credits', path.join(ROOT, 'credits.json'), '--style', 'minimal', '--lang', lang, '--out', path.join(OUT, lang === 'zh' ? 'self' : 'self/en')], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error('creditia site 失败');
  }
  const m = buildModel(own, { lang: 'zh' });
  ownStats = { lines: m.linesAdded, commits: m.commits, rows: m.rows.map((r) => ({ key: r.key, name: r.name, pct: r.pct, models: r.models.map((x) => x.name) })), verified: m.verifiedShare };
}

// ── 4. 支持的工具（单色 logo） ──
const tools = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'copilot', 'kimi', 'grok', 'devin', 'jules', 'aider', 'amp'].map((k) => {
  const lg = LOGOS[LOGO_OF[k] ?? ''];
  return { key: k, name: TOOL_NAMES[k] ?? k, svg: lg ? `<svg viewBox="${lg.vb}" aria-hidden="true"><path d="${lg.d}"/></svg>` : `<span class="mono">${(TOOL_NAMES[k] ?? k).split(/\s+/).map((w) => w[0]).join('').slice(0, 2)}</span>` };
});

// ── 5. 拼页面 ──
const styles = STYLES.map((s) => ({ id: s.id, name: s.name, en: s.en, note: s.note, dot: s.dot(), dark: s.dark }));
const logo = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8').replace(/#0E0E0E/g, 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');
const interact = readFileSync(path.join(ROOT, 'packages/render/src/interact.ts'), 'utf8');
const css = /export const INTERACT_CSS = `([\s\S]*?)`;/.exec(interact)![1];
const mount = await import('../packages/render/src/interact.ts').then((x) => x.mountHits.toString());
let html = readFileSync(path.join(import.meta.dirname, 'index.html'), 'utf8');
const data = { hero, gallery, styles, tools, own: ownStats, brand: BRAND };
html = html
  .replace('/*__INTERACT_CSS__*/', css)
  .replace('/*__DATA__*/', `const DATA = ${JSON.stringify(data)};\n${mount}`)
  .replaceAll('<!--__LOGO__-->', logo)
  .replace('__OWN_LINES__', ownStats ? fmt(ownStats.lines) : '—');
writeFileSync(path.join(OUT, 'index.html'), html);
cpSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), path.join(OUT, 'assets/logo.svg'));
writeFileSync(path.join(OUT, 'assets/favicon.svg'), logo.replace(/currentColor/g, '#111'));
writeFileSync(path.join(OUT, '.nojekyll'), '');
// 社交预览图（X、微信等不认 SVG）
for (const lang of langs) writeFileSync(path.join(OUT, `assets/og-${lang}.png`), exporter.png(art('share', buildModel(sample, { lang }), styleById('minimal')!)!, 1));
console.log(`→ ${OUT}/index.html`);
