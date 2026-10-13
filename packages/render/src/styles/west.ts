// 西方古典三套：黄金时代电影片头（Art Deco）、巴洛克（金箔、涡卷、丝绒）、魔典（羊皮卷、黑体字、蜡封）。
import { defineStyle, grain, linear, radial, blur, roughen, logo, type Style, type Box } from '../style.ts';
import { Scene, el, g, rect, circle, line, path, r, rng, hash, mix } from '../svg.ts';

// ───────────────────────── 电影（黄金时代片头） ─────────────────────────

const GOLD = '#CDA85C', IVORY = '#EFE6D2', NIGHT = '#08080B';
export const cinema: Style = defineStyle({
  id: 'cinema', name: '黄金时代', en: 'Golden Age', note: '老电影片头：黑底金线、Art Deco 放射光、胶片颗粒', noteEn: 'Old movie titles: gold rules on black, Art Deco sunbursts, film grain', dark: true,
  c: { bg: NIGHT, paper: '#0E0D11', ink: IVORY, soft: '#A39880', faint: '#5E5748', line: 'rgba(205,168,92,0.28)', accent: GOLD, accent2: GOLD },
  series: [GOLD, IVORY, '#A98B4F', '#D8CBB0', '#7C6A44', '#B9AE98'],
  f: {
    display: { family: 'Cinzel, Noto Serif SC', weight: 700 }, body: { family: 'Cormorant Garamond, Noto Serif SC', weight: 500 },
    num: { family: 'Cinzel, Noto Serif SC', weight: 500 }, label: { family: 'Cinzel, Noto Serif SC', weight: 500 }, mono: { family: 'Cormorant Garamond, Noto Serif SC' },
    script: { family: 'Pinyon Script, Noto Serif SC' },
  },
  labelTrack: 0.36, prefer: { tier: 'rows', roll: 'classic' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#08080B" stroke="#CDA85C" stroke-width="1"/>${[...Array(9)].map((_, i) => { const a = Math.PI * (0.1 + 0.8 * i / 8); return `<line x1="12" y1="19" x2="${(12 - Math.cos(a) * 10).toFixed(1)}" y2="${(19 - Math.sin(a) * 10).toFixed(1)}" stroke="#CDA85C" stroke-width=".6" opacity=".8"/>`; }).join('')}<rect x="7" y="17.5" width="10" height="1.4" fill="#CDA85C"/>`,
  ground(s, b) {
    s.bg = NIGHT;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: NIGHT }));
    // 放射光：从底部中心散开的细金线
    const cx = b.x + b.w / 2, cy = b.y + b.h + b.h * 0.05, R = Math.hypot(b.w, b.h);
    const rays: string[] = [];
    for (let i = 0; i <= 36; i++) { const a = Math.PI * (i / 36); rays.push(line(cx, cy, cx - Math.cos(a) * R, cy - Math.sin(a) * R, { stroke: GOLD, strokeWidth: i % 2 ? 0.6 : 1.2, opacity: i % 2 ? 0.07 : 0.12 })); }
    s.add(g({}, ...rays));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'cn-spot', [[0, '#2A2418', 0.55], [0.6, NIGHT, 0], [1, '#000000', 0.7]], { cx: 0.5, cy: 0.35, r: 0.75 }) }));
    // Art Deco 框：双线 + 阶梯角
    const p = 20, k = 18;
    const deco = (x: number, y: number, w: number, h: number, c: number) => `M${r(x + c)} ${r(y)}H${r(x + w - c)}V${r(y + c * 0.5)}H${r(x + w - c * 0.5)}V${r(y + c)}H${r(x + w)}V${r(y + h - c)}H${r(x + w - c * 0.5)}V${r(y + h - c * 0.5)}H${r(x + w - c)}V${r(y + h)}H${r(x + c)}V${r(y + h - c * 0.5)}H${r(x + c * 0.5)}V${r(y + h - c)}H${r(x)}V${r(y + c)}H${r(x + c * 0.5)}V${r(y + c * 0.5)}H${r(x + c)}Z`;
    s.add(path(deco(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, k), { fill: 'none', stroke: GOLD, strokeWidth: 1.4, opacity: 0.85 }));
    s.add(path(deco(b.x + p + 7, b.y + p + 7, b.w - p * 2 - 14, b.h - p * 2 - 14, k - 4), { fill: 'none', stroke: GOLD, strokeWidth: 0.6, opacity: 0.55 }));
    grain(s, b, { opacity: 0.16, seed: 51, freq: 1.4, color: '#FFFFFF' });
  },
  plate(s, b) { return { x: b.x + 10, y: b.y + 10, w: b.w - 20, h: b.h - 20 }; },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: cinema.f.display, size, fill: o.fill ?? IVORY, anchor, maxW: o.maxW, track: 0.06, upper: true });
  },
  bar(s, x, y, w, h, color) {
    if (w <= 0) return;
    const yy = y + h / 2;
    s.add(line(x, yy, x + w, yy, { stroke: color, strokeWidth: Math.max(1.5, h * 0.35) }));
    s.add(path(`M${r(x + w)} ${r(yy - h * 0.7)}L${r(x + w + h * 0.7)} ${r(yy)}L${r(x + w)} ${r(yy + h * 0.7)}L${r(x + w - h * 0.7)} ${r(yy)}Z`, { fill: color }));
  },
  track(s, x, y, w, h) { s.add(line(x, y + h / 2, x + w, y + h / 2, { stroke: GOLD, strokeOpacity: 0.16, strokeWidth: 1 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.36, { fill: 'none', stroke: GOLD, strokeOpacity: 0.4, strokeWidth: 1, strokeDasharray: '2 3' })); return; }
    s.add(circle(cx, cy, size * 0.5, { fill: 'none', stroke: GOLD, strokeWidth: 0.9, opacity: 0.8 }));
    s.add(logo(key, cx, cy, size * 0.54, IVORY));
  },
  rule(s, x, y, w) {
    const cx = x + w / 2;
    s.add(line(x, y, cx - 14, y, { stroke: GOLD, strokeWidth: 0.8, opacity: 0.7 }), line(cx + 14, y, x + w, y, { stroke: GOLD, strokeWidth: 0.8, opacity: 0.7 }));
    s.add(path(`M${r(cx)} ${r(y - 5)}L${r(cx + 6)} ${r(y)}L${r(cx)} ${r(y + 5)}L${r(cx - 6)} ${r(y)}Z`, { fill: GOLD }));
  },
  hatch(s) { return s.url('cn-h', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 6, 6, { fill: NIGHT }), rect(0, 0, 1.2, 6, { fill: GOLD, opacity: 0.5 }))); },
  on() { return NIGHT; },
});

