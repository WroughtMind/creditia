// Satori 对象树的最小工具和共用部件。这里不碰文件系统，Node 和浏览器端都能用。
import type { Theme } from './themes.ts';
import { BRANDS, famColor, logoUri } from './brand.ts';
import { NAMES, type Contributor } from './data.ts';

export type Node = any;
export function h(type: string, style: Record<string, any> = {}, ...kids: any[]): Node {
  const children = kids.flat(Infinity).filter((k) => k !== null && k !== undefined && k !== false && k !== '');
  // 数值样式统一保留两位小数，避免浮点误差产生 Satori 解析不了的长小数；undefined / null 的键直接丢掉
  const s: Record<string, any> = { display: 'flex' };
  for (const [k, v] of Object.entries(style)) {
    if (v === undefined || v === null) continue;
    s[k] = typeof v === 'number' ? Math.round(v * 100) / 100 : v;
  }
  return { type, props: { style: s, children: children.length === 1 ? children[0] : children } };
}
export const box = (style: Record<string, any>, ...kids: any[]) => h('div', style, ...kids);
export const col = (style: Record<string, any>, ...kids: any[]) => h('div', { flexDirection: 'column', ...style }, ...kids);
export const txt = (style: Record<string, any>, s: string | number) => h('div', style, String(s));
export const image = (src: string, w: number, hh: number, style: Record<string, any> = {}): Node => ({ type: 'img', props: { src, width: w, height: hh, style } });
/** base64 的 data URI；Node 和浏览器端都能用 */
export function dataUri(svg: string): string {
  if (typeof Buffer !== 'undefined') return 'data:image/svg+xml;base64,' + Buffer.from(svg).toString('base64');
  const bytes = new TextEncoder().encode(svg);
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return 'data:image/svg+xml;base64,' + btoa(bin);
}
export const svgImage = (svg: string, w: number, hh: number, style: Record<string, any> = {}) => image(dataUri(svg), w, hh, style);

/** 画布四周留白（放大内容时用）。没有就不放大 */
export interface Out { tree: Node; w: number; h: number; fit?: { padX: number; padY: number } }

/** 内容少时整体放大填满画面：出图时先量一遍这个节点，再按画布留白之内的空间等比放大，最多 max 倍。
 *  版式、字号之间的比例都不变，只是不再缩在画面中间一小块 */
export const FIT_KEY = 'creditia-fit';
/** 同一组的节点出图时量出各自的自然宽度，再统一设成最宽的那个（两栏对齐、中缝居中用）。不靠估算字宽 */
let eqSeq = 0;
export function eq(group: string, n: Node): Node { n.key = `creditia-eq:${group}:${eqSeq++}`; n.eqGroup = group; return n; }
/** 放大到宽度到顶以后，竖向还空着的部分分给这个节点里各项之间的间距（最多加到 spread 倍）。
 *  片尾字幕本来就是行距很宽的长名单，这样名单会铺开，不会挤在画面中间 */
export function spread(times: number, n: Node): Node { n.fitSpread = times; return n; }
export function fit(max: number, style: Record<string, any>, ...kids: any[]): Node {
  const n = col(style, ...kids);
  n.key = FIT_KEY;
  n.fitMax = max;
  return n;
}

/** Satori 的 CSS 解析器不接受浮点误差产生的长小数，数值统一保留两位 */
const r2 = (x: number) => Math.round(x * 100) / 100;
/** 小标签：大写、字距拉开、颜色退后 */
export const label = (t: Theme, s: string, size = 13, style: Record<string, any> = {}) =>
  txt({ fontFamily: t.labelFont, fontSize: size, letterSpacing: r2(size * t.labelTrack), color: t.soft, textTransform: 'uppercase', ...style }, s);

export const stripes = (t: Theme, w: number) =>
  `repeating-linear-gradient(45deg, ${t.stripeA} 0px, ${t.stripeA} ${w}px, ${t.stripeB} ${w}px, ${t.stripeB} ${w * 2}px)`;

/** 贡献方的填充：按家族给色；未确认用斜纹 */
export function fill(c: Contributor | { key: string }, t: Theme, rank = 0, stripeW = 3): Record<string, any> {
  if (c.key === 'unattributed') return { backgroundImage: stripes(t, stripeW) };
  return { backgroundColor: famColor(c.key, t, rank) };
}

