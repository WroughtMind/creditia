// 原版主题 → 新风格的钩子。原版只排得出自己那十几种作品；新风格的作品（河流、留存、出处、板块、代码地图、日历……）
// 借这一层按原版主题自己的底色、字体、色阶、框线画出来，看上去还是同一个主题。原版自己的作品不经过这里。
import { defineStyle, logo, type Style, type Box } from '../style.ts';
import { el, rect, circle, line, path, r, mix } from '../svg.ts';
import type { Scene } from '../svg.ts';
import { buildModel as buildV2, type Model as V2Model, type Group } from '../data.ts';
import { art as v2Art, frames as v2Frames } from '../registry.ts';
import { TIER_VARIANTS } from '../pieces/tier.ts';
import { BADGE_VARIANTS } from '../pieces/cards.ts';
import type { Art } from '../export.ts';
import { famColor } from './brand.ts';
import { themeDot, themeName, THEME_INFO } from './catalog.ts';
import type { Theme } from './themes.ts';

/** rgba(...) / #RRGGBB → 颜色 + 不透明度（SVG 属性里分开写，resvg 和浏览器都认） */
export function paint(c: string): { c: string; o: number } {
  const m = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+))?\s*\)$/.exec(c.trim());
  if (!m) return { c, o: 1 };
  const hex = '#' + [m[1], m[2], m[3]].map((x) => Number(x).toString(16).padStart(2, '0')).join('');
  return { c: hex, o: m[4] === undefined ? 1 : Number(m[4]) };
}
/** 半透明色叠在底色上得到的实色（色阶、混色要用实色） */
export const solid = (c: string, over: string) => { const p = paint(c); return p.o >= 1 ? p.c : mix(over, p.c, p.o); };
const fillOf = (c: string) => { const p = paint(c); return { fill: p.c, fillOpacity: p.o < 1 ? p.o : undefined }; };
const strokeOf = (c: string) => { const p = paint(c); return { stroke: p.c, strokeOpacity: p.o < 1 ? p.o : undefined }; };

/** 底板四周留出的框线宽度：内容画在框线以内 */
const INSET: Record<string, number> = { none: 6, double: 16, ornate: 16, rule: 10, thick: 16, pixel: 16 };

const cache = new Map<string, Style>();

