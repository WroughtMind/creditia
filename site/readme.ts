// README 用的图：node site/readme.ts → .github/readme/
// 代码地图（和官网首屏同一套算法，SVG 动画，深浅两版 × 中英文）、风格拼图（PNG）、工具 logo、creditia 自己的 logo。
// 图都是从样例项目 lumen 现画的，改了渲染器或样例以后重跑一遍就行。
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { buildModel, styleById, art, exporter } from '../packages/render/src/index.ts';
import { fetchSegs } from '../packages/render/src/text.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF } from '../packages/render/src/style.ts';
import * as C from '../packages/render/src/classic/index.ts';
import { ORDERED_THEMES } from '../packages/render/src/classic/catalog.ts';

const ROOT = path.resolve(import.meta.dirname, '..');
const OUT = path.join(ROOT, '.github/readme');
mkdirSync(path.join(OUT, 'logo'), { recursive: true });
const sample = JSON.parse(readFileSync(path.join(ROOT, 'packages/render/fixtures/lumen.json'), 'utf8'));
const LANGS = ['zh', 'en'] as const;
type Lang = (typeof LANGS)[number];
const put = (rel: string, data: string | Uint8Array) => { writeFileSync(path.join(OUT, rel), data); console.log(`  ${rel}  ${(Buffer.byteLength(data as any) / 1024).toFixed(0)} KB`); };
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');

// ───────── 代码地图 ─────────
// 一个文件一段、一行一道线；先是一片灰，一道光从左扫到右，扫过的行染上写它的工具的颜色。十秒一轮
const FT = ['claude-code', 'codex', 'cursor', 'kimi', 'grok', 'unattributed'];
const AGC: Record<string, string> = { 'claude-code': '#D97757', codex: '#4FA37F', cursor: '#7D8AE0', kimi: '#C59A4C', grok: '#9C978D' };
const THEMES = {
  dark: { bg: '#070708', ink: '#F1EADC', soft: '#A39B8B', line: '#1B1A18' },
  light: { bg: '#F3F1EC', ink: '#161512', soft: '#67635B', line: '#E2DED5' },
};
const W = 1280, H = 560, MX = 48, MY = 84, MW = W - 2 * MX, MH = 352;
const PITCH = 4.5, CW = 56, GAP = 22, SHADE = [1, 0.78, 0.6];

function rng(seed: number) { return () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }

type Tok = { x: number; y: number; w: number; t: number; s: number };
function lay(seed: number, wt: number[]): Tok[] {
  const R = rng(seed), tot = wt.reduce((a, b) => a + b, 0);
  const pick = () => { let v = R() * tot; for (let k = 0; k < wt.length; k++) { v -= wt[k]; if (v <= 0) return k; } return 0; };
  const cols = Math.floor((MW + GAP) / (CW + GAP)), rows = Math.floor(MH / PITCH), cw = (MW - (cols - 1) * GAP) / cols;
  const out: Tok[] = [];
  for (let c = 0; c < cols; c++) {
    let y = Math.floor(R() * 5);
    while (y < rows - 6) {
      const n = Math.min(rows - y, Math.max(10, Math.round(Math.exp(2.8 + R() * 1.6))));
      const dom = pick(); let tool = dom, block = 0, ind = 0;
      for (let i = 0; i < n; i++) {
        if (block <= 0) { block = 6 + Math.floor(R() * 40); tool = R() < 0.8 ? dom : pick(); }
        block--;
        if (i > 0 && R() < 0.13) continue;
        ind = i === 0 ? 0 : Math.max(0, Math.min(4, ind + (R() < 0.22 ? 1 : R() < 0.25 ? -1 : 0)));
        const step = cw / 11, x0 = MX + c * (cw + GAP) + ind * step, end = x0 + Math.max(3, (cw - ind * step) * (0.25 + R() * 0.75));
        for (let x = x0, first = true; x < end - 1.5; first = false) {
          const sw = Math.min(end - x, cw / 7 + R() * (cw / 2.2));
          out.push({ x, y: MY + (y + i) * PITCH, w: sw, t: tool, s: first ? 0 : R() < 0.45 ? 1 : R() < 0.5 ? 2 : 0 });
          x += sw + 3;
        }
      }
      y += n + 2;
    }
  }
  return out;
}

