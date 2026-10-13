// 流行两套：漫画（网点、集中线、拟声字）、终端（磷光、代码雨、ASCII 条）。
import { defineStyle, grain, linear, radial, blur, logo, type Style } from '../style.ts';
import { Scene, el, g, rect, circle, line, path, r, rng, hash, measure } from '../svg.ts';

// ───────────────────────── 漫画 ─────────────────────────

const K = '#111111', W = '#FFFFFF', MAG = '#B0287A', PUR = '#5B2A86', YEL = '#FFD23F';
/** 网点：圆点大小渐变（screentone） */
export function screentone(s: Scene, name: string, color = K, size = 7, dot = 1.6) {
  return s.url(`mg-tone-${name}`, (id) => el('pattern', { id, width: size, height: size, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, circle(size / 2, size / 2, dot, { fill: color })));
}
/** 集中线：从中心向外的黑色细三角 */
export function speedLines(s: Scene, cx: number, cy: number, R0: number, R1: number, n: number, seed: number, color = K) {
  const rnd = rng(seed);
  const parts: string[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.04;
    const w = 0.004 + rnd() * 0.012;
    const r0 = R0 * (0.85 + rnd() * 0.5);
    parts.push(path(`M${r(cx + Math.cos(a - w) * R1)} ${r(cy + Math.sin(a - w) * R1)}L${r(cx + Math.cos(a) * r0)} ${r(cy + Math.sin(a) * r0)}L${r(cx + Math.cos(a + w) * R1)} ${r(cy + Math.sin(a + w) * R1)}Z`, { fill: color }));
  }
  s.add(g({}, ...parts));
}
/** 拟声字：描边、倾斜、错落 */
export function sfx(s: Scene, text: string, x: number, y: number, size: number, rot: number, color = PUR) {
  s.text(text, x, y, { font: { family: 'Dela Gothic One, Noto Sans SC', weight: 400 }, size, fill: color, stroke: W, strokeW: size * 0.12, rotate: rot, anchor: 'middle' });
}
export const manga: Style = defineStyle({
  id: 'manga', name: '漫画', en: 'Manga', note: '少年漫画的张力：网点、集中线、「ゴゴゴ」与替身面板', noteEn: 'Shōnen manga tension: screentone, speed lines, “ゴゴゴ” and stand panels', dark: false,
  c: { bg: W, paper: W, ink: K, soft: '#444444', faint: '#9A9A9A', line: 'rgba(0,0,0,0.85)', accent: MAG, accent2: PUR },
  series: [K, MAG, PUR, '#2B6CB0', '#D9481C', '#3A8A3A'],
  f: {
    display: { family: 'Dela Gothic One, Noto Sans SC', weight: 400 }, body: { family: 'Noto Sans SC', weight: 700 }, num: { family: 'Bangers, Noto Sans SC', weight: 400 },
    label: { family: 'Noto Sans SC', weight: 900 }, mono: { family: 'Noto Sans SC', weight: 700 },
  },
  labelTrack: 0.08, prefer: { tier: 'stats', roll: 'card' },
  dot: () => `<defs><pattern id="mgd" width="3" height="3" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><circle cx="1.5" cy="1.5" r=".7" fill="#111"/></pattern></defs><circle cx="12" cy="12" r="11" fill="#fff" stroke="#111" stroke-width="1.4"/><path d="M12 1L14 9 23 8 15 13 19 22 12 16 5 22 9 13 1 8 10 9Z" fill="url(#mgd)"/><text x="12" y="15.5" font-size="9" font-weight="900" text-anchor="middle" fill="#5B2A86" stroke="#fff" stroke-width="1.2" paint-order="stroke" font-family="sans-serif">ゴ</text>`,
  ground(s, b, m) {
    s.bg = W;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: W }));
    // 斜分镜：两块网点面
    s.add(path(`M${r(b.x)} ${r(b.y)}H${r(b.x + b.w * 0.62)}L${r(b.x + b.w * 0.38)} ${r(b.y + b.h * 0.3)}H${r(b.x)}Z`, { fill: screentone(s, 'a', K, 6, 1.1), opacity: 0.55 }));
    speedLines(s, b.x + b.w * 0.82, b.y + b.h * 0.14, Math.min(b.w, b.h) * 0.12, Math.max(b.w, b.h) * 1.1, 120, hash(m.project), K);
    s.add(circle(b.x + b.w * 0.82, b.y + b.h * 0.14, Math.min(b.w, b.h) * 0.14, { fill: W, filter: blur(s, 10) }));
    // 拟声字
    const big = Math.min(b.w, b.h) * 0.075;
    sfx(s, 'ゴ', b.x + b.w * 0.9, b.y + b.h * 0.36, big, 12);
    sfx(s, 'ゴ', b.x + b.w * 0.94, b.y + b.h * 0.47, big * 0.82, 14);
    sfx(s, 'ゴ', b.x + b.w * 0.07, b.y + b.h * 0.86, big * 0.9, -10);
    sfx(s, 'ゴ', b.x + b.w * 0.12, b.y + b.h * 0.94, big * 0.7, -8);
  },
  plate(s, b) {
    // 粗黑框分镜
    s.add(rect(b.x + 6, b.y + 6, b.w - 12, b.h - 12, { fill: W, fillOpacity: 0.94, stroke: K, strokeWidth: 6 }));
    return { x: b.x + 6, y: b.y + 6, w: b.w - 12, h: b.h - 12 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    s.text(text, x + size * 0.06, y + size * 0.06, { font: manga.f.display, size: size * 1.1, fill: K, anchor, maxW: o.maxW, stroke: K, strokeW: size * 0.12 });
    return s.text(text, x, y, { font: manga.f.display, size: size * 1.1, fill: o.fill ?? YEL, anchor, maxW: o.maxW, stroke: K, strokeW: size * 0.1 });
  },
  num(s, text, x, y, size, anchor = 'end', o = {}) { return s.text(text, x, y, { font: manga.f.num, size: size * 1.2, fill: o.fill ?? K, anchor, track: 0.02 }); },
  bar(s, x, y, w, h, color, i) {
    if (w <= 0) return;
    const hh = Math.max(h, 14);
    s.add(path(`M${r(x)} ${r(y - (hh - h) / 2)}H${r(x + w)}L${r(x + w - hh * 0.5)} ${r(y + h + (hh - h) / 2)}H${r(x)}Z`, { fill: i === 0 ? K : screentone(s, `b${i}`, color, 5, 1.6), stroke: K, strokeWidth: 2 }));
  },
  track(s, x, y, w, h) { s.add(rect(x, y + h / 2 - 1, w, 2, { fill: K, opacity: 0.15 })); },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.text('？', cx, cy + size * 0.3, { font: manga.f.display, size: size * 0.8, fill: PUR, anchor: 'middle', stroke: W, strokeW: 3 }); return; }
    s.add(path(`M${r(cx - size * 0.52)} ${r(cy - size * 0.42)}L${r(cx + size * 0.52)} ${r(cy - size * 0.52)}L${r(cx + size * 0.46)} ${r(cy + size * 0.5)}L${r(cx - size * 0.5)} ${r(cy + size * 0.44)}Z`, { fill: W, stroke: K, strokeWidth: 2.4 }));
    s.add(logo(key, cx, cy, size * 0.58, K));
  },
  rule(s, x, y, w) { s.add(path(`M${r(x)} ${r(y - 2)}H${r(x + w)}L${r(x + w - 8)} ${r(y + 3)}H${r(x)}Z`, { fill: K })); },
  hatch(s) { return screentone(s, 'h', '#888888', 5, 1); },
  color(key, rank) { return key === 'unattributed' ? '#9A9A9A' : key === 'human' ? MAG : manga.series[Math.min(rank, 5)]; },
});

