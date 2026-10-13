// 东方三套：水墨（国画）、浮世绘、古籍（章回体刻本）。
import { defineStyle, grain, linear, radial, blur, roughen, logo, type Style, type Box } from '../style.ts';
import { Scene, el, g, rect, circle, line, path, r, rng, hash, mix, alpha } from '../svg.ts';
import type { Model } from '../data.ts';

// ───────────────────────── 共用笔触 ─────────────────────────

/** 山脊线：几段正弦叠加 + 尖峰，种子决定形状 */
export function ridge(x0: number, w: number, base: number, amp: number, seed: number, step = 6): string {
  const rnd = rng(seed);
  const ph = [rnd() * 6, rnd() * 6, rnd() * 6];
  const peaks = [rnd(), rnd(), rnd()].map((p, i) => ({ at: x0 + w * (0.15 + 0.3 * i + p * 0.2), h: 0.5 + rnd() * 0.5, sp: w * (0.08 + rnd() * 0.12) }));
  let d = `M${r(x0)} ${r(base + amp * 2)}`;
  for (let x = x0; x <= x0 + w + step; x += step) {
    const t = (x - x0) / w;
    let y = Math.sin(t * 5 + ph[0]) * 0.25 + Math.sin(t * 13 + ph[1]) * 0.1 + Math.sin(t * 31 + ph[2]) * 0.04;
    for (const p of peaks) y += p.h * Math.exp(-(((x - p.at) / p.sp) ** 2));
    d += `L${r(x)} ${r(base - y * amp)}`;
  }
  return d + `L${r(x0 + w + step)} ${r(base + amp * 2)}Z`;
}

/** 毛笔横画：起笔饱满、收笔飞白。用于条形 */
export function brushBar(s: Scene, x: number, y: number, w: number, h: number, color: string, seed: number) {
  if (w <= 0) return;
  const rnd = rng(seed + 11);
  const f = roughen(s, 'brush', Math.max(1.5, Math.min(4, h * 0.18)), 0.06, 3);
  const head = Math.min(h * 0.6, w / 2);
  const grad = linear(s, `bb-${color.slice(1)}`, [[0, color, 1], [0.75, color, 0.92], [1, color, 0.55]], [0, 0, 1, 0]);
  s.add(path(`M${r(x + head)} ${r(y)}Q${r(x - head * 0.3)} ${r(y + h * 0.1)} ${r(x)} ${r(y + h * 0.55)}Q${r(x + head * 0.2)} ${r(y + h * 1.05)} ${r(x + head)} ${r(y + h)}L${r(x + w - h * 0.4)} ${r(y + h * 0.92)}Q${r(x + w + h * 0.3)} ${r(y + h * 0.5)} ${r(x + w - h * 0.3)} ${r(y + h * 0.06)}Z`, { fill: grad, filter: f }));
  // 飞白：收笔处几道细丝
  const n = 3 + Math.floor(rnd() * 3);
  for (let i = 0; i < n; i++) {
    const yy = y + h * (0.15 + 0.7 * rnd());
    const len = Math.min(w * 0.25, h * (1.5 + rnd() * 3));
    s.add(line(x + w - len, yy, x + w + h * (0.2 + rnd() * 0.6), yy + (rnd() - 0.5) * 2, { stroke: '#F3ECDD', strokeWidth: 0.6 + rnd() * 1.2, opacity: 0.75, strokeLinecap: 'round' }));
  }
}