const share = (lang: Lang) => fetchSegs(buildModel(sample, { lang })).filter((l) => l.length >= 4 && l[0].color)
  .map((l) => ({ key: l[0].color as string, name: l[0].text.trim(), pct: parseFloat(l[3].text) }));

function bestLayout(): Tok[] {
  const sh = share('zh'), want = FT.map((k) => (sh.find((r) => r.key === k)?.pct || 0) / 100);
  let best: { err: number; toks: Tok[] } | null = null;
  for (let seed = 1; seed <= 40; seed++) {
    const toks = lay(seed, want), n = [0, 0, 0, 0, 0, 0];
    for (const t of toks) n[t.t]++;
    const err = n.reduce((e, v, k) => e + Math.abs(v / toks.length - want[k]), 0);
    if (!best || err < best.err) best = { err, toks };
    if (err < 0.03) break;
  }
  return best!.toks;
}

const f1 = (v: number) => +v.toFixed(1);
const pathOf = (toks: Tok[]) => toks.map((t) => `M${f1(t.x)} ${f1(t.y + 1)}h${f1(t.w)}`).join('');
const logoSvg = (key: string, x: number, y: number, size: number, fill: string) => {
  const lg = LOGOS[LOGO_OF[key] ?? ''];
  if (!lg) return `<circle cx="${x + size / 2}" cy="${y + size / 2}" r="${size / 2 - 1}" fill="none" stroke="${fill}" stroke-dasharray="2 2"/>`;
  return `<svg x="${x}" y="${y}" width="${size}" height="${size}" viewBox="${lg.vb}"><path d="${lg.d}" fill="${fill}"${lg.rule ? ` fill-rule="${lg.rule}"` : ''}/></svg>`;
};

function mapSvg(toks: Tok[], theme: keyof typeof THEMES, lang: Lang): string {
  const c = THEMES[theme], en = lang === 'en', sh = share(lang);
  const colorOf = (k: number) => (k === 5 ? c.soft : AGC[FT[k]]);
  const sans = `ui-sans-serif,-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Hiragino Sans GB','Microsoft YaHei','Noto Sans SC',sans-serif`;
  const mono = `ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace`;
  // 动画节奏：0–0.8s 静着，0.8–4.2s 扫过去，停到 9.2s，最后 0.8s 颜色褪回灰
  const DUR = '10s', KT = '0;.08;.42;1', SPL = '0 0 1 1;.65 0 .35 1;0 0 1 1';
  const sweep = (attr: string, a: string, b: string) => `<animate attributeName="${attr}" values="${a};${a};${b};${b}" keyTimes="${KT}" calcMode="spline" keySplines="${SPL}" dur="${DUR}" repeatCount="indefinite"/>`;
  const move = `<animateTransform attributeName="transform" type="translate" values="0 0;0 0;${W} 0;${W} 0" keyTimes="${KT}" calcMode="spline" keySplines="${SPL}" dur="${DUR}" repeatCount="indefinite"/>`;
  const layers: string[] = [];
  for (let k = 0; k < FT.length; k++) for (let s = 0; s < 3; s++) {
    const g = toks.filter((t) => t.t === k && t.s === s);
    if (g.length) layers.push(`<path d="${pathOf(g)}" stroke="${colorOf(k)}" stroke-opacity="${+(SHADE[s] * (k === 5 ? 0.6 : 1)).toFixed(2)}"/>`);
  }
  // 占比条和图例
  let bx = MX;
  const bar = sh.map((r) => { const w = (r.pct / 100) * MW, s = `<rect x="${f1(bx)}" y="458" width="${f1(Math.max(0, w - 2))}" height="4" fill="${colorOf(FT.indexOf(r.key))}"/>`; bx += w; return s; }).join('');
  let lx = MX;
  const legend = sh.map((r) => {
    const k = FT.indexOf(r.key), col = colorOf(k), name = r.key === 'unattributed' ? (en ? 'Unattributed' : '未确认') : r.name;
    const nameW = [...name].reduce((a, ch) => a + (ch.charCodeAt(0) > 0x2e80 ? 14 : 8), 0);
    const s = (r.key === 'unattributed' ? `<rect x="${lx + 1}" y="497" width="12" height="12" rx="3" fill="none" stroke="${c.soft}" stroke-dasharray="2 2"/>` : logoSvg(r.key, lx, 496, 14, col))
      + `<text x="${lx + 22}" y="508" font-family="${sans}" font-size="14" fill="${c.ink}">${esc(name)}<tspan dx="8" font-family="${mono}" font-size="13" fill="${c.soft}">${r.pct.toFixed(1)}%</tspan></text>`;
    lx += 22 + nameW + 8 + 40 + 48;
    return s;
  }).join('');
  const lines = 49360, cap = en ? `${lines.toLocaleString('en-US')} lines · 612 commits · 112 days` : `${lines.toLocaleString('en-US')} 行 · 612 次提交 · 112 天`;
  const note = en ? 'every line, matched to the agent that wrote it' : '每一行，对上写它的 AI';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}">
