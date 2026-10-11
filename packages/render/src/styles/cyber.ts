// 赛博：酸黄、青、警示红；倒角面板、HUD 角标、扫描线、错位故障。
import { defineStyle, grain, linear, chamfer, logo, type Style } from '../style.ts';
import { Scene, el, g, rect, line, path, circle, r, rng, hash, measure } from '../svg.ts';

const Y = '#F3E600', C = '#00E5F0', R = '#FF2A4D', BG = '#08090D', INK = '#E9ECEF';
const DISP = { family: 'Rajdhani, Noto Sans SC', weight: 700 };

export const cyber: Style = defineStyle({
  id: 'cyber', name: '赛博', en: 'Cyber', note: '霓虹都市的 HUD：倒角面板、故障错位、条形码与警示条', dark: true,
  c: { bg: BG, paper: '#0E1016', ink: INK, soft: '#8A93A3', faint: '#4A5262', line: 'rgba(0,229,240,0.16)', accent: Y, accent2: C },
  series: [Y, C, R, '#B6FF3B', '#FF8A1F', '#B983FF'],
  f: {
    display: DISP, body: { family: 'Chakra Petch, Noto Sans SC', weight: 400 }, num: { family: 'Rajdhani, Noto Sans SC', weight: 700 },
    label: { family: 'Share Tech Mono, Noto Sans SC', weight: 400 }, mono: { family: 'Share Tech Mono, Noto Sans SC', weight: 400 },
  },
  labelTrack: 0.18,
  prefer: { tier: 'hud', roll: 'terminal' },
  dot: () => `<circle cx="12" cy="12" r="11" fill="#08090D" stroke="#F3E600" stroke-width="1.2"/><path d="M5 9h9l2 2v4H5z" fill="#F3E600"/><rect x="4" y="15.5" width="11" height="1.6" fill="#00E5F0"/><rect x="13" y="7" width="7" height="1.4" fill="#FF2A4D"/>`,
  ground(s, b, m) {
    s.bg = BG;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: BG }));
    // 网格 + 扫描线
    const grid = s.url('cy-grid', (id) => el('pattern', { id, width: 32, height: 32, patternUnits: 'userSpaceOnUse' }, path('M32 0H0V32', { fill: 'none', stroke: C, strokeOpacity: 0.07, strokeWidth: 1 })));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: grid }));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: linear(s, 'cy-glow', [[0, '#1A1030', 0.0], [0.65, '#1A1030', 0.0], [1, '#2A0E2E', 0.85]]) }));
    const scan = s.url('cy-scan', (id) => el('pattern', { id, width: 4, height: 4, patternUnits: 'userSpaceOnUse' }, rect(0, 0, 4, 1.2, { fill: '#000000', opacity: 0.35 })));
    // 角标
    const k = 26, pad = 18;
    const corner = (x: number, y: number, sx: number, sy: number) => path(`M${r(x)} ${r(y + sy * k)}V${r(y)}H${r(x + sx * k)}`, { fill: 'none', stroke: Y, strokeWidth: 2 });
    s.add(corner(b.x + pad, b.y + pad, 1, 1), corner(b.x + b.w - pad, b.y + pad, -1, 1), corner(b.x + pad, b.y + b.h - pad, 1, -1), corner(b.x + b.w - pad, b.y + b.h - pad, -1, -1));
    // 警示条 + 系统字
    const hz = s.url('cy-hz', (id) => el('pattern', { id, width: 14, height: 14, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 7, 14, { fill: Y })));
    s.add(rect(b.x + b.w - pad - 120, b.y + pad + 8, 92, 8, { fill: hz }));
    s.text(`SYS//CREDITIA  ${String(hash(m.project) % 10000).padStart(4, '0')}-${String(m.commits).padStart(4, '0')}`, b.x + pad + 34, b.y + pad + 16, { font: cyber.f.label, size: 11, fill: C, track: 0.2, opacity: 0.85 });
    // 条形码
    const rnd = rng(hash(m.project) + 3);
    let bx = b.x + b.w - pad - 34 - 110;
    const by = b.y + b.h - pad - 30;
    const bars: string[] = [];
    while (bx < b.x + b.w - pad - 34) { const w = 1 + Math.floor(rnd() * 3); bars.push(rect(bx, by, w, 18, { fill: INK, opacity: 0.55 })); bx += w + 1 + Math.floor(rnd() * 3); }
    s.add(...bars);
    s.add(rect(b.x, b.y, b.w, b.h, { fill: scan }));
    grain(s, b, { opacity: 0.06, seed: 41, color: '#FFFFFF' });
  },
  plate(s, b) {
    s.add(path(chamfer(b.x, b.y, b.w, b.h, 22), { fill: '#0C0E14', fillOpacity: 0.92, stroke: Y, strokeOpacity: 0.85, strokeWidth: 1.5 }));
    s.add(rect(b.x + 22, b.y - 1, 70, 3, { fill: Y }));
    s.add(rect(b.x + b.w - 60, b.y + b.h - 2, 40, 3, { fill: C }));
    return b;
  },
  title(s, text, x, y, size, anchor = 'start', o = {}) {
    const t = text.toUpperCase();
    const opts = { font: DISP, size, anchor, maxW: o.maxW, track: 0.02 } as const;
    s.text(t, x - size * 0.035, y, { ...opts, fill: C, opacity: 0.85 });
    s.text(t, x + size * 0.035, y + size * 0.015, { ...opts, fill: R, opacity: 0.85 });
    const w = s.text(t, x, y, { ...opts, fill: o.fill ?? Y });
    // 故障切片：一条横带错位
    const x0 = anchor === 'middle' ? x - w / 2 : anchor === 'end' ? x - w : x;
    const clip = s.def(`cy-slice-${Math.round(x)}-${Math.round(y)}`, (id) => el('clipPath', { id }, rect(x0 - 20, y - size * 0.52, w + 40, size * 0.12)));
    s.add(g({ clipPath: `url(#${clip})`, transform: `translate(${r(size * 0.06)} 0)` }, rect(x0 - 20, y - size * 0.52, w + 40, size * 0.12, { fill: BG }),
      el('text', { x, y, fontFamily: DISP.family, fontWeight: 700, fontSize: Math.min(size, o.maxW ? size : size), textAnchor: anchor === 'start' ? null : anchor, fill: INK, letterSpacing: r(size * 0.02) }, t)));
    return w;
  },
  bar(s, x, y, w, h, color, i) {
    if (h > w * 1.5) {
      const seg = Math.max(6, w * 0.7), gap = Math.max(2, seg * 0.3);
      const n = Math.max(1, Math.floor((h + gap) / (seg + gap)));
      const parts: string[] = [];
      for (let k = 0; k < n; k++) parts.push(rect(x, y + h - (k + 1) * (seg + gap) + gap, w, seg, { fill: k === n - 1 ? '#FFFFFF' : color, opacity: k === n - 1 ? 0.95 : 0.6 + 0.4 * (k / n) }));
      s.add(g({}, ...parts));
      return;
    }
    // LED 分段：最后一段发白
    const seg = Math.max(6, h * 1.1), gap = Math.max(2, seg * 0.28);
    const n = Math.max(1, Math.floor((w + gap) / (seg + gap)));
    const parts: string[] = [];
    for (let k = 0; k < n; k++) parts.push(rect(x + k * (seg + gap), y, seg, h, { fill: k === n - 1 ? '#FFFFFF' : color, opacity: k === n - 1 ? 0.95 : 0.6 + 0.4 * (k / n) }));
    s.add(g({}, ...parts));
    void i;
  },
  track(s, x, y, w, h) {
    const seg = Math.max(6, h * 1.1), gap = Math.max(2, seg * 0.28);
    const n = Math.floor((w + gap) / (seg + gap));
    const parts: string[] = [];
    for (let k = 0; k < n; k++) parts.push(rect(x + k * (seg + gap), y, seg, h, { fill: C, opacity: 0.08 }));
    s.add(g({}, ...parts));
  },
  mark(s, key, cx, cy, size, color) {
    if (key === 'unattributed') { s.add(path(chamfer(cx - size * 0.45, cy - size * 0.45, size * 0.9, size * 0.9, size * 0.2), { fill: 'none', stroke: cyber.c.faint, strokeWidth: 1.2, strokeDasharray: '3 3' })); return; }
    s.add(path(chamfer(cx - size * 0.5, cy - size * 0.5, size, size, size * 0.22), { fill: '#0C0E14', stroke: color ?? Y, strokeWidth: 1.3 }));
    s.add(logo(key, cx, cy, size * 0.56, color ?? Y));
  },
  rule(s, x, y, w, kind) {
    s.add(line(x, y, x + w, y, { stroke: C, strokeOpacity: 0.35, strokeWidth: 1 }));
    if (kind === 'orn') s.add(rect(x, y - 1.5, 46, 3, { fill: Y }), rect(x + w - 12, y - 1.5, 12, 3, { fill: R }));
  },
  hatch(s) {
    return s.url('cy-hatch', (id) => el('pattern', { id, width: 8, height: 8, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 8, 8, { fill: '#11131A' }), rect(0, 0, 2, 8, { fill: '#4A5262' })));
  },
  on(color) { return color === Y || color === C || color === '#B6FF3B' ? BG : '#FFFFFF'; },
});

void measure; void circle; void Scene;