export function bridgeStyle(t: Theme): Style {
  const hit = cache.get(t.id);
  if (hit) return hit;
  const paper = solid(t.surface, t.bg);
  const frame = t.frame ?? 'none';
  const numWeight = t.num === t.display ? t.displayWeight : t.num.startsWith('Inter') ? 600 : 400;
  // 终端：图形落在等宽字符的格子上，一格一个方块字（原版的 ■■■···）。格子全图同一套，叠着的几段、上下几行都对得齐。
  // 方块字用图案铺，一根条一个元素，出图不慢
  const term = t.family === 'terminal';
  const G = 12, Q = 9, off = (G - Q) / 2;
  const glyphs = (s: Scene, cols: boolean, color: string) => {
    const h8 = /^(#[0-9a-f]{6})([0-9a-f]{2})$/i.exec(color);
    const p = h8 ? { c: h8[1], o: parseInt(h8[2], 16) / 255 } : paint(color.startsWith('url(') ? t.faint : color);
    return s.url(`glyph-${cols ? 'x' : 'b'}${p.c.replace(/[^0-9a-z]/gi, '')}${p.o < 1 ? Math.round(p.o * 100) : ''}`, (id) => el('pattern', { id, width: G, height: G, patternUnits: 'userSpaceOnUse' },
      rect(off, cols ? 0 : off, Q, cols ? G : Q, { fill: p.c, fillOpacity: p.o < 1 ? p.o : undefined })));
  };
  // 落到格子上：中心在这一段里的格子都算；一格都没有时也给最近的一格（和原版字符条一样，有就至少一个字）
  const span = (a: number, len: number): [number, number] => {
    let k0 = Math.ceil((a - G / 2) / G), k1 = Math.ceil((a + len - G / 2) / G) - 1;
    if (k1 < k0) k0 = k1 = Math.round((a + len / 2 - G / 2) / G);
    return [k0 * G, (k1 - k0 + 1) * G];
  };
  const cells = (s: Scene, x: number, y: number, w: number, h: number, color: string) => {
    if (w <= 0 || h <= 0) return;
    const [x0, ww] = span(x, w), [y0, hh] = span(y, h);
    s.add(rect(x0, y0, ww, hh, { fill: glyphs(s, false, color) }));
  };
  // 条：矮的是一行方块字（高过一个字的居中成一行），高的切成格子；图例色块那么小的本身就是一个字
  const termBar = (s: Scene, x: number, y: number, w: number, h: number, color: string) => {
    if (w <= 0 || h <= 0) return;
    if (w <= 2 * G && Math.abs(w - h) < 2) { const q = Math.min(w, Q); s.add(rect(x + (w - q) / 2, y + (h - q) / 2, q, q, fillOf(color))); return; }
    if (h >= 3 * G) { cells(s, x, y, w, h, color); return; }
    const [x0, ww] = span(x, w), hh = Math.min(h, Q);
    s.add(rect(x0, y + (h - hh) / 2, ww, hh, { fill: glyphs(s, true, color) }));
  };
  const st: Style = defineStyle({
    id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'),
    note: THEME_INFO[t.id]?.note ?? '', noteEn: THEME_INFO[t.id]?.noteEn ?? '', dark: t.dark,
    c: { bg: t.bg, paper, ink: t.ink, soft: t.soft, faint: t.faint, line: solid(t.line, paper), accent: t.accent, accent2: t.soft },
    series: [0, 1, 2, 3, 4, 5].map((i) => famColor('', t, i)),
    f: {
      display: { family: t.display, weight: t.displayWeight },
      body: { family: t.sans, weight: 400 },
      num: { family: t.num, weight: numWeight },
      label: { family: t.labelFont, weight: t.labelFont.startsWith('Inter') ? 500 : 400 },
      mono: { family: 'JetBrains Mono, Noto Sans SC', weight: 400 },
    },
    labelUpper: true, labelTrack: t.labelTrack, radius: t.barRadius,
    dot: () => themeDot(t),
    ground(s: Scene, b: Box, m: V2Model) {
      s.bg = t.bg;
      s.add(rect(b.x, b.y, b.w, b.h, { fill: t.bg }));
      // Glass：底色上两团柔光，颜色取前两名（和原版的 backdrop 一样）
      if (t.glow) {
        const keys = m.rows.filter((x) => x.key !== 'unattributed' && x.key !== 'other').map((x) => x.key);
        const a = famColor(keys[0] ?? '', t) || '#8EA2FF', c = famColor(keys[1] ?? '', t) || '#FFB38E';
        const R = Math.max(b.w, b.h);
        const orb = (name: string, color: string, cx: number, cy: number, rr: number, op: number) =>
          s.add(circle(cx, cy, rr, { fill: s.url(name, (id) => el('radialGradient', { id }, el('stop', { offset: 0, stopColor: color, stopOpacity: op }), el('stop', { offset: 1, stopColor: color, stopOpacity: 0 }))) }));
        orb('bg-a', a, b.x + b.w * 0.18, b.y + b.h * 0.22, R * 0.42, 0.33);
        orb('bg-b', c, b.x + b.w * 0.82, b.y + b.h * 0.78, R * 0.46, 0.31);
        orb('bg-c', '#FFFFFF', b.x + b.w * 0.7, b.y + b.h * 0.18, R * 0.4, 1);
      }
    },
    plate(s: Scene, b: Box): Box {
      const rr = frame === 'none' ? t.radius : 0;
      if (t.shadow !== 'none') {
        const sh = s.url('shadow', (id) => el('filter', { id, x: '-10%', y: '-10%', width: '120%', height: '130%' },
          el('feDropShadow', { dx: 0, dy: 12, stdDeviation: 20, floodColor: t.dark ? '#000000' : '#1A1A1A', floodOpacity: t.dark ? 0.4 : 0.07 })));
        s.add(rect(b.x, b.y, b.w, b.h, { fill: paper, rx: rr, filter: sh }));
      }
      s.add(rect(b.x, b.y, b.w, b.h, { ...fillOf(t.surface), rx: rr }));
      // CRT 的扫描线
      if (t.pattern) {
        const sl = s.url('scan', (id) => el('pattern', { id, width: 4, height: 4, patternUnits: 'userSpaceOnUse' }, rect(0, 0, 4, 1, { fill: '#000000', fillOpacity: 0.3 })));
        s.add(rect(b.x, b.y, b.w, b.h, { fill: sl }));
      }
      const box = (x: number, y: number, w: number, h: number, c: string, sw: number) =>
        s.add(rect(x + sw / 2, y + sw / 2, w - sw, h - sw, { fill: 'none', ...strokeOf(c), strokeWidth: sw, rx: rr ? Math.max(0, rr - sw / 2) : undefined }));
      switch (frame) {
        case 'double': box(b.x, b.y, b.w, b.h, t.accent, 1); box(b.x + 10, b.y + 10, b.w - 20, b.h - 20, t.edge, 1); break;
        case 'ornate': box(b.x, b.y, b.w, b.h, t.edge, 2); box(b.x + 9, b.y + 9, b.w - 18, b.h - 18, t.accent, 3); break;
        case 'rule': s.add(rect(b.x, b.y, b.w, 5, { fill: t.ink }), rect(b.x, b.y + b.h - 5, b.w, 5, { fill: t.ink })); break;
        case 'thick': box(b.x, b.y, b.w, b.h, t.ink, 7); break;
        case 'pixel': box(b.x, b.y, b.w, b.h, t.edge, 4); box(b.x + 8, b.y + 8, b.w - 16, b.h - 16, /^#[0-9a-f]{6}$/i.test(t.accent) ? t.accent + '66' : t.accent, 2); break;
        default: box(b.x, b.y, b.w, b.h, t.edge, t.family === 'editorial' || t.family === 'mono' ? 2 : 1);
      }
      const k = INSET[frame] ?? 6;
      return { x: b.x + k, y: b.y + k, w: b.w - k * 2, h: b.h - k * 2 };
    },
    bar(s: Scene, x: number, y: number, w: number, h: number, color: string) {
      if (term) { termBar(s, x, y, w, h, color); return; }
      const rx = Math.min(t.barRadius, h / 2, w / 2);
      if (t.family === 'neon' && w > 0 && h > 0) {
        // 霓虹：和原版作品一样是灯管——后面一层模糊的光，前面亮边暗芯；太细的条直接是一根亮线。
        // 光晕的组里垫一个不画的大框，滤镜区域按它算，细条的包围盒再扁也不会把光裁掉
        const glow = s.url('neon-glow', (id) => el('filter', { id, x: '-10%', y: '-10%', width: '120%', height: '120%' }, el('feGaussianBlur', { stdDeviation: 5 })));
        s.add(el('g', { filter: glow, opacity: 0.85 }, rect(x - 16, y - 16, w + 32, h + 32, { fill: 'none' }), rect(x, y, w, h, { fill: color, rx })));
        if (w >= 8 && h >= 8) s.add(rect(x + 0.75, y + 0.75, w - 1.5, h - 1.5, { fill: color, fillOpacity: 0.22, stroke: color, strokeWidth: 1.5, rx }));
        else s.add(rect(x, y, w, h, { fill: color, rx }));
        return;
      }
      s.add(rect(x, y, w, h, { fill: color, rx }));
    },
    // 霓虹的大色块（板块）：亮边暗芯的灯管，不加光晕（一张图几十块，光晕会让出图慢很多）；别的原版主题不写，照旧画实色
    ...(t.family === 'neon' ? {
      block(s: Scene, x: number, y: number, w: number, h: number, color: string) {
        const rx = Math.min(t.barRadius, 6, h / 2, w / 2);
        if (w < 6) { s.add(rect(x, y, w, h, { fill: color, rx })); return; }
        s.add(rect(x + 0.75, y + 0.75, w - 1.5, h - 1.5, { fill: color, fillOpacity: 0.22, stroke: color, strokeWidth: 1.5, rx }));
      },
    } : {}),
    ...(term ? { grid: G, cells, block: termBar } : {}),
    track(s: Scene, x: number, y: number, w: number, h: number) {
      // 终端：没走到的地方是一排点（原版的 ···），点在每一格正中，条画上来正好盖住
      if (term) {
        const [x0, ww] = span(x, w);
        s.add(line(x0 + G / 2 - 1, y + h / 2, x0 + ww - G / 2 + 1, y + h / 2, { ...strokeOf(t.faint), strokeWidth: 2, strokeDasharray: `2 ${G - 2}` }));
        return;
      }
      s.add(rect(x, y, w, h, { ...fillOf(t.track), rx: Math.min(t.barRadius, h / 2) }));
    },
    mark(s: Scene, key: string, cx: number, cy: number, size: number, color?: string) {
      if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.31, { fill: 'none', stroke: t.faint, strokeWidth: Math.max(1.25, size / 11), strokeDasharray: `${r(size / 8)} ${r(size / 10)}` })); return; }
      s.add(logo(key, cx, cy, size, color ?? t.ink));
    },
    rule(s: Scene, x: number, y: number, w: number, kind?: 'thin' | 'orn') {
      if (kind !== 'orn') { s.add(line(x, y, x + w, y, { ...strokeOf(t.line), strokeWidth: 1 })); return; }
      // 装饰分隔：两段细线中间一个菱形（各体裁的符号在原版作品里，这里统一成几何形，免得缺字）
      const cx = x + w / 2, d = 4;
      s.add(line(x, y, cx - d * 3, y, { ...strokeOf(t.line), strokeWidth: 1 }), line(cx + d * 3, y, x + w, y, { ...strokeOf(t.line), strokeWidth: 1 }),
        path(`M${r(cx)} ${r(y - d)}L${r(cx + d)} ${r(y)}L${r(cx)} ${r(y + d)}L${r(cx - d)} ${r(y)}Z`, { fill: t.accent }));
    },
    hatch(s: Scene) {
      return s.url('hatch', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' },
        rect(0, 0, 6, 6, { fill: t.stripeB }), rect(0, 0, 3, 6, { fill: t.stripeA })));
    },
    color(key: string, rank: number) {
      if (key === 'unattributed') return t.faint;
      if (key === 'other') return mix(t.faint, paper, 0.3);
      if (key === 'human') return t.soft;
      return famColor(key, t, rank);
    },
  });
  cache.set(t.id, st);
  return st;
}