/** logo；未确认是虚线圆；没有 logo 的人或工具不画占位，只留空位对齐 */
export function mark(key: string, t: Theme, size: number, color?: string): Node {
  if (key === 'unattributed')
    return box({ width: size, height: size, alignItems: 'center', justifyContent: 'center', flexShrink: 0 },
      box({ width: size * 0.62, height: size * 0.62, borderRadius: 99, border: `${Math.max(1.25, size / 11)}px dashed ${t.faint}` }));
  const uri = logoUri(key, color ?? t.ink);
  if (uri) return image(uri, size, size, { flexShrink: 0 });
  // 没有 logo 的工具写首字母，人、「其他」只留空位对齐
  const tool = key.split('/')[0];
  if (!BRANDS[tool]) return box({ width: size, height: size, flexShrink: 0 });
  const initials = (NAMES[tool] ?? tool).split(/[\s-]+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase();
  return box({ width: size, height: size, flexShrink: 0, alignItems: 'center', justifyContent: 'center', borderRadius: 99, border: `${Math.max(1, size / 16)}px solid ${color ?? t.ink}` },
    txt({ fontFamily: 'Inter, Noto Sans SC', fontWeight: 700, fontSize: size * (initials.length > 1 ? 0.36 : 0.48), color: color ?? t.ink, lineHeight: 1 }, initials));
}

const alpha = (hex: string, a: string) => (/^#[0-9a-fA-F]{6}$/.test(hex) ? hex + a : hex);

/** 装饰分隔：每个体裁用自己熟悉的符号（胶片菱形、剧院星、报纸花叶、终端等号线…） */
export function ornament(t: Theme, size = 14, style: Record<string, any> = {}): Node {
  const ch: Record<string, string> = {
    cinema: '◆', playbill: '★', gala: '◆', broadsheet: '§', crt: '═', arcade: '▲', neon: '★', poster: '■', film: '·', editorial: '▪',
  };
  const n = t.family === 'poster' || t.family === 'crt' ? 8 : 5;
  return txt({ fontFamily: t.deco ?? t.display, fontSize: size, color: t.accent, letterSpacing: size * 0.85, ...style }, Array(n).fill(ch[t.family] ?? '·').join(''));
}

/** 底板的边距与描边总宽，算高度时要用 */
export function plateMetrics(t: Theme): { pad: number; border: number } {
  switch (t.frame) {
    case 'double': return { pad: 35, border: 4 };
    case 'ornate': return { pad: 33, border: 10 };
    case 'rule': return { pad: 18, border: 10 };
    case 'thick': return { pad: 28, border: 14 };
    case 'pixel': return { pad: 24, border: 12 };
    default: return { pad: 28, border: 0 };
  }
}

/** 底板：每个体裁的框线语汇不同（金框双线、剧院花框、报刊上下粗线、海报粗黑框、像素框） */
export function plate(t: Theme, style: Record<string, any>, kids: any[], inner: Record<string, any> = {}): Node {
  const pat = t.pattern ? { backgroundImage: t.pattern } : {};
  const base = { boxSizing: 'border-box' as const, backgroundColor: t.surface, ...pat, ...style };
  const wrap = (extra: Record<string, any>) => col({ ...extra, width: '100%', flexGrow: 1, ...inner }, ...kids);
  switch (t.frame) {
    case 'double': return col({ ...base, border: `1px solid ${t.accent}`, padding: 9 }, wrap({ border: `1px solid ${t.edge}`, padding: 26 }));
    case 'ornate': return col({ ...base, border: `2px solid ${t.edge}`, padding: 7 }, wrap({ border: `3px solid ${t.accent}`, padding: 26 }));
    case 'rule': return col({ ...base, borderTop: `5px solid ${t.ink}`, borderBottom: `5px solid ${t.ink}`, padding: '18px 0' }, wrap({ padding: '0 36px' }));
    case 'thick': return col({ ...base, border: `7px solid ${t.ink}` }, wrap({ padding: 28 }));
    case 'pixel': return col({ ...base, border: `4px solid ${t.edge}`, padding: 4 }, wrap({ border: `2px solid ${alpha(t.accent, '66')}`, padding: 20 }));
    default: return col({ ...base, padding: 28 }, wrap({}));
  }
}

export function bar(t: Theme, c: Contributor, rank: number, height = t.barH, minW = 3): Node {
  return box({ height, borderRadius: t.barRadius, backgroundColor: t.track, width: '100%' },
    box({ width: `${Math.max(c.pct, 0)}%`, minWidth: minW, height: '100%', borderRadius: t.barRadius, ...fill(c, t, rank, Math.max(2, height / 2)) }));
}

/** 背景：Glass 风格在底色上叠几团柔光，颜色取前两名贡献方 */
export function backdrop(t: Theme, keys: string[], style: Record<string, any> = {}): Record<string, any> {
  if (!t.glow) return { backgroundColor: t.bg, ...style };
  const c = keys.map((k) => famColor(k, t)).filter(Boolean);
  const a = c[0] ?? '#8EA2FF', b = c[1] ?? '#FFB38E';
  return {
    backgroundColor: t.bg,
    backgroundImage: `radial-gradient(circle at 18% 22%, ${a}55 0%, ${a}00 42%), radial-gradient(circle at 82% 78%, ${b}50 0%, ${b}00 46%), radial-gradient(circle at 70% 18%, #FFFFFF 0%, #FFFFFF00 40%)`,
    ...style,
  };
}

export const surface = (t: Theme, style: Record<string, any> = {}): Record<string, any> => ({
  backgroundColor: t.surface,
  borderRadius: t.radius,
  boxShadow: t.shadow === 'none' ? undefined : t.shadow,
  border: `${t.family === 'editorial' || t.family === 'mono' ? 2 : 1}px solid ${t.edge}`,
  ...style,
});

/** 终端家族的方块条：■ 填充、· 剩余 */
export const MONO_FONT = 'JetBrains Mono, Noto Sans SC';
export function blocks(c: Contributor, t: Theme, rank: number, n = 10, size = 15): Node {
  const k = Math.max(c.pct >= 0.05 ? 1 : 0, Math.round((c.pct / 100) * n));
  const color = c.key === 'unattributed' ? t.faint : famColor(c.key, t, rank);
  return box({ alignItems: 'center', flexShrink: 0 },
    txt({ fontFamily: MONO_FONT, fontSize: size, color, whiteSpace: 'pre', lineHeight: 1 }, '■'.repeat(k)),
    txt({ fontFamily: MONO_FONT, fontSize: size, color: t.faint, whiteSpace: 'pre', lineHeight: 1 }, '·'.repeat(Math.max(n - k, 0))));
}