// ───────────────────────── 终端 ─────────────────────────

const PH = '#7CFF9E', TBG = '#060A08';
const MONO = { family: 'JetBrains Mono, DotGothic16, Noto Sans SC', weight: 400 };
export const terminal: Style = defineStyle({
  id: 'terminal', name: '终端', en: 'Terminal', note: '磷光绿的老终端：提示符、ASCII 条、背后的代码雨', noteEn: 'A phosphor-green terminal: prompts, ASCII bars and code rain behind', dark: true,
  c: { bg: TBG, paper: '#09100C', ink: '#D6FFE0', soft: '#5FBF7E', faint: '#2E6B45', line: 'rgba(124,255,158,0.18)', accent: PH, accent2: '#FFB347' },
  series: [PH, '#54D6FF', '#FFB347', '#FF6B8B', '#C9A0FF', '#E8F57A'],
  f: { display: { family: 'JetBrains Mono, DotGothic16, Noto Sans SC', weight: 700 }, body: MONO, num: { family: 'JetBrains Mono, Noto Sans SC', weight: 700 }, label: MONO, mono: MONO },
  labelUpper: false, labelTrack: 0.04, prefer: { tier: 'rows', roll: 'terminal' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#060A08" stroke="#2E6B45" stroke-width="1"/><path d="M6 8l3.4 3L6 14" fill="none" stroke="#7CFF9E" stroke-width="1.6"/><rect x="11" y="13.4" width="6" height="1.6" fill="#7CFF9E"/>`,
  ground(s, b, m) {
    s.bg = TBG;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: TBG }));
    // 代码雨：竖列片假名与数字，越往下越淡
    const rnd = rng(hash(m.project) + 77);
    const glyphs = 'アイウエオカキクケコサシスセソタチツテト0123456789ナニヌネノハヒフヘホマミムメモ';
    const cols: string[] = [];
    for (let x = b.x + 10; x < b.x + b.w; x += 22) {
      if (rnd() < 0.45) continue;
      const len = 4 + Math.floor(rnd() * 14), y0 = b.y + rnd() * b.h * 0.7;
      for (let k = 0; k < len; k++) cols.push(el('text', { x, y: y0 + k * 20, fontFamily: 'DotGothic16, JetBrains Mono', fontSize: 16, fill: PH, opacity: r(0.04 + 0.16 * (k / len)) }, glyphs[Math.floor(rnd() * glyphs.length)]));
    }
    s.add(g({}, ...cols));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'tm-glow', [[0, '#123A22', 0.55], [1, TBG, 0]], { cx: 0.5, cy: 0.4, r: 0.7 }) }));
    const scan = s.url('tm-scan', (id) => el('pattern', { id, width: 3, height: 3, patternUnits: 'userSpaceOnUse' }, rect(0, 0, 3, 1, { fill: '#000000', opacity: 0.45 })));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: scan }));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'tm-vig', [[0.6, '#000000', 0], [1, '#000000', 0.7]], { cx: 0.5, cy: 0.5, r: 0.75 }) }));
  },
  plate(s, b) {
    // 窗口：标题栏三个点
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#08100B', fillOpacity: 0.86, stroke: '#1E3D2A', strokeWidth: 1.2, rx: 10 }));
    s.add(rect(b.x, b.y, b.w, 34, { fill: '#0E1A13', rx: 10 }), rect(b.x, b.y + 24, b.w, 10, { fill: '#0E1A13' }));
    ['#FF5F57', '#FEBC2E', '#28C840'].forEach((c, i) => s.add(circle(b.x + 20 + i * 18, b.y + 17, 5, { fill: c, opacity: 0.85 })));
    s.text('creditia — zsh', b.x + b.w / 2, b.y + 22, { font: MONO, size: 12, fill: '#5FBF7E', anchor: 'middle' });
    return { x: b.x, y: b.y + 30, w: b.w, h: b.h - 30 };
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    const glow = s.url('tm-gl', (id) => el('filter', { id, x: '-20%', y: '-50%', width: '140%', height: '200%' }, el('feGaussianBlur', { stdDeviation: 4, result: 'b' }), el('feMerge', {}, el('feMergeNode', { in: 'b' }), el('feMergeNode', { in: 'SourceGraphic' }))));
    const prompt = anchor === 'start' ? s.text('$ ', x, y, { font: terminal.f.display, size: size * 0.7, fill: '#2E6B45' }) : 0;
    return prompt + s.text(text, x + prompt, y, { font: terminal.f.display, size: size * 0.7, fill: o.fill ?? PH, anchor, maxW: o.maxW, extra: { filter: glow } });
  },
  bar(s, x, y, w, h, color) {
    if (h > w * 1.5) {
      // 竖条：一格一格的方块往上堆
      const cell = Math.max(6, w * 0.8), n = Math.max(1, Math.floor(h / (cell + 2)));
      const parts: string[] = [];
      for (let k = 0; k < n; k++) parts.push(rect(x, y + h - (k + 1) * (cell + 2), w, cell, { fill: color }));
      s.add(g({}, ...parts));
      return;
    }
    const size = 16, cw = measure('█', MONO, size);
    const n = Math.max(1, Math.round(w / cw));
    s.text('█'.repeat(n), x, y + h / 2 + 6, { font: MONO, size, fill: color });
  },
  track(s, x, y, w, h) { const cw = measure('░', MONO, 16); s.text('░'.repeat(Math.floor(w / cw)), x, y + h / 2 + 6, { font: MONO, size: 16, fill: '#1F4A2E' }); },
  mark(s, key, cx, cy, size, color) {
    if (key === 'unattributed') { s.text('??', cx, cy + 6, { font: MONO, size: 16, fill: '#2E6B45', anchor: 'middle' }); return; }
    s.add(logo(key, cx, cy, size * 0.72, color ?? PH));
  },
  rule(s, x, y, w) { const cw = measure('─', MONO, 14); s.text('─'.repeat(Math.floor(w / cw)), x, y + 5, { font: MONO, size: 14, fill: '#1F4A2E' }); },
  hatch(s) { return s.url('tm-h', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 6, 6, { fill: TBG }), rect(0, 0, 1.4, 6, { fill: '#2E6B45' }))); },
  color(key, rank) { return key === 'unattributed' ? '#2E6B45' : key === 'human' ? '#FFB347' : terminal.series[Math.min(rank, 5)]; },
  on() { return TBG; },
});

void linear; void grain; void line; void YEL;
