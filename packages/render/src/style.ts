// 风格：不是换色，是一套视觉语言。每个风格提供同一组钩子（底、标题、条、标记、分隔、斜纹……），
// 作品只调用钩子，所以任何作品 × 任何风格都成立；风格还可以指定某个作品的默认版式。
import { Scene, el, g, rect, path, circle, line, r, esc, measure, mix, alpha, lum, type FontSpec, type Attrs } from './svg.ts';
import { LOGOS } from './logos.ts';
import type { Model } from './data.ts';
import { SHADES } from './data.ts';

export interface Box { x: number; y: number; w: number; h: number; /** 底板角上装饰（涡卷）伸进内容区的方块边长：标题、图例这类贴角的文字要让开 */ corner?: number }
export interface Palette {
  bg: string; paper: string; ink: string; soft: string; faint: string; line: string; accent: string; accent2: string;
}
export interface Fonts { display: FontSpec; body: FontSpec; num: FontSpec; label: FontSpec; mono: FontSpec; script?: FontSpec }

export interface Style {
  id: string;
  name: string;
  en: string;
  /** 一句话说明：灵感来源、气质 */
  note: string;
  noteEn: string;
  dark: boolean;
  c: Palette;
  /** 贡献方配色，按名次取 */
  series: string[];
  f: Fonts;
  labelUpper: boolean;
  labelTrack: number;
  radius: number;
  /** 各作品的默认版式 */
  prefer: Record<string, string>;
  /** 底图（天空、远山、海浪）占用的上下边距：作品内容避开这一块 */
  reserve: { top: number; bottom: number };
  /** 小圆点：24×24 里画出风格的样子（App 左下角） */
  dot(): string;
  /** 整张作品的底：纸、天空、电路板…… */
  ground(s: Scene, b: Box, m: Model): void;
  /** 窄幅（署名行 1200×120）的底，可选：大画幅的构图挪到窄条上会压住文字，这里装饰只放在上下边和两头，中间留整条文字带。
   *  aside 是右端统计那一块，风格可以给它垫一块底 */
  strip?(s: Scene, b: Box, m: Model, aside: Box): void;
  /** 可选：板块这类大色块的画法（不写就是实色圆角矩形）。原版霓虹用它把色块画成灯管 */
  block?(s: Scene, x: number, y: number, w: number, h: number, color: string): void;
  /** 可选：字符格的格距。和 cells 一起写：河流这类平滑图形按全图同一套格子画，一格一个方块字（原版终端） */
  grid?: number;
  /** 可选：把这块区域画成落在字符格上的方块字（■）；opacity 用来画浅一层的底 */
  cells?(s: Scene, x: number, y: number, w: number, h: number, color: string, opacity?: number): void;
  /** 卡片面（可选）：玻璃、羊皮纸、面板。返回内容区 */
  plate(s: Scene, b: Box): Box;
  title(s: Scene, text: string, x: number, y: number, size: number, anchor?: 'start' | 'middle' | 'end', o?: { maxW?: number; fill?: string }): number;
  label(s: Scene, text: string, x: number, y: number, o?: { anchor?: 'start' | 'middle' | 'end'; size?: number; fill?: string; maxW?: number }): number;
  num(s: Scene, text: string, x: number, y: number, size: number, anchor?: 'start' | 'middle' | 'end', o?: { fill?: string }): number;
  bar(s: Scene, x: number, y: number, w: number, h: number, color: string, i: number): void;
  track(s: Scene, x: number, y: number, w: number, h: number): void;
  mark(s: Scene, key: string, cx: number, cy: number, size: number, color?: string): void;
  rule(s: Scene, x: number, y: number, w: number, kind?: 'thin' | 'orn'): void;
  /** 未确认的填充 */
  hatch(s: Scene): string;
  color(key: string, rank: number): string;
  /** 底色上的文字颜色 */
  on(color: string): string;
}

// ───────────────────────── 品牌 ─────────────────────────

/** 工具 → logo 名 */
export const LOGO_OF: Record<string, string> = {
  'claude-code': 'claude', codex: 'openai', cursor: 'cursor', grok: 'grok', kimi: 'kimi', gemini: 'geminicli', qwen: 'qwen', cline: 'cline',
  copilot: 'githubcopilot', amp: 'amp', windsurf: 'windsurf', trae: 'trae', deepseek: 'deepseek', 'github-actions': 'github-actions',
  dependabot: 'dependabot', renovate: 'renovate', opencode: 'opencode', roo: 'roocode', kilo: 'kilocode', devin: 'devin', jules: 'googlejules', aider: 'aider',
  crush: 'crush', goose: 'goose', continue: 'continue', zed: 'zed', 'amazon-q': 'amazonq', antigravity: 'antigravity',
};
/** 各家熟悉的品牌色（只在愿意用品牌色的风格里用） */
export const BRAND: Record<string, string> = {
  'claude-code': '#D97757', codex: '#10A37F', cursor: '#3B82F6', grok: '#6B6B70', kimi: '#2F6BFF', gemini: '#8E75B2', qwen: '#615CED',
  opencode: '#E8A33D', cline: '#5B8DEF', roo: '#C2410C', kilo: '#E5484D', copilot: '#6E40C9', aider: '#14B014', amp: '#F34E3F',
  windsurf: '#0FBF8F', trae: '#FF4D4F', crush: '#6B50FF', goose: '#1A1A1A', continue: '#3B3B3B', zed: '#084CCF', 'amazon-q': '#8C4FFF', antigravity: '#3279F9', deepseek: '#4D6BFE', 'github-actions': '#2088FF', devin: '#1F2937', jules: '#7C5CFF',
};