/** 印章：朱红方印，白文（字是空的）或朱文 */
export function seal(s: Scene, cx: number, cy: number, size: number, chars: string, o: { color?: string; white?: boolean; font?: string; seed?: number } = {}) {
  const color = o.color ?? '#B23A2E';
  const f = roughen(s, 'seal', size * 0.035 + 1, 0.12, o.seed ?? 4);
  const x = cx - size / 2, y = cy - size / 2;
  const font = o.font ?? 'ZCOOL XiaoWei, Noto Serif SC';
  const cs = [...chars];
  const cells: [string, number, number][] = cs.length >= 4 ? [[cs[0], 1, 0], [cs[1], 1, 1], [cs[2], 0, 0], [cs[3], 0, 1]]
    : cs.length === 2 ? [[cs[0], 0.5, 0], [cs[1], 0.5, 1]] : [[cs[0] ?? '印', 0.5, 0.5]];
  const fs = cs.length >= 2 ? size * 0.4 : size * 0.66;
  const txt = cells.map(([ch, col, row]) => el('text', {
    x: x + size * (cs.length >= 4 ? 0.28 + col * 0.44 : 0.5), y: y + size * (cs.length >= 2 ? 0.44 + row * 0.42 : 0.72),
    fontFamily: font, fontSize: cs.length === 2 ? size * 0.42 : fs, textAnchor: 'middle', fill: o.white === false ? color : '#F6EEE0',
  }, ch));
  if (o.white === false) {
    s.add(g({ filter: f }, rect(x + size * 0.05, y + size * 0.05, size * 0.9, size * 0.9, { fill: 'none', stroke: color, strokeWidth: size * 0.07, rx: size * 0.04 }), ...txt));
  } else {
    s.add(g({ filter: f }, rect(x, y, size, size, { fill: color, rx: size * 0.06 }), ...txt));
  }
}

// ───────────────────────── 水墨 ─────────────────────────

