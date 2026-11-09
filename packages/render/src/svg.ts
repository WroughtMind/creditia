// SVG 场景：所有作品都是手写 SVG。一张作品 = 一个 Scene：画面（字符串）+ 可交互区域（hits）。
// 文字用 <text>，字宽按 metrics.json 估算；导出时由 resvg 转成路径，所以任何环境下看到的都一样。
import METRICS from './metrics.json' with { type: 'json' };
import { familiesIn, FONTS } from './fonts.ts';

type Metric = { w: Record<string, number>; cjk: number | null; cap: number; asc: number; desc: number };
const M = METRICS as Record<string, Metric>;

/** 两位小数，避免浮点长尾 */
export const r = (n: number) => Math.round(n * 100) / 100;
export const esc = (s: unknown) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

// SVG 里本来就是驼峰的属性，原样保留；其余驼峰键转成连字符
const CAMEL = new Set(['viewBox', 'preserveAspectRatio', 'gradientUnits', 'gradientTransform', 'patternUnits', 'patternContentUnits', 'patternTransform',
  'stdDeviation', 'baseFrequency', 'numOctaves', 'stitchTiles', 'xChannelSelector', 'yChannelSelector', 'tableValues', 'filterUnits', 'primitiveUnits',
  'maskUnits', 'maskContentUnits', 'clipPathUnits', 'textLength', 'lengthAdjust', 'surfaceScale', 'specularConstant', 'specularExponent',
  'diffuseConstant', 'kernelMatrix', 'limitingConeAngle', 'pointsAtX', 'pointsAtY', 'pointsAtZ', 'markerWidth', 'markerHeight', 'refX', 'refY', 'startOffset']);
const kebab = (k: string) => (CAMEL.has(k) ? k : k.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase()));