// ───────────────────────── 巴洛克 ─────────────────────────

const VELVET = '#2B0B12', OX = '#5A1420', G1 = '#F3D98B', G2 = '#B8862E', G3 = '#7A5418';
/** 金箔渐变 */
export const goldFill = (s: Scene, name = 'bq-gold') => linear(s, name, [[0, G1], [0.35, '#D9AE52'], [0.55, G2], [0.75, '#E9C979'], [1, G3]], [0, 0, 0.3, 1]);

/** 涡卷角饰：100×100 的单位里画，按角翻转 */
function scroll(s: Scene, x: number, y: number, size: number, fx: number, fy: number) {
  const k = size / 100;
  const tr = `translate(${r(x)} ${r(y)}) scale(${r(k * fx)} ${r(k * fy)})`;
  const gold = goldFill(s);
  const d = [
    'M4 96C4 52 26 22 68 12C86 8 98 16 96 30C94 42 80 46 74 37C69 30 75 22 83 25',
    'M4 96C18 72 38 62 58 64C72 66 77 78 68 85C61 90 53 85 56 78',
    'M30 40C36 34 46 30 56 31', 'M20 58C26 50 36 46 46 46',
  ];
  s.add(g({ transform: tr, fill: 'none', stroke: gold, strokeLinecap: 'round' },
    path(d[0], { strokeWidth: 5 }), path(d[1], { strokeWidth: 4 }), path(d[2], { strokeWidth: 2.4 }), path(d[3], { strokeWidth: 2.4 }),
    el('ellipse', { cx: 44, cy: 24, rx: 9, ry: 4, transform: 'rotate(-30 44 24)', fill: gold, stroke: 'none' }),
    el('ellipse', { cx: 30, cy: 72, rx: 8, ry: 3.6, transform: 'rotate(-58 30 72)', fill: gold, stroke: 'none' }),
    circle(4, 96, 6, { fill: gold, stroke: 'none' })));
}

