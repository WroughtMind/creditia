// 故事四套：打字机手稿（文学）、夏日草原（手绘动画）、对称粉彩（Wes Anderson 式电影）、剪纸片头（Saul Bass 式电影）。
import { defineStyle, grain, linear, radial, blur, roughen, logo, type Style } from '../style.ts';
import { Scene, el, g, rect, circle, line, path, r, rng, hash, mix, measure } from '../svg.ts';

// ───────────────────────── 打字机手稿 ─────────────────────────

const TYPE = '#24221F', RED = '#C2382B', SHEET = '#F3EDE1';
const TW = { family: 'Courier Prime, Noto Serif SC', weight: 400 };
const MS_GLYPH = ['=', '#', '*', '+', '~', ':'];

export const manuscript: Style = defineStyle({
  id: 'manuscript', name: '手稿', en: 'Manuscript', note: '打字机打出的书稿，编辑用红铅笔批注，咖啡渍还在', noteEn: 'A typewritten manuscript with an editor\'s red-pencil notes and a coffee ring', dark: false,
  c: { bg: '#E7DFCF', paper: SHEET, ink: TYPE, soft: '#5E5850', faint: '#A39B8D', line: 'rgba(36,34,31,0.2)', accent: RED, accent2: RED },
  series: [TYPE, '#4A4640', '#6F6A62', '#8F897F', '#ABA59A', '#C3BDB1'],
  f: { display: { family: 'Courier Prime, Noto Serif SC', weight: 700 }, body: TW, num: { family: 'Courier Prime, Noto Serif SC', weight: 700 }, label: TW, mono: TW, script: { family: 'Long Cang, Noto Serif SC' } },
  labelTrack: 0.08, prefer: { tier: 'rows', roll: 'classic' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#F3EDE1" stroke="#24221F" stroke-width=".8"/><path d="M6 9h12M6 12h9M6 15h11" stroke="#24221F" stroke-width="1.1"/><ellipse cx="13" cy="12" rx="5.6" ry="3.2" fill="none" stroke="#C2382B" stroke-width="1" transform="rotate(-8 13 12)"/>`,
  ground(s, b, m) {
    s.bg = '#E7DFCF';
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#E7DFCF' }));
    // 一张纸，略微歪一点，底下还压着一张
    const p = 26;
    s.add(rect(b.x + p + 6, b.y + p + 8, b.w - p * 2, b.h - p * 2, { fill: '#E9E1D2', transform: `rotate(0.8 ${r(b.x + b.w / 2)} ${r(b.y + b.h / 2)})`, filter: blur(s, 1) }));
    s.add(rect(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, { fill: SHEET }));
    s.add(rect(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, { fill: radial(s, 'ms-age', [[0.6, '#000000', 0], [1, '#8A6A3A', 0.16]], { cx: 0.5, cy: 0.5, r: 0.75 }) }));
    // 咖啡渍
    const rnd = rng(hash(m.project) + 9);
    const cx = b.x + b.w * (0.72 + rnd() * 0.12), cy = b.y + b.h * (0.75 + rnd() * 0.12), R = Math.min(b.w, b.h) * 0.13;
    s.add(g({ filter: roughen(s, 'ms-cof', 4, 0.05, 3) }, circle(cx, cy, R, { fill: 'none', stroke: '#8B5A2B', strokeOpacity: 0.22, strokeWidth: R * 0.06 }), circle(cx, cy, R * 0.94, { fill: '#8B5A2B', opacity: 0.045 })));
    // 回形针
    const px = b.x + b.w * 0.16, py = b.y + p - 14;
    s.add(path(`M${r(px)} ${r(py + 70)}V${r(py + 8)}a10 10 0 0 1 20 0V${r(py + 60)}a6 6 0 0 1 -12 0V${r(py + 18)}`, { fill: 'none', stroke: '#8D9399', strokeWidth: 2.4, strokeLinecap: 'round' }));
    grain(s, b, { opacity: 0.08, seed: 81, color: '#3A3020' });
  },
  plate(s, b) { return { x: b.x + 20, y: b.y + 14, w: b.w - 40, h: b.h - 28 }; },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    const w = s.text(text.toUpperCase(), x, y, { font: manuscript.f.display, size: size * 0.82, fill: o.fill ?? TYPE, anchor, maxW: o.maxW, track: 0.04, extra: { filter: roughen(s, 'ms-t', 0.7, 0.9, 2) } });
    // 编辑的红铅笔下划线
    const x0 = anchor === 'middle' ? x - w / 2 : anchor === 'end' ? x - w : x;
    s.add(path(`M${r(x0 - 4)} ${r(y + 10)}Q${r(x0 + w / 2)} ${r(y + 15)} ${r(x0 + w + 8)} ${r(y + 8)}`, { fill: 'none', stroke: RED, strokeWidth: 2.4, strokeLinecap: 'round', opacity: 0.85 }));
    return w;
  },
  num(s, text, x, y, size, anchor = 'end', o = {}) { return s.text(text, x, y, { font: manuscript.f.num, size: size * 0.8, fill: o.fill ?? TYPE, anchor, extra: { filter: roughen(s, 'ms-n', 0.6, 0.9, 4) } }); },
  bar(s, x, y, w, h, color, i) {
    if (h > w * 1.5) { s.add(rect(x, y, w, h, { fill: color, filter: roughen(s, 'ms-bv', 0.8, 0.6, 6) })); return; }
    // 打字机的字符串：每一档灰配一个字符，图例和条里按颜色对得上
    const k = manuscript.series.indexOf(color), ch = color === RED ? 'x' : MS_GLYPH[(k < 0 ? i ?? 0 : k) % MS_GLYPH.length];
    // 图例里的小方块：只放得下一个字符，就把它打大、打粗，填满方块
    if (w <= h * 1.5) {
      s.text(ch, x + w / 2, y + h * 0.5 + h * 0.42, { font: manuscript.f.num, size: h * 1.5, fill: color, anchor: 'middle', extra: { filter: roughen(s, 'ms-b', 0.6, 0.9, 6) } });
      return;
    }
    const size = 16, cw = measure(ch, TW, size);
    const n = Math.max(1, Math.floor(w / cw));
    s.text(ch.repeat(n), x, y + h / 2 + 5, { font: TW, size, fill: color, extra: { filter: roughen(s, 'ms-b', 0.6, 0.9, 6) } });
  },
  track(s, x, y, w, h) { s.text('.'.repeat(Math.floor(w / measure('.', TW, 16))), x, y + h / 2 + 5, { font: TW, size: 16, fill: TYPE, opacity: 0.25 }); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.text('?', cx, cy + 7, { font: TW, size: 22, fill: RED, anchor: 'middle' }); return; }
    s.add(logo(key, cx, cy, size * 0.72, TYPE));
  },
  rule(s, x, y, w) { s.text('- '.repeat(Math.floor(w / measure('- ', TW, 14))), x, y + 4, { font: TW, size: 14, fill: TYPE, opacity: 0.5 }); },
  hatch(s) { return s.url('ms-h', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 6, 6, { fill: SHEET }), rect(0, 0, 1.2, 6, { fill: RED, opacity: 0.6 }))); },
  color(key, rank) { return key === 'human' || key === 'unattributed' ? RED : manuscript.series[Math.min(rank, 5)]; },
});

