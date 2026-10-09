// Satori 对象树的最小工具和共用部件
import type { Theme } from './themes.ts';
import { colorOf, logoUri } from './brand.ts';
import type { Contributor } from './data.ts';

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
export const svgImage = (svg: string, w: number, hh: number, style: Record<string, any> = {}) =>
  image('data:image/svg+xml;base64,' + Buffer.from(svg).toString('base64'), w, hh, style);

export interface Out { tree: Node; w: number; h: number }

/** 小标签：字距拉开，颜色退后 */
/** Satori 的 CSS 解析器不接受浮点误差产生的长小数（如 3.5200000000000005），数值统一保留两位 */
const r2 = (x: number) => Math.round(x * 100) / 100;
export const label = (t: Theme, s: string, size = 13, style: Record<string, any> = {}) =>
  txt({ fontFamily: t.labelFont, fontSize: size, letterSpacing: r2(size * t.labelTrack), color: t.soft, textTransform: 'uppercase', ...style }, s);

export const stripes = (t: Theme, w: number) =>
  `repeating-linear-gradient(45deg, ${t.stripeA} 0px, ${t.stripeA} ${w}px, ${t.stripeB} ${w}px, ${t.stripeB} ${w * 2}px)`;

/** 贡献方的填充：品牌色；未确认用斜纹 */
export function fill(c: Contributor | { key: string }, t: Theme, rank = 0, stripeW = 3): Record<string, any> {
  if (c.key === 'unattributed') return { backgroundImage: stripes(t, stripeW) };
  return { backgroundColor: colorOf(c.key, t, rank) };
}

/** logo；未确认是虚线圆；没有 logo 的人或工具不画占位，只留空位对齐 */
export function mark(key: string, t: Theme, size: number, color?: string): Node {
  if (key === 'unattributed')
    return box({ width: size, height: size, alignItems: 'center', justifyContent: 'center', flexShrink: 0 },
      box({ width: size * 0.62, height: size * 0.62, borderRadius: 99, border: `${Math.max(1.25, size / 11)}px dashed ${t.faint}` }));
  const uri = logoUri(key, color ?? t.ink);
  if (uri) return image(uri, size, size, { flexShrink: 0 });
  return box({ width: size, height: size, flexShrink: 0 });
}

export function bar(t: Theme, c: Contributor, rank: number, height = t.barH, minW = 3): Node {
  return box({ height, borderRadius: t.barRadius, backgroundColor: t.track, width: '100%' },
    box({ width: `${Math.max(c.pct, 0)}%`, minWidth: minW, height: '100%', borderRadius: t.barRadius, ...fill(c, t, rank, Math.max(2, height / 2)) }));
}

/** 背景：Glass 风格在底色上叠几团柔光，颜色取前两名贡献方 */
export function backdrop(t: Theme, keys: string[], style: Record<string, any> = {}): Record<string, any> {
  if (!t.glow) return { backgroundColor: t.bg, ...style };
  const c = keys.map((k) => colorOf(k, t)).filter(Boolean);
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
  border: t.id === 'editorial' || t.id === 'mono' ? `2px solid ${t.edge}` : `1px solid ${t.edge}`,
  ...style,
});