export const baroque: Style = defineStyle({
  id: 'baroque', name: '巴洛克', en: 'Baroque', note: '丝绒、金箔、涡卷与月桂；宫廷里的荣誉榜', noteEn: 'Velvet, gold leaf, scrolls and laurels; a roll of honour at court', dark: true,
  c: { bg: VELVET, paper: '#35101A', ink: '#F6E7C4', soft: '#CDAE7A', faint: '#8A6A48', line: 'rgba(243,217,139,0.25)', accent: G1, accent2: '#E9C979' },
  series: ['#E9C979', '#D8D2C4', '#C98E5A', '#A7B6A0', '#B88AA0', '#8F9BB3'],
  f: {
    display: { family: 'Cormorant Garamond, Noto Serif SC', weight: 700 }, body: { family: 'Cormorant Garamond, Noto Serif SC', weight: 500 },
    num: { family: 'Cormorant Garamond, Noto Serif SC', weight: 700 }, label: { family: 'Cormorant Garamond, Noto Serif SC', weight: 600 }, mono: { family: 'Cormorant Garamond, Noto Serif SC' },
    script: { family: 'Pinyon Script, Noto Serif SC' },
  },
  labelTrack: 0.3, prefer: { tier: 'podium', roll: 'classic' },
  dot: () => `<defs><linearGradient id="bqd" x1="0" y1="0" x2=".3" y2="1"><stop offset="0" stop-color="#F3D98B"/><stop offset=".5" stop-color="#B8862E"/><stop offset="1" stop-color="#E9C979"/></linearGradient></defs><circle cx="12" cy="12" r="11" fill="#2B0B12" stroke="url(#bqd)" stroke-width="2"/><path d="M7 15c0-4 3-7 7-7 2 0 3 1 3 2.4 0 1.2-1.6 1.6-2 .6" fill="none" stroke="url(#bqd)" stroke-width="1.4" stroke-linecap="round"/><circle cx="7" cy="15.4" r="1.3" fill="#F3D98B"/>`,
  ground(s, b) {
    s.bg = VELVET;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'bq-vel', [[0, '#4A1420'], [0.7, VELVET], [1, '#160509']], { cx: 0.5, cy: 0.4, r: 0.8 }) }));
    // 锦缎底纹：菱格 + 四瓣花
    const dm = s.url('bq-damask', (id) => el('pattern', { id, width: 56, height: 56, patternUnits: 'userSpaceOnUse' },
      path('M28 0L56 28L28 56L0 28Z', { fill: 'none', stroke: G1, strokeOpacity: 0.06, strokeWidth: 1 }),
      g({ transform: 'translate(28 28)', fill: G1, opacity: 0.07 }, ...[0, 90, 180, 270].map((a) => el('ellipse', { cx: 0, cy: -7, rx: 3.2, ry: 7, transform: `rotate(${a})` })), circle(0, 0, 2.4)),
      g({ fill: G1, opacity: 0.05 }, circle(0, 0, 2), circle(56, 0, 2), circle(0, 56, 2), circle(56, 56, 2))));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: dm }));
    grain(s, b, { opacity: 0.1, seed: 61, color: '#000000' });
  },
  plate(s, b) {
    const gold = goldFill(s);
    const p = 14;
    s.add(rect(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, { fill: 'none', stroke: gold, strokeWidth: 4 }));
    s.add(rect(b.x + p + 9, b.y + p + 9, b.w - p * 2 - 18, b.h - p * 2 - 18, { fill: 'none', stroke: gold, strokeWidth: 1.2 }));
    // 珠串
    const beads: string[] = [];
    for (let x = b.x + p + 40; x < b.x + b.w - p - 40; x += 14) beads.push(circle(x, b.y + p + 4.5, 1.6), circle(x, b.y + b.h - p - 4.5, 1.6));
    s.add(g({ fill: G1, opacity: 0.8 }, ...beads));
    const sz = Math.min(120, b.w * 0.16);
    scroll(s, b.x + p - 4, b.y + p - 4 + sz, sz, 1, -1);
    scroll(s, b.x + b.w - p + 4, b.y + p - 4 + sz, sz, -1, -1);
    scroll(s, b.x + p - 4, b.y + b.h - p + 4 - sz, sz, 1, 1);
    scroll(s, b.x + b.w - p + 4, b.y + b.h - p + 4 - sz, sz, -1, 1);
    return { x: b.x + 30, y: b.y + 30, w: b.w - 60, h: b.h - 60 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: baroque.f.display, size: size * 1.08, fill: o.fill ?? goldFill(s), anchor, maxW: o.maxW, track: 0.02 });
  },
  num(s, text, x, y, size, anchor = 'end', o = {}) {
    return s.text(text, x, y, { font: baroque.f.num, size: size * 1.05, fill: o.fill ?? '#F6E7C4', anchor });
  },
  bar(s, x, y, w, h, color, i) {
    if (w <= 0) return;
    const hh = Math.max(h, 10);
    const fill = i === 0 ? goldFill(s) : linear(s, `bq-b${i}`, [[0, mix(color, '#FFFFFF', 0.35)], [0.5, color], [1, mix(color, '#000000', 0.35)]]);
    const vert = h > w * 1.5;
    s.add(rect(x, y - (hh - h) / 2, w, hh, { fill, rx: vert ? 4 : Math.min(hh, w) / 2, stroke: G3, strokeWidth: 0.8 }));
    if (!vert) s.add(rect(x + hh / 2, y - (hh - h) / 2 + hh * 0.18, Math.max(0, w - hh), hh * 0.18, { fill: '#FFFFFF', opacity: 0.3, rx: hh * 0.09 }));
  },
  track(s, x, y, w, h) { s.add(rect(x, y + h / 2 - 0.5, w, 1, { fill: G1, opacity: 0.18 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.42, { fill: 'none', stroke: G2, strokeWidth: 1.2, strokeDasharray: '3 3' })); return; }
    const gold = goldFill(s);
    s.add(circle(cx, cy, size * 0.56, { fill: gold }));
    s.add(circle(cx, cy, size * 0.46, { fill: OX, stroke: G3, strokeWidth: 0.8 }));
    s.add(logo(key, cx, cy, size * 0.5, G1));
  },
  rule(s, x, y, w) {
    const gold = goldFill(s);
    const cx = x + w / 2;
    s.add(path(`M${r(x)} ${r(y)}Q${r(cx - 40)} ${r(y - 6)} ${r(cx - 16)} ${r(y)}`, { fill: 'none', stroke: gold, strokeWidth: 1.4 }), path(`M${r(x + w)} ${r(y)}Q${r(cx + 40)} ${r(y - 6)} ${r(cx + 16)} ${r(y)}`, { fill: 'none', stroke: gold, strokeWidth: 1.4 }));
    s.add(g({ transform: `translate(${r(cx)} ${r(y)})`, fill: gold }, ...[0, 90, 180, 270].map((a) => el('ellipse', { cx: 0, cy: -5, rx: 2.4, ry: 5, transform: `rotate(${a})` }))));
  },
  hatch(s) { return s.url('bq-h', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 6, 6, { fill: VELVET }), rect(0, 0, 1.4, 6, { fill: G2, opacity: 0.7 }))); },
  color(key, rank) {
    if (key === 'unattributed') return '#8A6A48';
    if (key === 'human') return '#E9C979';
    return baroque.series[Math.min(rank, 5)];
  },
  on() { return VELVET; },
});