<defs>
<path id="all" d="${pathOf(toks)}"/>
<clipPath id="cut"><rect x="0" y="0" width="0" height="${H}">${sweep('width', '0', String(W))}</rect></clipPath>
<linearGradient id="glow" x1="0" x2="1"><stop offset="0" stop-color="${c.ink}" stop-opacity="0"/><stop offset="1" stop-color="${c.ink}" stop-opacity=".09"/></linearGradient>
<linearGradient id="fl" x1="0" x2="1"><stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#fff"/></linearGradient>
<mask id="flash" maskUnits="userSpaceOnUse" x="0" y="0" width="${W}" height="${H}"><rect x="-150" y="0" width="150" height="${H}" fill="url(#fl)">${move}</rect></mask>
</defs>
<rect width="${W}" height="${H}" rx="18" fill="${c.bg}"/>
<text x="${MX}" y="50" font-family="${mono}" font-size="15" fill="${c.ink}">lumen</text>
<text x="${MX + 62}" y="50" font-family="${sans}" font-size="14" fill="${c.soft}">${esc(note)}</text>
<text x="${W - MX}" y="50" text-anchor="end" font-family="${mono}" font-size="13" fill="${c.soft}">${esc(cap)}</text>
<g fill="none" stroke-width="2">
<use href="#all" stroke="${c.ink}" stroke-opacity=".16"/>
<g clip-path="url(#cut)">${layers.join('')}<animate attributeName="opacity" values="1;1;0" keyTimes="0;.92;1" dur="${DUR}" repeatCount="indefinite"/></g>
<use href="#all" stroke="${c.ink}" stroke-opacity=".7" mask="url(#flash)"/>
</g>
<g opacity="0"><rect x="-90" y="${MY - 12}" width="90" height="${MH + 20}" fill="url(#glow)"/><rect x="-1" y="${MY - 12}" width="1" height="${MH + 20}" fill="${c.ink}" opacity=".85"/>${move}<animate attributeName="opacity" values="0;0;1;1;0;0" keyTimes="0;.07;.09;.4;.43;1" dur="${DUR}" repeatCount="indefinite"/></g>
<rect x="${MX}" y="458" width="${MW}" height="4" fill="${c.ink}" fill-opacity=".1"/>
<g clip-path="url(#cut)">${bar}<animate attributeName="opacity" values="1;1;0" keyTimes="0;.92;1" dur="${DUR}" repeatCount="indefinite"/></g>
${legend}
</svg>
`;
}

console.log('code map');
const toks = bestLayout();
for (const theme of ['dark', 'light'] as const) for (const lang of LANGS) put(`map-${theme}${lang === 'en' ? '-en' : ''}.svg`, mapSvg(toks, theme, lang));

// ───────── logo ─────────
console.log('logos');
const own = readFileSync(path.join(ROOT, 'packages/render/assets/creditia/logo.svg'), 'utf8');
put('logo-light.svg', own);
put('logo-dark.svg', own.replaceAll('#0E0E0E', '#F1EADC'));
// 工具表里的小 logo：中灰，深浅底都看得清
const TOOLS = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'opencode', 'cline', 'roo', 'kilo', 'grok', 'kimi', 'copilot', 'devin', 'jules', 'aider'];
for (const k of TOOLS) {
  const lg = LOGOS[LOGO_OF[k]];
  if (lg) writeFileSync(path.join(OUT, 'logo', `${k}.svg`), `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${lg.vb}" width="20" height="20"><path d="${lg.d}" fill="#8A877F"${lg.rule ? ` fill-rule="${lg.rule}"` : ''}/></svg>\n`);
}