const INK = '#1F1C18';
export const ink: Style = defineStyle({
  id: 'ink', name: '水墨', en: 'Ink Wash', note: '宣纸、远山、朱印；国画的留白与浓淡', noteEn: 'Rice paper, distant mountains and a red seal; the empty space and washes of ink painting', dark: false,
  c: { bg: '#EFE7D6', paper: '#F3ECDD', ink: INK, soft: '#5F584D', faint: '#A39A89', line: 'rgba(31,28,24,0.18)', accent: '#B23A2E', accent2: '#7A2E25' },
  series: [INK, '#3A3631', '#57524A', '#7A7469', '#9E978A', '#BDB5A6'],
  f: {
    display: { family: 'Ma Shan Zheng, Noto Serif SC', weight: 400 }, body: { family: 'Noto Serif SC', weight: 400 },
    num: { family: 'Cormorant Garamond, Noto Serif SC', weight: 600 }, label: { family: 'Noto Serif SC', weight: 500 }, mono: { family: 'Noto Serif SC' },
    script: { family: 'Zhi Mang Xing, Ma Shan Zheng, Noto Serif SC' },
  },
  labelUpper: false, labelTrack: 0.3,
  prefer: { tier: 'rows', roll: 'vertical' }, reserve: { top: 0, bottom: 110 },
  dot: () => `<defs><radialGradient id="ik" cx=".42" cy=".4" r=".7"><stop offset="0" stop-color="#1F1C18"/><stop offset=".55" stop-color="#4A453E"/><stop offset="1" stop-color="#4A453E" stop-opacity="0"/></radialGradient></defs><circle cx="12" cy="12" r="11" fill="#F1EADB"/><circle cx="11" cy="11.5" r="9" fill="url(#ik)"/><rect x="15" y="14.5" width="5" height="5" rx=".6" fill="#B23A2E"/>`,
  ground(s, b, m) {
    s.bg = ink.c.bg;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: ink.c.bg }));
    // 纸纤维：横向拉长的噪点 + 四周略暗
    const fib = s.def('fiber', (id) => el('filter', { id, x: 0, y: 0, width: '100%', height: '100%' },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: '0.006 0.09', numOctaves: 4, seed: 5 }),
      el('feColorMatrix', { type: 'matrix', values: '0 0 0 0 0.35 0 0 0 0 0.29 0 0 0 0 0.2 0 0 0 -1.6 1.05' })));
    s.add(rect(b.x, b.y, b.w, b.h, { filter: `url(#${fib})`, opacity: 0.16 }));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'ink-vig', [[0.55, '#000000', 0], [1, '#5A4628', 0.18]], { cx: 0.5, cy: 0.5, r: 0.75 }) }));
    // 远山三层：越远越淡越虚，山脚化进雾里
    const seed = hash(m.project || 'ink');
    const H = b.h, base = b.y + H;
    const layers: [number, number, number, number][] = [[0.09, 7, 0.2, 0.12], [0.16, 3.5, 0.13, 0.06], [0.3, 1.4, 0.07, 0.015]];
    layers.forEach(([op, bl, amp, off], i) => {
      const fade = linear(s, `mist${i}`, [[0, '#2E3B42', op], [0.5, '#2E3B42', op * 0.45], [1, '#2E3B42', 0]]);
      const top = base - H * off - H * amp;
      s.add(g({ filter: blur(s, bl) }, path(ridge(b.x - 20, b.w + 40, base - H * off, H * amp, seed + i * 97, 5), { fill: fade, transform: `translate(0 ${r(top * 0)})` })));
    });
    s.add(rect(b.x, base - H * 0.12, b.w, H * 0.12, { fill: linear(s, 'mistb', [[0, ink.c.bg, 0], [1, ink.c.bg, 0.9]]) }));
    grain(s, b, { opacity: 0.08, seed: 8, freq: 1.2, color: '#3B2F1E' });
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: ink.f.display, size: size * 1.12, fill: o.fill ?? INK, anchor, maxW: o.maxW, extra: { filter: roughen(s, 'title', 1.6, 0.2, 3) } });
  },
  bar(s, x, y, w, h, color, i) {
    if (h > w * 1.5) {
      // 竖笔：上端起笔浓、下端略淡
      s.add(rect(x, y, w, h, { fill: linear(s, `ink-v-${color.slice(1)}`, [[0, color, 1], [1, color, 0.82]]), rx: w * 0.3, filter: roughen(s, 'brushv', Math.min(3, w * 0.2), 0.08, 5) }));
      return;
    }
    const hh = Math.max(h * 1.5, 12); brushBar(s, x, y - (hh - h) / 2, w, hh, color, i * 7 + 3);
  },
  track(s, x, y, w, h) { s.add(line(x, y + h / 2, x + w, y + h / 2, { stroke: INK, strokeOpacity: 0.12, strokeWidth: 1, strokeDasharray: '1 5', strokeLinecap: 'round' })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.34, { fill: 'none', stroke: INK, strokeOpacity: 0.35, strokeWidth: 1.6, filter: roughen(s, 'ring', 1.5, 0.3, 6) })); return; }
    const f = roughen(s, 'mk', size * 0.04 + 0.8, 0.14, 2);
    s.add(g({ filter: f }, rect(cx - size * 0.5, cy - size * 0.5, size, size, { fill: '#B23A2E', rx: size * 0.08 })));
    s.add(logo(key, cx, cy, size * 0.62, '#F6EEE0'));
  },
  rule(s, x, y, w, kind) {
    if (kind === 'orn') { brushBar(s, x, y - 2, w, 4, INK, 5); return; }
    s.add(line(x, y, x + w, y, { stroke: INK, strokeOpacity: 0.28, strokeWidth: 1, filter: roughen(s, 'rl', 1.2, 0.2, 1) }));
  },
  hatch(s) {
    return s.url('ink-hatch', (id) => el('pattern', { id, width: 7, height: 7, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(-30)' }, rect(0, 0, 7, 7, { fill: '#E6DCC6' }), line(0, 3.5, 7, 3.5, { stroke: INK, strokeOpacity: 0.35, strokeWidth: 1 })));
  },
  color(key, rank) {
    if (key === 'unattributed') return '#A39A89';
    if (key === 'human') return '#B23A2E';
    if (key === 'other') return '#BDB5A6';
    return ink.series[Math.min(rank, 5)];
  },
});

// ───────────────────────── 浮世绘 ─────────────────────────

const AI = '#1E3A5F';      // 普鲁士蓝（ベロ藍）
const AI2 = '#5E86A8';
const SHU = '#C0432D';     // 朱
const KI = '#D8A23E';      // 黄土
const SUMI = '#2A2520';
const WASHI = '#EEE1C4';
export const ukiyoe: Style = defineStyle({
  id: 'ukiyoe', name: '浮世绘', en: 'Ukiyo-e', note: '江户版画：青海波、远富士、题签与朱色版元印', noteEn: 'Edo woodblock prints: wave patterns, a far-off Fuji, a title slip and a red publisher\'s seal', dark: false,
  c: { bg: WASHI, paper: '#F3E8CF', ink: SUMI, soft: '#5D5446', faint: '#A49679', line: 'rgba(42,37,32,0.25)', accent: SHU, accent2: AI },
  series: [AI, SHU, KI, '#4E6B4A', AI2, '#8C5A3C'],
  f: {
    display: { family: 'Shippori Mincho B1, Noto Serif SC', weight: 800 }, body: { family: 'Shippori Mincho B1, Noto Serif SC', weight: 500 },
    num: { family: 'Shippori Mincho B1, Noto Serif SC', weight: 800 }, label: { family: 'Shippori Mincho B1, Noto Serif SC', weight: 500 }, mono: { family: 'Noto Serif SC' },
  },
  labelUpper: false, labelTrack: 0.24, radius: 0,
  prefer: { tier: 'rows', roll: 'vertical' }, reserve: { top: 120, bottom: 120 },
  dot: () => `<defs><clipPath id="uk"><circle cx="12" cy="12" r="11"/></clipPath></defs><g clip-path="url(#uk)"><rect width="24" height="24" fill="#EEE1C4"/><rect width="24" height="9" fill="#1E3A5F"/><circle cx="12" cy="26" r="12" fill="#1E3A5F"/><circle cx="12" cy="26" r="9" fill="none" stroke="#EEE1C4" stroke-width="1.3"/><circle cx="12" cy="26" r="6" fill="none" stroke="#EEE1C4" stroke-width="1.3"/><circle cx="17" cy="11" r="2.6" fill="#C0432D"/></g><circle cx="12" cy="12" r="11" fill="none" stroke="#2A2520" stroke-width=".8"/>`,
  ground(s, b, m) {
    s.bg = WASHI;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: WASHI }));
    // 天空ぼかし：顶部一条普鲁士蓝渐隐；远富士与朱日立在天边
    const sky = Math.min(b.h * 0.24, 34 + ukiyoe.reserve.top);
    s.add(rect(b.x, b.y, b.w, sky, { fill: linear(s, 'uk-sky', [[0, AI, 0.95], [0.55, AI, 0.4], [1, AI, 0]]) }));
    const fw = Math.min(b.w * 0.3, 300), fx = b.x + b.w * 0.8, fy = b.y + sky - 6;
    s.add(circle(b.x + b.w * 0.62, b.y + sky * 0.42, Math.min(sky * 0.22, 30), { fill: SHU, opacity: 0.92 }));
    s.add(path(`M${r(fx - fw / 2)} ${r(fy)}L${r(fx - fw * 0.09)} ${r(fy - fw * 0.26)}Q${r(fx)} ${r(fy - fw * 0.3)} ${r(fx + fw * 0.09)} ${r(fy - fw * 0.26)}L${r(fx + fw / 2)} ${r(fy)}Z`, { fill: AI, opacity: 0.75 }));
    s.add(path(`M${r(fx - fw * 0.09)} ${r(fy - fw * 0.26)}Q${r(fx)} ${r(fy - fw * 0.3)} ${r(fx + fw * 0.09)} ${r(fy - fw * 0.26)}L${r(fx + fw * 0.15)} ${r(fy - fw * 0.2)}L${r(fx + fw * 0.08)} ${r(fy - fw * 0.17)}L${r(fx + fw * 0.03)} ${r(fy - fw * 0.21)}L${r(fx - fw * 0.03)} ${r(fy - fw * 0.16)}L${r(fx - fw * 0.09)} ${r(fy - fw * 0.2)}L${r(fx - fw * 0.15)} ${r(fy - fw * 0.2)}Z`, { fill: '#F6EEDB' }));
    s.add(line(b.x, fy, b.x + b.w, fy, { stroke: SUMI, strokeWidth: 1.2, opacity: 0.5 }));
    // 青海波：底部一条
    const bandH = Math.min(b.h * 0.16, 34 + ukiyoe.reserve.bottom - 20);
    seigaiha(s, b.x, b.y + b.h - bandH, b.w, bandH, Math.max(18, Math.min(30, b.w / 36)));
    grain(s, b, { opacity: 0.12, seed: 12, freq: 0.75, color: '#4A3A20' });
    void m;
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: ukiyoe.f.display, size, fill: o.fill ?? SUMI, anchor, maxW: o.maxW });
  },
  bar(s, x, y, w, h, color, i) {
    // 木版平涂 + 一端ぼかし + 墨线
    if (w <= 0) return;
    const grad = linear(s, `uk-b${i}`, [[0, color, 1], [0.7, color, 1], [1, mix(color, WASHI, 0.55), 1]], [0, 0, 1, 0]);
    s.add(rect(x, y, w, h, { fill: grad, stroke: SUMI, strokeWidth: 1.2, filter: roughen(s, 'uk', 0.9, 0.3, 4) }));
  },
  track(s, x, y, w, h) { s.add(rect(x, y, w, h, { fill: 'none', stroke: SUMI, strokeOpacity: 0.2, strokeWidth: 1 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.42, { fill: 'none', stroke: SUMI, strokeOpacity: 0.4, strokeWidth: 1.2, strokeDasharray: '3 3' })); return; }
    // 家纹式：白圆 + 墨圈
    s.add(circle(cx, cy, size * 0.5, { fill: '#F6EEDB', stroke: SUMI, strokeWidth: 1.4 }));
    s.add(circle(cx, cy, size * 0.42, { fill: 'none', stroke: SUMI, strokeWidth: 0.6 }));
    s.add(logo(key, cx, cy, size * 0.52, SUMI));
  },
  rule(s, x, y, w, kind) {
    if (kind === 'orn') { seigaiha(s, x, y - 7, w, 14, 7); return; }
    s.add(line(x, y - 1.5, x + w, y - 1.5, { stroke: SUMI, strokeWidth: 1.2 }), line(x, y + 1.5, x + w, y + 1.5, { stroke: SUMI, strokeWidth: 0.6 }));
  },
  hatch(s) {
    return s.url('uk-hatch', (id) => el('pattern', { id, width: 8, height: 8, patternUnits: 'userSpaceOnUse' }, rect(0, 0, 8, 8, { fill: '#E4D5B3' }), circle(4, 4, 1.4, { fill: SUMI, opacity: 0.35 })));
  },
});