// ───────────────────────── 夏日草原 ─────────────────────────

const LEAF = '#2F3B2A';
/** 积云：一团团圆叠起来，底部带一点阴影 */
export function cloud(s: Scene, cx: number, cy: number, W: number, seed: number) {
  const rnd = rng(seed);
  const parts: string[] = [];
  const n = 7 + Math.floor(rnd() * 4);
  for (let i = 0; i < n; i++) {
    const t = i / (n - 1);
    const rr = W * (0.12 + 0.12 * Math.sin(Math.PI * t) + rnd() * 0.05);
    parts.push(circle(cx - W / 2 + W * t, cy - rr * 0.55 * (0.6 + Math.sin(Math.PI * t)), rr, { fill: '#FFFFFF' }));
  }
  s.add(g({ filter: blur(s, 1.4) }, el('ellipse', { cx, cy: cy + W * 0.06, rx: W * 0.56, ry: W * 0.1, fill: '#C9DCEB' }), ...parts));
  s.add(el('ellipse', { cx, cy: cy + W * 0.02, rx: W * 0.5, ry: W * 0.06, fill: '#DCE9F3', opacity: 0.8, filter: blur(s, 4) }));
}
export const meadow: Style = defineStyle({
  id: 'meadow', name: '夏日', en: 'Summer Meadow', note: '手绘动画里的夏天：积云、山坡、风吹过草地', noteEn: 'Summer in hand-drawn animation: cumulus clouds, hillsides, wind over the grass', dark: false,
  c: { bg: '#BFE3F2', paper: '#FFFDF6', ink: LEAF, soft: '#5E6B57', faint: '#A3AE9C', line: 'rgba(47,59,42,0.12)', accent: '#E07A5F', accent2: '#3D85C6' },
  series: ['#E07A5F', '#3D85C6', '#F2B84B', '#6FA67F', '#9C6ADE', '#E68FA8'],
  f: {
    display: { family: 'Klee One, Noto Serif SC', weight: 600 }, body: { family: 'Zen Maru Gothic, Noto Sans SC', weight: 500 },
    num: { family: 'Klee One, Noto Serif SC', weight: 600 }, label: { family: 'Zen Maru Gothic, Noto Sans SC', weight: 700 }, mono: { family: 'Zen Maru Gothic, Noto Sans SC' },
  },
  labelUpper: false, labelTrack: 0.16, radius: 99, prefer: { tier: 'rows', roll: 'ed' }, reserve: { top: 60, bottom: 120 },
  dot: () => `<defs><linearGradient id="mdd" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7EC4E6"/><stop offset=".6" stop-color="#D6EEF7"/></linearGradient><clipPath id="mdc"><circle cx="12" cy="12" r="11"/></clipPath></defs><g clip-path="url(#mdc)"><rect width="24" height="24" fill="url(#mdd)"/><circle cx="9" cy="10" r="3.4" fill="#fff"/><circle cx="12.5" cy="8.6" r="4" fill="#fff"/><circle cx="16" cy="10.4" r="3" fill="#fff"/><path d="M0 18q8-5 24-1v8H0z" fill="#7FB45A"/><path d="M0 21q12-3 24 0v4H0z" fill="#5A9446"/></g>`,
  ground(s, b, m) {
    s.bg = '#BFE3F2';
    s.add(rect(b.x, b.y, b.w, b.h, { fill: linear(s, 'md-sky', [[0, '#6FB9E0'], [0.45, '#BFE3F2'], [0.78, '#F4F1DC'], [1, '#F7E9C6']]) }));
    const seed = hash(m.project);
    cloud(s, b.x + b.w * 0.24, b.y + b.h * 0.2, b.w * 0.5, seed + 1);
    cloud(s, b.x + b.w * 0.8, b.y + b.h * 0.34, b.w * 0.34, seed + 2);
    cloud(s, b.x + b.w * 0.55, b.y + b.h * 0.1, b.w * 0.22, seed + 3);
    // 山坡两层 + 草叶
    const H = b.h, base = b.y + H;
    s.add(path(`M${r(b.x)} ${r(base - H * 0.2)}C${r(b.x + b.w * 0.3)} ${r(base - H * 0.3)} ${r(b.x + b.w * 0.6)} ${r(base - H * 0.16)} ${r(b.x + b.w)} ${r(base - H * 0.26)}V${r(base)}H${r(b.x)}Z`, { fill: linear(s, 'md-h1', [[0, '#9CCB72'], [1, '#77AE58']]) }));
    s.add(path(`M${r(b.x)} ${r(base - H * 0.1)}C${r(b.x + b.w * 0.35)} ${r(base - H * 0.04)} ${r(b.x + b.w * 0.7)} ${r(base - H * 0.15)} ${r(b.x + b.w)} ${r(base - H * 0.08)}V${r(base)}H${r(b.x)}Z`, { fill: linear(s, 'md-h2', [[0, '#5E9C49'], [1, '#447E36']]) }));
    const rnd = rng(seed + 5);
    const blades: string[] = [];
    for (let i = 0; i < 140; i++) {
      const x = b.x + rnd() * b.w, y = base - rnd() * H * 0.1, hh = 6 + rnd() * 12, lean = 3 + rnd() * 6;
      blades.push(path(`M${r(x)} ${r(y)}q${r(lean * 0.3)} ${r(-hh * 0.6)} ${r(lean)} ${r(-hh)}`, { fill: 'none', stroke: rnd() > 0.5 ? '#3E7330' : '#8CC266', strokeWidth: 1.4, strokeLinecap: 'round' }));
    }
    for (let i = 0; i < 18; i++) blades.push(circle(b.x + rnd() * b.w, base - rnd() * H * 0.12, 2 + rnd() * 1.6, { fill: ['#FFFFFF', '#F7D154', '#F29BB0'][i % 3] }));
    s.add(g({}, ...blades));
    // 风
    for (let i = 0; i < 3; i++) { const y = b.y + H * (0.42 + i * 0.07), x = b.x + b.w * (0.1 + rnd() * 0.5); s.add(path(`M${r(x)} ${r(y)}q${r(b.w * 0.08)} -10 ${r(b.w * 0.16)} 0t${r(b.w * 0.16)} 0`, { fill: 'none', stroke: '#FFFFFF', strokeWidth: 2, strokeLinecap: 'round', opacity: 0.6 })); }
  },
  plate(s, b) {
    const sh = s.url('md-sh', (id) => el('filter', { id, x: '-10%', y: '-10%', width: '120%', height: '130%' }, el('feDropShadow', { dx: 0, dy: 8, stdDeviation: 14, floodColor: '#2F5A3A', floodOpacity: 0.16 })));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#FFFDF6', opacity: 0.9, rx: 28, filter: sh }));
    s.add(rect(b.x + 8, b.y + 8, b.w - 16, b.h - 16, { fill: 'none', rx: 22, stroke: '#2F3B2A', strokeOpacity: 0.12, strokeWidth: 1.4, strokeDasharray: '6 6' }));
    return b;
  },
  bar(s, x, y, w, h, color, i) {
    if (w <= 0) return;
    const hh = Math.max(h, 12);
    // 水彩：边缘略晕开，中间浅一点
    s.add(rect(x, y - (hh - h) / 2, w, hh, { fill: color, opacity: 0.88, rx: Math.min(hh, w) / 2, filter: roughen(s, `md-w${i % 3}`, 2.2, 0.06, i + 2) }));
    s.add(rect(x + hh * 0.4, y - (hh - h) / 2 + hh * 0.25, Math.max(0, w - hh * 0.8), hh * 0.3, { fill: '#FFFFFF', opacity: 0.25, rx: hh * 0.15 }));
  },
  mark(s, key, cx, cy, size, color) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.4, { fill: 'none', stroke: '#A3AE9C', strokeWidth: 1.6, strokeDasharray: '3 4' })); return; }
    s.add(circle(cx, cy, size * 0.54, { fill: '#FFFFFF', stroke: color ?? LEAF, strokeWidth: 1.6 }));
    s.add(logo(key, cx, cy, size * 0.56, LEAF));
  },
  rule(s, x, y, w) { s.add(path(`M${r(x)} ${r(y)}q${r(w / 8)} -6 ${r(w / 4)} 0t${r(w / 4)} 0t${r(w / 4)} 0t${r(w / 4)} 0`, { fill: 'none', stroke: '#6FA67F', strokeWidth: 1.6, strokeLinecap: 'round', opacity: 0.6 })); },
});