export type Attrs = Record<string, string | number | boolean | null | undefined>;
export function attrs(a: Attrs = {}): string {
  let s = '';
  for (const [k, v] of Object.entries(a)) {
    if (v === null || v === undefined || v === false) continue;
    // 字体名中的数字起始词（如 Press Start 2P）不是合法 CSS 标识符。
    // resvg 宽容接受未引号的名称，浏览器却会丢弃整个列表并回退到界面字体。
    const value = typeof v === 'string' && (k === 'fontFamily' || k === 'font-family')
      ? v.split(',').map(part => /(?:^|\s)\d/.test(part.trim()) && !/^\s*["']/.test(part)
        ? part.replace(/^(\s*)(.*?)(\s*)$/, (_all, lead, name, tail) => `${lead}"${name.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"${tail}`)
        : part).join(',')
      : v;
    s += ` ${kebab(k)}="${typeof value === 'number' ? r(value) : esc(value === true ? '' : value)}"`;
  }
  return s;
}
export function el(tag: string, a: Attrs = {}, ...kids: (string | null | undefined | false)[]): string {
  const inner = kids.filter(Boolean).join('');
  return inner ? `<${tag}${attrs(a)}>${inner}</${tag}>` : `<${tag}${attrs(a)}/>`;
}
export const g = (a: Attrs, ...kids: (string | null | undefined | false)[]) => el('g', a, ...kids);

// ───────────────────────── 字体与度量 ─────────────────────────

export interface FontSpec { family: string; weight?: number; italic?: boolean }

const CJK = /[⺀-⿿　-〿぀-ヿ㄀-ㇿ㐀-䶿一-鿿가-힯豈-﫿︰-﹏＀-￯]/;
export const isCjk = (ch: string) => CJK.test(ch);

/** 找度量：按 font-family 顺序取第一个有度量的字体，字重取最接近的 */
function metricOf(f: FontSpec): Metric | null {
  for (const fam of familiesIn(f.family)) {
    const keys = Object.keys(M).filter((k) => k.split('|')[0] === fam);
    if (!keys.length) continue;
    const want = f.weight ?? 400;
    const pick = (italic: boolean) => keys.filter((k) => k.endsWith('i') === italic).sort((a, b) => Math.abs(parseInt(a.split('|')[1]) - want) - Math.abs(parseInt(b.split('|')[1]) - want))[0];
    return M[pick(!!f.italic) ?? pick(!f.italic)];
  }
  return null;
}

/** 字符宽（1/1000 em）。拉丁字体里没有的 CJK 字符交给后备字体，按 1em 计 */
function adv(m: Metric | null, ch: string): number {
  if (m?.w[ch] !== undefined) return m.w[ch];
  if (isCjk(ch)) return m?.cjk ?? 1000;
  return 560;
}

/** 文字宽度（px）。track 是字距，单位 em */
export function measure(s: string, f: FontSpec, size: number, track = 0): number {
  const m = metricOf(f);
  const chars = [...s];
  let w = 0;
  for (const ch of chars) w += adv(m, ch);
  return (w / 1000) * size + Math.max(0, chars.length - 1) * track * size;
}
/** 大写字高（px），用于垂直居中 */
export const capOf = (f: FontSpec, size: number) => ((metricOf(f)?.cap ?? 700) / 1000) * size;

/** 截断到 maxW，末尾加省略号 */
export function clip(s: string, f: FontSpec, size: number, maxW: number, track = 0): string {
  if (measure(s, f, size, track) <= maxW) return s;
  const chars = [...s];
  while (chars.length > 1 && measure(chars.join('') + '…', f, size, track) > maxW) chars.pop();
  return chars.join('').trimEnd() + '…';
}
/** 缩到放得下：字号最低降到 min 倍 */
export function fitSize(s: string, f: FontSpec, size: number, maxW: number, track = 0, min = 0.55): number {
  const w = measure(s, f, size, track);
  return w <= maxW ? size : Math.max(size * min, (size * maxW) / w);
}
/** 按宽度折行（CJK 逐字可断，拉丁按词断） */
export function wrap(s: string, f: FontSpec, size: number, maxW: number, track = 0): string[] {
  const tokens = s.match(/[⺀-鿿豈-﫿＀-￯]|[^\s⺀-鿿豈-﫿＀-￯]+|\s+/g) ?? [];
  const lines: string[] = [];
  let cur = '';
  for (const t of tokens) {
    const next = cur + t;
    if (cur && measure(next.trimEnd(), f, size, track) > maxW && !/^\s+$/.test(t)) { lines.push(cur.trimEnd()); cur = t.trimStart(); }
    else cur = next;
  }
  if (cur.trim()) lines.push(cur.trimEnd());
  return lines;
}

/** 中西文混排：resvg 给一个文字块挑同一个字体，第一个字体（拉丁字体）缺中文时整块都落到中文字体上，
 *  西文就不再是这个风格的字，宽度也和度量对不上。拆成几段：每段从量好的位置另起一块（tspan 带 x），
 *  中文段只写中文字体，西文段用原字体。返回 null 表示不用拆 */
const CJK_FAMILIES = new Set(FONTS.filter((f) => f.cjk).map((f) => f.family));
function scriptRuns(str: string, f: FontSpec, size: number, track: number, x0: number): string | null {
  const fams = familiesIn(f.family);
  if (fams.length < 2 || CJK_FAMILIES.has(fams[0]) || !CJK.test(str) || ![...str].some((ch) => !isCjk(ch) && ch.trim())) return null;
  const cjk = fams.filter((x) => CJK_FAMILIES.has(x)).join(', ') || 'Noto Sans SC';
  const runs = str.match(new RegExp(`${CJK.source}+|(?:(?!${CJK.source}).)+`, 'gu')) ?? [str];
  let x = x0;
  return runs.map((run) => {
    const at = r(x);
    // SVG 默认把连续空白并成一个，量宽也按并过的算，否则下一段会往右多空出一截
    x += measure(run.replace(/\s+/g, ' '), f, size, track) + track * size;
    return el('tspan', { x: at, fontFamily: CJK.test(run) ? cjk : null }, esc(run));
  }).join('');
}

// ───────────────────────── 场景 ─────────────────────────

/** 悬停提示：k 是贡献方 key，同 key 的区域一起高亮 */
export interface Tip { k?: string; t: string; d?: string[] }
export interface Hit extends Tip { x: number; y: number; w: number; h: number }

/** 已完成缩放、截断及对齐的一行文字度量；仅在调用方临时捕获时保留。 */
export interface TextBounds {
  x: number; y: number; w: number; h: number; baseline: number; size: number; rotate?: number;
}

export interface TextOpts {
  font: FontSpec;
  size: number;
  fill?: string;
  anchor?: 'start' | 'middle' | 'end';
  /** 字距（em） */
  track?: number;
  upper?: boolean;
  opacity?: number;
  /** 超宽时：shrink 缩字号（默认），clip 截断 */
  maxW?: number;
  overflow?: 'shrink' | 'clip';
  /** y 的含义：baseline（默认）、middle（大写字高居中）、top */
  base?: 'baseline' | 'middle' | 'top';
  stroke?: string;
  strokeW?: number;
  rotate?: number;
  extra?: Attrs;
}

let seq = 0;
export class Scene {
  readonly id: string;
  private defs = new Map<string, string>();
  private out: string[] = [];
  private textCollectors: TextBounds[][] = [];
  readonly hits: Hit[] = [];
  /** 背景色（导出 PDF / 尺寸画布时用来铺底） */
  bg = '#ffffff';
  w: number;
  h: number;
  constructor(w: number, h: number) {
    this.w = Math.round(w);
    this.h = Math.round(h);
    this.id = 'c' + (seq++).toString(36);
  }
  /** 定义一次，返回 id；同名只保留第一个 */
  def(name: string, make: (id: string) => string): string {
    const id = `${this.id}-${name}`;
    if (!this.defs.has(id)) this.defs.set(id, make(id));
    return id;
  }
  url(name: string, make: (id: string) => string) { return `url(#${this.def(name, make)})`; }
  add(...parts: (string | null | undefined | false)[]) { for (const p of parts) if (p) this.out.push(p); return this; }
  /** 作者声明的完整绘制组；保留原输出，只另外记录其已有画面，不拆字或扫描叶子。 */
  capturePaint(draw: () => void): string {
    const start = this.out.length;
    draw();
    return this.out.slice(start).join('');
  }
  hit(x: number, y: number, w: number, h: number, tip: Tip) {
    if (w > 0 && h > 0) this.hits.push({ x: r(x), y: r(y), w: r(w), h: r(h), ...tip });
    return this;
  }
  /** 捕获本次绘制的真实字面，嵌套捕获各自完整；弧形 textPath 不会误作横行。 */
  captureTextBounds(draw: () => void): TextBounds[] {
    const found: TextBounds[] = [];
    this.textCollectors.push(found);
    try { draw(); } finally { this.textCollectors.pop(); }
    return found;
  }
  /** 写一行字，返回实际宽度 */
  text(s: string | number, x: number, y: number, o: TextOpts): number {
    let str = String(s);
    if (o.upper) str = str.toUpperCase();
    let size = o.size;
    const track = o.track ?? 0;
    if (o.maxW) {
      if (o.overflow === 'clip') str = clip(str, o.font, size, o.maxW, track);
      else size = fitSize(str, o.font, size, o.maxW, track);
    }
    const w = measure(str, o.font, size, track);
    let yy = y;
    if (o.base === 'middle') yy = y + capOf(o.font, size) / 2;
    else if (o.base === 'top') yy = y + capOf(o.font, size);
    // 混排拆段时各段的位置是量出来的，统一从左边排起
    const x0 = o.anchor === 'middle' ? x - w / 2 : o.anchor === 'end' ? x - w : x;
    if (str.trim() && this.textCollectors.length) {
      const h = capOf(o.font, size);
      const bounds: TextBounds = { x: x0, y: yy - h, w, h, baseline: yy, size, ...(o.rotate ? { rotate: o.rotate } : {}) };
      for (const collector of this.textCollectors) collector.push(bounds);
    }
    const runs = scriptRuns(str, o.font, size, track, x0);
    // letter-spacing 会在末字后面也加一份间距；居中、右对齐时补偿回来
    const dx = track && o.anchor === 'middle' ? (track * size) / 2 : track && o.anchor === 'end' ? track * size : 0;
    this.out.push(el('text', {
      x: runs ? r(x0) : x + dx, y: yy, fontFamily: o.font.family, fontSize: size, fontWeight: o.font.weight ?? 400, fontStyle: o.font.italic ? 'italic' : null,
      fill: o.fill ?? '#000', textAnchor: !runs && o.anchor && o.anchor !== 'start' ? o.anchor : null, letterSpacing: track ? r(track * size) : null,
      opacity: o.opacity, stroke: o.stroke, strokeWidth: o.strokeW, paintOrder: o.stroke ? 'stroke' : null, strokeLinejoin: o.stroke ? 'round' : null,
      transform: o.rotate ? `rotate(${o.rotate} ${r(x)} ${r(y)})` : null, ...(o.extra ?? {}),
    }, runs ?? esc(str)));
    return w;
  }
  /** 竖排：汉字直立，连续的拉丁字母、数字转 90°。(x, y) 是第一个字的顶部中心。返回总高度 */
  vtext(s: string, x: number, y: number, o: Omit<TextOpts, 'anchor' | 'base' | 'rotate'> & { gap?: number }): number {
    const size = o.size;
    const step = size * (1 + (o.gap ?? 0.08));
    const runs = s.match(/[⺀-鿿豈-﫿＀-￯　-〿]|[^⺀-鿿豈-﫿＀-￯　-〿]+/g) ?? [];
    let yy = y;
    for (const run of runs) {
      if (isCjk(run)) {
        // 竖排标点：句读放右上
        this.text(run, x, yy + size * 0.88, { ...o, anchor: 'middle' });
        yy += step;
      } else {
        const t = run.trim();
        if (!t) { yy += size * 0.4; continue; }
        const len = measure(t, o.font, size * 0.92, o.track ?? 0);
        this.text(t, x - size * 0.3, yy + size * 0.06, { ...o, size: size * 0.92, anchor: 'start', rotate: 90 });
        yy += len + size * 0.3;
      }
    }
    return yy - y;
  }
  toString(): string {
    return `<svg xmlns="http://www.w3.org/2000/svg" width="${r(this.w)}" height="${r(this.h)}" viewBox="0 0 ${r(this.w)} ${r(this.h)}">`
      + (this.defs.size ? `<defs>${[...this.defs.values()].join('')}</defs>` : '')
      + this.out.join('') + '</svg>';
  }
}

// ───────────────────────── 常用图形 ─────────────────────────

export const rect = (x: number, y: number, w: number, h: number, a: Attrs = {}) => el('rect', { x, y, width: Math.max(0, w), height: Math.max(0, h), ...a });
export const circle = (cx: number, cy: number, rr: number, a: Attrs = {}) => el('circle', { cx, cy, r: rr, ...a });
export const line = (x1: number, y1: number, x2: number, y2: number, a: Attrs = {}) => el('line', { x1, y1, x2, y2, ...a });
export const path = (d: string, a: Attrs = {}) => el('path', { d, ...a });

/** 圆环扇形路径 */
export function arc(cx: number, cy: number, r1: number, r0: number, a0: number, a1: number): string {
  const p = (rr: number, a: number) => `${r(cx + rr * Math.cos(a))} ${r(cy + rr * Math.sin(a))}`;
  const big = a1 - a0 > Math.PI ? 1 : 0;
  if (a1 - a0 >= Math.PI * 2 - 1e-6) {
    return `M${p(r1, 0)}A${r1} ${r1} 0 1 1 ${p(r1, Math.PI)}A${r1} ${r1} 0 1 1 ${p(r1, 0)}Z` + (r0 > 0 ? `M${p(r0, 0)}A${r0} ${r0} 0 1 0 ${p(r0, Math.PI)}A${r0} ${r0} 0 1 0 ${p(r0, 0)}Z` : '');
  }
  return r0 > 0
    ? `M${p(r1, a0)}A${r1} ${r1} 0 ${big} 1 ${p(r1, a1)}L${p(r0, a1)}A${r0} ${r0} 0 ${big} 0 ${p(r0, a0)}Z`
    : `M${cx} ${cy}L${p(r1, a0)}A${r1} ${r1} 0 ${big} 1 ${p(r1, a1)}Z`;
}

/** 可复现的伪随机：同样的种子画出同样的纹理 */
export function rng(seed: number) {
  let s = (seed % 2147483647) || 1;
  return () => (s = (s * 16807) % 2147483647) / 2147483647;
}
export function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return Math.abs(h);
}

/** 颜色：hex 混合、加透明度 */
export function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  if (!/^#[0-9a-f]{6}$/i.test(a) || !/^#[0-9a-f]{6}$/i.test(b)) return a;
  const [x, y] = [p(a), p(b)];
  return '#' + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, '0')).join('');
}
export const alpha = (hex: string, a: number) => (/^#[0-9a-f]{6}$/i.test(hex) ? hex + Math.round(a * 255).toString(16).padStart(2, '0') : hex);
export function lum(hex: string): number {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) return 0.5;
  const [r1, g1, b1] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  return 0.2126 * r1 + 0.7152 * g1 + 0.0722 * b1;
}

/** 动画缓动：t∈[0,1] 全局进度，按序号错开 */
export function ease(t: number, i = 0, n = 1, span = 0.6): number {
  if (t >= 1) return 1;
  const start = n > 1 ? (i / (n - 1)) * (1 - span) : 0;
  const k = Math.min(1, Math.max(0, (t - start) / span));
  return 1 - Math.pow(1 - k, 3);
}