/** 青海波：一排排扇形同心圆，后排先画 */
export function seigaiha(s: Scene, x: number, y: number, w: number, h: number, R: number) {
  const clip = s.def(`sgclip${Math.round(x)}-${Math.round(y)}-${Math.round(w)}`, (id) => el('clipPath', { id }, rect(x, y, w, h)));
  const parts: string[] = [];
  const rows = Math.ceil(h / (R / 2)) + 2;
  for (let j = 0; j < rows; j++) {
    const cy = y + j * (R / 2);
    const off = j % 2 ? R : 0;
    for (let cx = x - R * 2 + off; cx <= x + w + R * 2; cx += R * 2) {
      parts.push(circle(cx, cy, R, { fill: AI }));
      for (const k of [0.78, 0.56, 0.34]) parts.push(circle(cx, cy, R * k, { fill: 'none', stroke: WASHI, strokeWidth: Math.max(1, R * 0.07) }));
    }
  }
  s.add(g({ clipPath: `url(#${clip})` }, ...parts));
}

// ───────────────────────── 古籍（刻本） ─────────────────────────

const ZHU = '#A8322A';
export const woodblock: Style = defineStyle({
  id: 'woodblock', name: '古籍', en: 'Woodblock Book', note: '明清刻本：竖排、界栏、版心鱼尾、朱批', noteEn: 'A Ming–Qing printed book: vertical lines, ruled columns, fishtail centre folds and red annotations', dark: false,
  c: { bg: '#E9DDC3', paper: '#EFE4CB', ink: '#231E18', soft: '#5C5143', faint: '#9E8F75', line: 'rgba(35,30,24,0.55)', accent: ZHU, accent2: ZHU },
  series: ['#231E18', '#4A4034', '#6E6252', '#938571', '#B2A48C', '#C9BCA2'],
  f: {
    display: { family: 'Zen Antique, Noto Serif SC', weight: 400 }, body: { family: 'Noto Serif SC', weight: 500 },
    num: { family: 'Noto Serif SC', weight: 700 }, label: { family: 'Noto Serif SC', weight: 500 }, mono: { family: 'Noto Serif SC' },
  },
  labelUpper: false, labelTrack: 0.2,
  prefer: { tier: 'vertical', roll: 'vertical' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#EFE4CB" stroke="#231E18" stroke-width="1"/><rect x="5.5" y="5.5" width="13" height="13" fill="none" stroke="#231E18" stroke-width=".9"/><line x1="9.8" y1="5.5" x2="9.8" y2="18.5" stroke="#231E18" stroke-width=".5"/><line x1="14.2" y1="5.5" x2="14.2" y2="18.5" stroke="#231E18" stroke-width=".5"/><path d="M10.6 8l1.4 1.2 1.4-1.2v2.2l-1.4-1.1-1.4 1.1z" fill="#231E18"/><circle cx="16.4" cy="15.6" r="1.3" fill="#A8322A"/>`,
  ground(s, b) {
    s.bg = woodblock.c.bg;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: woodblock.c.bg }));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'wb-age', [[0.5, '#000000', 0], [1, '#6B4E22', 0.22]], { cx: 0.5, cy: 0.45, r: 0.8 }) }));
    // 水渍、虫蛀感：大尺度低频噪声
    const stain = s.def('stain', (id) => el('filter', { id, x: 0, y: 0, width: '100%', height: '100%' },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: 0.004, numOctaves: 4, seed: 21 }),
      el('feColorMatrix', { type: 'matrix', values: '0 0 0 0 0.45 0 0 0 0 0.32 0 0 0 0 0.15 0 0 0 1.6 -0.75' })));
    s.add(rect(b.x, b.y, b.w, b.h, { filter: `url(#${stain})`, opacity: 0.35 }));
    grain(s, b, { opacity: 0.1, seed: 31, freq: 1.1, color: '#3A2A12' });
  },
  plate(s, b) {
    // 版框：外粗内细（文武边）
    const f = roughen(s, 'wbk', 1.1, 0.25, 7);
    s.add(g({ filter: f }, rect(b.x, b.y, b.w, b.h, { fill: 'none', stroke: '#231E18', strokeWidth: 3.2 }), rect(b.x + 6, b.y + 6, b.w - 12, b.h - 12, { fill: 'none', stroke: '#231E18', strokeWidth: 0.9 })));
    return { x: b.x + 6, y: b.y + 6, w: b.w - 12, h: b.h - 12 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    return s.text(text, x, y, { font: woodblock.f.display, size, fill: o.fill ?? '#231E18', anchor, maxW: o.maxW, extra: { filter: roughen(s, 'wbt', 0.8, 0.35, 2) } });
  },
  bar(s, x, y, w, h, color) { s.add(rect(x, y, w, h, { fill: color, filter: roughen(s, 'wbb', 1, 0.3, 3) })); },
  track(s, x, y, w, h) { s.add(rect(x, y, w, h, { fill: 'none', stroke: '#231E18', strokeOpacity: 0.25, strokeWidth: 0.8 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.32, { fill: 'none', stroke: ZHU, strokeWidth: 1.4 })); return; }
    s.add(rect(cx - size * 0.48, cy - size * 0.48, size * 0.96, size * 0.96, { fill: 'none', stroke: '#231E18', strokeWidth: 1 }));
    s.add(logo(key, cx, cy, size * 0.6, '#231E18'));
  },
  rule(s, x, y, w) { s.add(line(x, y, x + w, y, { stroke: '#231E18', strokeWidth: 0.9 })); },
  color(key, rank) {
    if (key === 'unattributed') return '#B2A48C';
    if (key === 'human') return ZHU;
    return woodblock.series[Math.min(rank, 5)];
  },
});

export { seal as inkSeal };
export type { Box, Model };
void alpha;