// ───────────────────────── 对称粉彩 ─────────────────────────

const PINK = '#F2C4C6', ROSE = '#D98A95', WINE = '#6B2737', MINT = '#A8D8C8', MUSTARD = '#E2AE4C', CREAM = '#FBF1E6';
export const pastel: Style = defineStyle({
  id: 'pastel', name: '对称', en: 'Symmetry', note: '粉彩、居中对称、小号大写字和细框；像一部精致的旅馆电影', noteEn: 'Pastels, centred symmetry, small caps and hairline frames, like a fastidious hotel film', dark: false,
  c: { bg: PINK, paper: CREAM, ink: WINE, soft: '#9A5B66', faint: '#C9939B', line: 'rgba(107,39,55,0.2)', accent: MUSTARD, accent2: WINE },
  series: [WINE, '#E07A5F', '#5FA89B', MUSTARD, '#8E7CC3', ROSE],
  f: {
    display: { family: 'Jost, Noto Sans SC', weight: 500 }, body: { family: 'Jost, Noto Sans SC', weight: 400 }, num: { family: 'Jost, Noto Sans SC', weight: 500 },
    label: { family: 'Jost, Noto Sans SC', weight: 500 }, mono: { family: 'Jost, Noto Sans SC' }, script: { family: 'Pinyon Script, Noto Serif SC' },
  },
  labelTrack: 0.32, radius: 0, prefer: { tier: 'center', roll: 'card' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#F2C4C6"/><rect x="6.5" y="5.5" width="11" height="13" rx="1" fill="#FBF1E6" stroke="#6B2737" stroke-width=".8"/><rect x="8.5" y="8" width="7" height="1.2" fill="#6B2737"/><rect x="9.5" y="11" width="5" height="1" fill="#E2AE4C"/><rect x="9" y="13.6" width="6" height="1" fill="#A8D8C8"/>`,
  ground(s, b) {
    s.bg = PINK;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: PINK }));
    // 竖条墙纸
    const st = s.url('pa-stripe', (id) => el('pattern', { id, width: 36, height: 10, patternUnits: 'userSpaceOnUse' }, rect(0, 0, 18, 10, { fill: '#F6D2D3' })));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: st }));
    grain(s, b, { opacity: 0.05, seed: 91 });
  },
  plate(s, b) {
    const p = 26;
    s.add(rect(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, { fill: CREAM }));
    s.add(rect(b.x + p + 10, b.y + p + 10, b.w - p * 2 - 20, b.h - p * 2 - 20, { fill: 'none', stroke: WINE, strokeWidth: 1.5 }));
    s.add(rect(b.x + p + 15, b.y + p + 15, b.w - p * 2 - 30, b.h - p * 2 - 30, { fill: 'none', stroke: WINE, strokeWidth: 0.6 }));
    // 角上小方块
    for (const [x, y] of [[b.x + p + 10, b.y + p + 10], [b.x + b.w - p - 10, b.y + p + 10], [b.x + p + 10, b.y + b.h - p - 10], [b.x + b.w - p - 10, b.y + b.h - p - 10]]) s.add(rect(x - 5, y - 5, 10, 10, { fill: MUSTARD, stroke: WINE, strokeWidth: 1 }));
    return { x: b.x + p + 16, y: b.y + p + 16, w: b.w - (p + 16) * 2, h: b.h - (p + 16) * 2 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: pastel.f.display, size: size * 0.9, fill: o.fill ?? WINE, anchor, maxW: o.maxW, track: 0.18, upper: true });
  },
  bar(s, x, y, w, h, color) { s.add(rect(x, y, w, Math.max(h, 8), { fill: color })); },
  track(s, x, y, w, h) { s.add(rect(x, y + Math.max(h, 8) / 2 - 0.5, w, 1, { fill: WINE, opacity: 0.2 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(rect(cx - size * 0.35, cy - size * 0.35, size * 0.7, size * 0.7, { fill: 'none', stroke: ROSE, strokeWidth: 1.2, strokeDasharray: '3 3' })); return; }
    s.add(rect(cx - size * 0.48, cy - size * 0.48, size * 0.96, size * 0.96, { fill: MINT, stroke: WINE, strokeWidth: 1 }));
    s.add(logo(key, cx, cy, size * 0.56, WINE));
  },
  rule(s, x, y, w) {
    const cx = x + w / 2;
    s.add(line(x + w * 0.2, y, cx - 10, y, { stroke: WINE, strokeWidth: 0.8 }), line(cx + 10, y, x + w * 0.8, y, { stroke: WINE, strokeWidth: 0.8 }), rect(cx - 3.5, y - 3.5, 7, 7, { fill: MUSTARD, transform: `rotate(45 ${r(cx)} ${r(y)})` }));
  },
  color(key, rank) { return key === 'unattributed' ? '#C9939B' : key === 'human' ? MUSTARD : pastel.series[Math.min(rank, 5)]; },
});