/** 月桂环：给第一名 */
export function laurel(s: Scene, cx: number, cy: number, R: number, fill: string) {
  const leaves: string[] = [];
  for (const side of [-1, 1]) {
    for (let i = 0; i < 9; i++) {
      const a = Math.PI * (0.62 + i * 0.075);
      const x = cx + side * Math.cos(a) * -R, y = cy + Math.sin(a) * R;
      const rot = (side < 0 ? 180 - (a * 180) / Math.PI : (a * 180) / Math.PI) + side * 35;
      leaves.push(el('ellipse', { cx: x, cy: y, rx: R * 0.16, ry: R * 0.065, transform: `rotate(${r(rot)} ${r(x)} ${r(y)})`, fill }));
    }
  }
  s.add(g({}, ...leaves));
}

// ───────────────────────── 魔典（羊皮卷） ─────────────────────────

const PARCH = '#D8C59C', BROWN = '#2E1F12', BLOOD = '#7A1418', SILVER = '#9EA3AA';
export const grimoire: Style = defineStyle({
  id: 'grimoire', name: '魔典', en: 'Grimoire', note: '暗黑奇幻的怪物图鉴：焦边羊皮卷、黑体字、蜡封与符文', noteEn: 'A dark-fantasy bestiary: burnt-edge parchment, blackletter, wax seals and runes', dark: false,
  c: { bg: PARCH, paper: '#E0CFA8', ink: BROWN, soft: '#5E4630', faint: '#9C8462', line: 'rgba(46,31,18,0.3)', accent: BLOOD, accent2: BLOOD },
  series: [BROWN, BLOOD, '#4C5A4A', '#6B4A2E', '#3E4A5C', '#8A6A3A'],
  f: {
    display: { family: 'Pirata One, UnifrakturMaguntia, Noto Serif SC', weight: 400 }, body: { family: 'IM Fell English, Noto Serif SC', weight: 400 },
    num: { family: 'IM Fell English, Noto Serif SC', weight: 400 }, label: { family: 'IM Fell English, Noto Serif SC', weight: 400 }, mono: { family: 'IM Fell English, Noto Serif SC' },
    script: { family: 'UnifrakturMaguntia, Noto Serif SC' },
  },
  labelTrack: 0.22, prefer: { tier: 'rows', roll: 'classic' },
  dot: () => `<defs><radialGradient id="gmd" cx=".5" cy=".5" r=".6"><stop offset=".55" stop-color="#D8C59C"/><stop offset="1" stop-color="#5A3A1E"/></radialGradient></defs><circle cx="12" cy="12" r="11" fill="url(#gmd)"/><circle cx="15" cy="15" r="4.6" fill="#7A1418"/><circle cx="15" cy="15" r="3" fill="none" stroke="#4E0C0F" stroke-width=".8"/><path d="M6 7h7M6 9.5h5" stroke="#2E1F12" stroke-width="1"/>`,
  ground(s, b, m) {
    s.bg = '#1A120B';
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#1A120B' }));
    // 羊皮纸：边缘焦黑、不规则
    const f = s.def('gm-burn', (id) => el('filter', { id, x: '-5%', y: '-5%', width: '110%', height: '110%' },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: 0.025, numOctaves: 4, seed: hash(m.project) % 50, result: 'n' }),
      el('feDisplacementMap', { in: 'SourceGraphic', in2: 'n', scale: 26, xChannelSelector: 'R', yChannelSelector: 'G' })));
    s.add(rect(b.x + 14, b.y + 14, b.w - 28, b.h - 28, { fill: PARCH, filter: `url(#${f})` }));
    s.add(rect(b.x + 14, b.y + 14, b.w - 28, b.h - 28, { fill: radial(s, 'gm-age', [[0.45, '#000000', 0], [0.85, '#6B4320', 0.35], [1, '#2A1708', 0.85]], { cx: 0.5, cy: 0.5, r: 0.72 }), filter: `url(#${f})` }));
    // 污渍
    const stain = s.def('gm-stain', (id) => el('filter', { id, x: 0, y: 0, width: '100%', height: '100%' },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: 0.008, numOctaves: 3, seed: 13 }),
      el('feColorMatrix', { type: 'matrix', values: '0 0 0 0 0.35 0 0 0 0 0.22 0 0 0 0 0.1 0 0 0 2.2 -1.1' })));
    s.add(rect(b.x + 20, b.y + 20, b.w - 40, b.h - 40, { filter: `url(#${stain})`, opacity: 0.4 }));
    grain(s, b, { opacity: 0.14, seed: 71, color: '#2E1F12', freq: 1 });
  },
  plate(s, b) {
    const f = roughen(s, 'gm-ink', 1.2, 0.3, 5);
    const p = 34;
    s.add(g({ filter: f, fill: 'none', stroke: BROWN }, rect(b.x + p, b.y + p, b.w - p * 2, b.h - p * 2, { strokeWidth: 2 }), rect(b.x + p + 6, b.y + p + 6, b.w - p * 2 - 12, b.h - p * 2 - 12, { strokeWidth: 0.8 })));
    // 角上的符文结
    for (const [x, y] of [[b.x + p, b.y + p], [b.x + b.w - p, b.y + p], [b.x + p, b.y + b.h - p], [b.x + b.w - p, b.y + b.h - p]]) {
      s.add(g({ filter: f }, path(`M${r(x - 9)} ${r(y)}L${r(x)} ${r(y - 9)}L${r(x + 9)} ${r(y)}L${r(x)} ${r(y + 9)}Z`, { fill: PARCH, stroke: BROWN, strokeWidth: 1.4 }), circle(x, y, 2.4, { fill: BLOOD })));
    }
    return { x: b.x + p + 8, y: b.y + p + 8, w: b.w - (p + 8) * 2, h: b.h - (p + 8) * 2 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: grimoire.f.display, size: size * 1.1, fill: o.fill ?? BROWN, anchor, maxW: o.maxW, extra: { filter: roughen(s, 'gm-t', 1, 0.4, 2) } });
  },
  bar(s, x, y, w, h, color, i) {
    if (w <= 0) return;
    const hh = Math.max(h, 10);
    const hx = s.url(`gm-x${i}`, (id) => el('pattern', { id, width: 4, height: 4, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(-35)' }, rect(0, 0, 4, 4, { fill: mix(color, PARCH, 0.55) }), rect(0, 0, 1.4, 4, { fill: color })));
    s.add(rect(x, y - (hh - h) / 2, w, hh, { fill: hx, stroke: BROWN, strokeWidth: 1.2, filter: roughen(s, 'gm-b', 1.4, 0.25, 3) }));
  },
  track(s, x, y, w, h) { s.add(line(x, y + h / 2, x + w, y + h / 2, { stroke: BROWN, strokeOpacity: 0.25, strokeWidth: 1, strokeDasharray: '2 4' })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.4, { fill: 'none', stroke: BROWN, strokeOpacity: 0.45, strokeWidth: 1.2, strokeDasharray: '2 3' })); return; }
    // 蜡封
    const f = roughen(s, 'gm-wax', size * 0.06, 0.09, 8);
    s.add(g({ filter: f }, circle(cx, cy, size * 0.56, { fill: BLOOD })));
    s.add(circle(cx, cy, size * 0.4, { fill: 'none', stroke: '#4E0C0F', strokeWidth: 1.4 }));
    s.add(circle(cx - size * 0.14, cy - size * 0.2, size * 0.12, { fill: '#FFFFFF', opacity: 0.18 }));
    s.add(logo(key, cx, cy, size * 0.48, '#3A0A0C'));
  },
  rule(s, x, y, w) {
    const f = roughen(s, 'gm-r', 1, 0.3, 4);
    const cx = x + w / 2;
    s.add(g({ filter: f }, line(x, y, cx - 26, y, { stroke: BROWN, strokeWidth: 1.2 }), line(cx + 26, y, x + w, y, { stroke: BROWN, strokeWidth: 1.2 }),
      path(`M${r(cx - 16)} ${r(y)}L${r(cx)} ${r(y - 7)}L${r(cx + 16)} ${r(y)}L${r(cx)} ${r(y + 7)}Z`, { fill: 'none', stroke: BROWN, strokeWidth: 1.2 }), circle(cx, y, 2.4, { fill: BLOOD }),
      circle(cx - 22, y, 1.6, { fill: BROWN }), circle(cx + 22, y, 1.6, { fill: BROWN })));
  },
  hatch(s) { return s.url('gm-h', (id) => el('pattern', { id, width: 5, height: 5, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 5, 5, { fill: '#CDB88C' }), rect(0, 0, 1, 5, { fill: BROWN, opacity: 0.5 }))); },
  color(key, rank) {
    if (key === 'unattributed') return '#9C8462';
    if (key === 'human') return BLOOD;
    return grimoire.series[Math.min(rank, 5)];
  },
});

export type { Box };
void blur; void rng; void SILVER;