/** 品牌色：按模型分组时 key 是「工具/模型」，同一工具的第二、三个模型在品牌色上依次调深、调浅 */
export function brandOf(key: string): string | undefined {
  const i = key.indexOf('/');
  const b = BRAND[i < 0 ? key : key.slice(0, i)];
  if (!b || i < 0) return b;
  const k = SHADES.get(key) ?? 0;
  return k === 0 ? b : mix(b, k % 2 ? '#000000' : '#FFFFFF', Math.min(0.6, 0.28 + 0.14 * Math.floor((k - 1) / 2)));
}

/** 画 logo；没有 logo 的工具画首字母。按模型分组时 key 是「工具/模型」，画工具的 logo */
export function logo(key: string, cx: number, cy: number, size: number, fill: string, font: FontSpec = { family: 'Inter, Noto Sans SC', weight: 700 }): string {
  key = key.split('/')[0];
  const lg = LOGOS[LOGO_OF[key] ?? ''];
  if (lg) {
    const [, , vw, vh] = lg.vb.split(/\s+/).map(Number);
    const k = size / Math.max(vw, vh);
    return el('path', { d: lg.d, fill, fillRule: lg.rule, transform: `translate(${r(cx - (vw * k) / 2)} ${r(cy - (vh * k) / 2)}) scale(${r(k * 1000) / 1000})` });
  }
  if (key === 'unattributed' || key === 'other') return '';
  const initials = key.replace(/[^a-z0-9]/gi, ' ').trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase() || '?';
  const fs = size * (initials.length > 1 ? 0.5 : 0.66);
  return el('text', { x: cx, y: cy + fs * 0.36, fontFamily: font.family, fontWeight: font.weight ?? 700, fontSize: fs, textAnchor: 'middle', fill }, esc(initials));
}

// ───────────────────────── 默认实现 ─────────────────────────

export type StyleDef = Omit<Style, 'ground' | 'plate' | 'title' | 'label' | 'num' | 'bar' | 'track' | 'mark' | 'rule' | 'hatch' | 'color' | 'on' | 'prefer' | 'labelUpper' | 'labelTrack' | 'radius' | 'reserve'>
  & Partial<Pick<Style, 'ground' | 'plate' | 'title' | 'label' | 'num' | 'bar' | 'track' | 'mark' | 'rule' | 'hatch' | 'color' | 'on' | 'prefer' | 'labelUpper' | 'labelTrack' | 'radius' | 'reserve'>>;

export function defineStyle(d: StyleDef): Style {
  const st: Style = {
    labelUpper: true, labelTrack: 0.14, radius: 0, prefer: {}, reserve: { top: 0, bottom: 0 },
    ground(s, b) { s.bg = st.c.bg; s.add(rect(b.x, b.y, b.w, b.h, { fill: st.c.bg })); },
    plate(_s, b) { return b; },
    title(s, text, x, y, size, anchor = 'start', o = {}) {
      return s.text(text, x, y, { font: st.f.display, size, fill: o.fill ?? st.c.ink, anchor, maxW: o.maxW });
    },
    label(s, text, x, y, o = {}) {
      return s.text(text, x, y, { font: st.f.label, size: o.size ?? 13, fill: o.fill ?? st.c.soft, anchor: o.anchor, track: st.labelTrack, upper: st.labelUpper, maxW: o.maxW });
    },
    num(s, text, x, y, size, anchor = 'end', o = {}) {
      return s.text(text, x, y, { font: st.f.num, size, fill: o.fill ?? st.c.ink, anchor });
    },
    bar(s, x, y, w, h, color) { s.add(rect(x, y, w, h, { fill: color, rx: Math.min(st.radius, h / 2, w / 2) })); },
    track(s, x, y, w, h) { s.add(rect(x, y, w, h, { fill: st.c.line, rx: Math.min(st.radius, h / 2) })); },
    mark(s, key, cx, cy, size, color) {
      if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.32, { fill: 'none', stroke: st.c.faint, strokeWidth: Math.max(1.2, size / 12), strokeDasharray: `${r(size / 8)} ${r(size / 10)}` })); return; }
      s.add(logo(key, cx, cy, size, color ?? st.c.ink, st.f.display));
    },
    rule(s, x, y, w) { s.add(line(x, y, x + w, y, { stroke: st.c.line, strokeWidth: 1 })); },
    hatch(s) {
      const a = st.c.faint, b = st.c.paper;
      return s.url('hatch', (id) => el('pattern', { id, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, rect(0, 0, 6, 6, { fill: b }), rect(0, 0, 2.4, 6, { fill: a })));
    },
    color(key, rank) {
      if (key === 'unattributed') return st.c.faint;
      if (key === 'other') return mix(st.c.faint, st.c.paper, 0.3);
      if (key === 'human') return st.c.accent2;
      return st.series[Math.min(rank, st.series.length - 1)] ?? st.c.ink;
    },
    on(color) { return lum(color) > 0.55 ? '#111111' : '#FFFFFF'; },
    ...d,
  } as Style;
  // 用户自定义的钩子里可能用到 st（闭包），所以最后再挂一次
  return Object.assign(st, d);
}