// ───────────────────────── 剪纸片头 ─────────────────────────

const ORANGE = '#E8562A', CHAR = '#151311', PAPER = '#F1E8D8';
/** 剪刀剪出来的不规则四边形 */
function cut(x: number, y: number, w: number, h: number, seed: number, j = 0.04): string {
  const rnd = rng(seed);
  const jx = () => (rnd() - 0.5) * w * j, jy = () => (rnd() - 0.5) * h * j * 2;
  const pts: [number, number][] = [[x + jx(), y + jy()], [x + w * 0.5 + jx(), y + jy() * 0.6], [x + w + jx(), y + jy()], [x + w + jx() * 0.6, y + h * 0.5], [x + w + jx(), y + h + jy()], [x + w * 0.5 + jx(), y + h + jy() * 0.6], [x + jx(), y + h + jy()], [x + jx() * 0.6, y + h * 0.5]];
  return 'M' + pts.map(([a, b2]) => `${r(a)} ${r(b2)}`).join('L') + 'Z';
}
export const cutout: Style = defineStyle({
  id: 'cutout', name: '剪纸', en: 'Cut-Paper Titles', note: '五六十年代电影片头：剪纸色块、粗壮字、黑橙白', noteEn: 'Film titles from the fifties and sixties: cut-paper shapes, sturdy type, black, orange and white', dark: false,
  c: { bg: ORANGE, paper: PAPER, ink: CHAR, soft: '#4E3A2E', faint: '#9B6A4E', line: 'rgba(21,19,17,0.3)', accent: CHAR, accent2: PAPER },
  series: [CHAR, ORANGE, '#2D5DA8', '#F2C230', '#7A2E1C', '#8C8C8C'],
  f: { display: { family: 'Anton, Noto Sans SC', weight: 400 }, body: { family: 'Oswald, Noto Sans SC', weight: 400 }, num: { family: 'Anton, Noto Sans SC', weight: 400 }, label: { family: 'Oswald, Noto Sans SC', weight: 600 }, mono: { family: 'Oswald, Noto Sans SC' } },
  labelTrack: 0.2, prefer: { tier: 'rows', roll: 'billing' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#E8562A"/><path d="M4 7l9-1 1 4-10 1z" fill="#151311"/><path d="M9 13l11-1-1 6-9 1z" fill="#151311"/><path d="M5 15l4 .4-.6 4-3.6-.6z" fill="#F1E8D8"/>`,
  ground(s, b, m) {
    s.bg = ORANGE;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: ORANGE }));
    const seed = hash(m.project);
    const rnd = rng(seed);
    // 几块斜着的黑纸条
    for (let i = 0; i < 4; i++) {
      const w = b.w * (0.25 + rnd() * 0.4), h = 14 + rnd() * 50, x = b.x + rnd() * (b.w - w), y = b.y + b.h * (0.05 + rnd() * 0.9);
      s.add(path(cut(x, y, w, h, seed + i, 0.06), { fill: i === 2 ? PAPER : CHAR, opacity: i === 2 ? 0.9 : 1, transform: `rotate(${r((rnd() - 0.5) * 16)} ${r(x + w / 2)} ${r(y + h / 2)})` }));
    }
    grain(s, b, { opacity: 0.12, seed: 101, color: '#3A1A08' });
  },
  strip(s, b, m, aside) {
    // 窄条：黑纸条贴着上边、米白纸条贴着下边，右端一张米白小纸片托住统计；中间的文字带只留橙色
    s.bg = ORANGE;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: ORANGE }));
    const seed = hash(m.project), rnd = rng(seed);
    const w1 = b.w * (0.4 + rnd() * 0.2);
    s.add(path(cut(b.x + b.w - w1 - 40, b.y - 8, w1, 22, seed, 0.03), { fill: CHAR, transform: `rotate(-1.2 ${r(b.x + b.w - w1 / 2)} ${r(b.y)})` }));
    const w2 = b.w * (0.3 + rnd() * 0.15);
    s.add(path(cut(b.x + 60, b.y + b.h - 12, w2, 18, seed + 1, 0.04), { fill: PAPER, opacity: 0.9, transform: `rotate(0.8 ${r(b.x + 60 + w2 / 2)} ${r(b.y + b.h)})` }));
    const tw = aside.w + 44, tx = aside.x - 22, ty = aside.y - 12;
    s.add(path(cut(tx, ty, tw, aside.h + 24, seed + 2, 0.03), { fill: PAPER, transform: `rotate(-1.5 ${r(tx + tw / 2)} ${r(ty + aside.h / 2 + 12)})` }));
    grain(s, b, { opacity: 0.12, seed: 101, color: '#3A1A08' });
  },
  plate(s, b) {
    s.add(path(cut(b.x + 30, b.y + 30, b.w - 60, b.h - 60, 7, 0.012), { fill: PAPER }));
    return { x: b.x + 30, y: b.y + 30, w: b.w - 60, h: b.h - 60 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: cutout.f.display, size: size * 1.15, fill: o.fill ?? CHAR, anchor, maxW: o.maxW, upper: true, track: 0.01, extra: { filter: roughen(s, 'co-t', 1.4, 0.05, 3) } });
  },
  bar(s, x, y, w, h, color, i) { if (w > 0) s.add(path(cut(x, y - 3, w, Math.max(h, 10) + 6, i * 13 + 5, 0.02), { fill: color === PAPER ? '#D9CBB2' : color })); },
  track(s, x, y, w, h) { s.add(rect(x, y + Math.max(h, 10) / 2, w, 1.4, { fill: CHAR, opacity: 0.18 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(path(cut(cx - size * 0.35, cy - size * 0.35, size * 0.7, size * 0.7, 3, 0.1), { fill: 'none', stroke: CHAR, strokeWidth: 1.4, strokeDasharray: '3 3' })); return; }
    s.add(path(cut(cx - size * 0.5, cy - size * 0.5, size, size, hash(key), 0.12), { fill: CHAR }));
    s.add(logo(key, cx, cy, size * 0.56, PAPER));
  },
  rule(s, x, y, w) { s.add(path(cut(x, y - 2, w, 4, 11, 0.01), { fill: CHAR })); },
  color(key, rank) { return key === 'unattributed' ? '#9B6A4E' : key === 'human' ? '#2D5DA8' : cutout.series[Math.min(rank, 5)]; },
  on(color) { return color === PAPER || color === '#F2C230' ? CHAR : PAPER; },
});

void mix;