// ───────────────────────── 新作品 ─────────────────────────

/** 原版没有、借新风格的作品画出来的：河流、留存、出处、板块、代码地图、日历 */
export const BRIDGED = ['stream', 'survival', 'evidence', 'areas', 'codemap', 'calendar'];
/** 原版作品追加的新风格版式（和原版版式重名的不追加）；原版自己的画法叫 original，仍是默认 */
export const BRIDGED_LAYOUTS: Record<string, string[]> = {
  tier: TIER_VARIANTS,
  roll: ['card', 'vertical', 'ed', 'terminal', 'troupe'],
  badge: BADGE_VARIANTS,
  donut: ['nested'],
  // 一个工具里多个模型时的版式
  receipt: ['itemized'],
  waffle: ['blocks'],
  byline: ['models'],
};
export const isBridged = (id: string, variant?: string) => BRIDGED.includes(id) || (!!variant && !!BRIDGED_LAYOUTS[id]?.includes(variant));

/** 新作品读新模型；同一份档案、同一种语言、同一种分组只建一次 */
const models = new WeakMap<object, Record<string, V2Model>>();
export function v2ModelOf(raw: any, lang: 'zh' | 'en', group: Group = 'tool'): V2Model {
  const k = raw && typeof raw === 'object' ? raw : {};
  const hit = models.get(k) ?? {};
  const key = `${lang}|${group}`;
  if (!hit[key]) { hit[key] = buildV2(raw, { lang, named: false, sign: '', group }); models.set(k, hit); }
  return hit[key];
}

// 分组方式跟着原版模型走（它已经按 auto 选好了），两套渲染器的数字一致
export const bridgeArt = (id: string, m: { raw: any; group?: Group }, t: Theme, o: { lang: 'zh' | 'en'; variant?: string }): Art | null =>
  v2Art(id, v2ModelOf(m.raw, o.lang, m.group), bridgeStyle(t), { variant: o.variant });
export const bridgeFrames = (id: string, m: { raw: any; group?: Group }, t: Theme, o: { lang: 'zh' | 'en'; variant?: string }, n = 24): Art[] =>
  v2Frames(id, v2ModelOf(m.raw, o.lang, m.group), bridgeStyle(t), { variant: o.variant }, n);