// ───────────────────────── 共用小工具 ─────────────────────────

/** 纸纹：低对比的噪点，叠在底色上 */
export function grain(s: Scene, b: Box, o: { color?: string; opacity?: number; freq?: number; seed?: number } = {}) {
  const id = s.def(`grain${o.seed ?? 1}`, (id) => el('filter', { id, x: 0, y: 0, width: '100%', height: '100%', filterUnits: 'objectBoundingBox' },
    el('feTurbulence', { type: 'fractalNoise', baseFrequency: o.freq ?? 0.9, numOctaves: 2, seed: o.seed ?? 1, stitchTiles: 'stitch' }),
    el('feColorMatrix', { type: 'matrix', values: `0 0 0 0 ${hexC(o.color ?? '#000000', 0)} 0 0 0 0 ${hexC(o.color ?? '#000000', 1)} 0 0 0 0 ${hexC(o.color ?? '#000000', 2)} 0 0 0 -1.4 1.1` })));
  s.add(rect(b.x, b.y, b.w, b.h, { filter: `url(#${id})`, opacity: o.opacity ?? 0.18, fill: '#000' }));
}
const hexC = (hex: string, i: number) => r(parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255);

/** 粗糙边缘：笔刷、木刻、印章的边 */
export function roughen(s: Scene, name: string, scale = 3, freq = 0.08, seed = 2): string {
  return s.url(`rough-${name}`, (id) => el('filter', { id, x: '-5%', y: '-20%', width: '110%', height: '140%' },
    el('feTurbulence', { type: 'fractalNoise', baseFrequency: freq, numOctaves: 3, seed, result: 'n' }),
    el('feDisplacementMap', { in: 'SourceGraphic', in2: 'n', scale, xChannelSelector: 'R', yChannelSelector: 'G' })));
}
/** 柔化：薄雾、远山、光晕 */
export function blur(s: Scene, sd: number): string {
  // 区域用画布本身（用户坐标）：超大的对象包围盒会让 resvg 崩溃，画布外的部分反正也看不见
  return s.url(`blur${sd}`, (id) => el('filter', { id, filterUnits: 'userSpaceOnUse', x: 0, y: 0, width: s.w, height: s.h }, el('feGaussianBlur', { stdDeviation: sd })));
}
export function linear(s: Scene, name: string, stops: [number, string, number?][], dir: [number, number, number, number] = [0, 0, 0, 1]): string {
  return s.url(name, (id) => el('linearGradient', { id, x1: dir[0], y1: dir[1], x2: dir[2], y2: dir[3] },
    ...stops.map(([o, c, a]) => el('stop', { offset: o, stopColor: c, stopOpacity: a ?? 1 }))));
}
export function radial(s: Scene, name: string, stops: [number, string, number?][], at: Attrs = {}): string {
  return s.url(name, (id) => el('radialGradient', { id, ...at }, ...stops.map(([o, c, a]) => el('stop', { offset: o, stopColor: c, stopOpacity: a ?? 1 }))));
}

/** 倒角多边形（赛博面板） */
export function chamfer(x: number, y: number, w: number, h: number, c: number, corners = [true, false, true, false]): string {
  const [tl, tr, br, bl] = corners.map((k) => (k ? c : 0));
  return `M${r(x + tl)} ${r(y)}H${r(x + w - tr)}L${r(x + w)} ${r(y + tr)}V${r(y + h - br)}L${r(x + w - br)} ${r(y + h)}H${r(x + bl)}L${r(x)} ${r(y + h - bl)}V${r(y + tl)}Z`;
}

export { Scene, el, g, rect, path, circle, line, r, esc, measure, mix, alpha, lum };
export type { FontSpec };