// ───────── 风格拼图 ─────────
// 两条带子，每条按宽高比排满一行。每张先单独出 PNG，再嵌进一张大 SVG 里一起栅格化
type Item = [style: string, piece: string, variant?: string];
const ROWS: Item[][] = [
  [['cinema', 'roll', 'cast'], ['ukiyoe', 'share'], ['woodblock', 'tier', 'vertical'], ['manga', 'tier', 'stats'], ['cyber', 'tier', 'hud']],
  [['neon', 'donut'], ['broadsheet', 'versions'], ['grimoire', 'receipt', 'ticket'], ['film', 'fetch'], ['baroque', 'tier', 'podium']],
];
const CW_ = 1600, CGAP = 14;
const isClassic = (id: string) => ORDERED_THEMES.some((t) => t.id === id);
async function piece([style, p, variant]: Item, lang: Lang) {
  if (isClassic(style)) {
    const a = (await C.art(p, C.buildModel(sample, { director: false, lang }), C.themeById(style)!, { lang, variant }))!;
    return { ar: a.w / a.h, png: (w: number) => C.raster.png(a.svg, w) };
  }
  const a = art(p, buildModel(sample, { lang }), styleById(style)!, { variant })!;
  return { ar: a.w / a.h, png: (w: number) => exporter.png(a, w / a.w) };
}
console.log('styles collage');
for (const lang of LANGS) {
  const parts: string[] = [];
  let y = 0;
  for (const row of ROWS) {
    const ps = [];
    for (const it of row) ps.push(await piece(it, lang));
    const h = (CW_ - CGAP * (row.length - 1)) / ps.reduce((a, p) => a + p.ar, 0);
    let x = 0;
    for (const p of ps) {
      const w = p.ar * h, id = `c${parts.length}`;
      const b64 = Buffer.from(p.png(Math.round(w * 1.5))).toString('base64');
      parts.push(`<clipPath id="${id}"><rect x="${f1(x)}" y="${f1(y)}" width="${f1(w)}" height="${f1(h)}" rx="10"/></clipPath><image href="data:image/png;base64,${b64}" x="${f1(x)}" y="${f1(y)}" width="${f1(w)}" height="${f1(h)}" preserveAspectRatio="none" clip-path="url(#${id})"/>`);
      x += w + CGAP;
    }
    console.log(`  row ${lang}: h=${h.toFixed(0)}`);
    y += h + CGAP;
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${CW_} ${f1(y - CGAP)}" width="${CW_}" height="${f1(y - CGAP)}">${parts.join('')}</svg>`;
  put(`styles${lang === 'en' ? '-en' : ''}.png`, C.raster.png(svg, CW_));
}
