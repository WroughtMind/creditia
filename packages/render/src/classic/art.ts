// 原版作品 → 统一的 Art（App、官网、命令行导出都吃这个）。satori 由调用方传进来：Node 用完整版，浏览器用 standalone。
import { FORMS, type Ctx } from './registry.ts';
import { badge } from './forms.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Art } from '../export.ts';
import { pieceName } from './catalog.ts';
import { FIT_KEY } from './ui.ts';
import { enTree, enSvg } from './i18n.ts';
import { BRIDGED, BRIDGED_LAYOUTS, isBridged, bridgeArt, bridgeFrames } from './bridge.ts';
import { reveal, rise, RISING } from './motion.ts';
import { hitsOf, type Seen } from './hits.ts';

export type Satori = (tree: any, o: { width: number; height: number; fonts: any[]; onNodeDetected?: (n: any) => void }) => Promise<string>;

/** 同一页面会内联多张 satori 出的 SVG，它们的 clipPath/mask id 都是 satori_ 开头，会互相顶掉；每张换成独有前缀 */
let seq = 0;
const tidy = (svg: string, w: number, h: number) => svg.replace(/satori_/g, `s${(seq++).toString(36)}_`)
  // satori 给 1px 高的虚线边框算出负高度的遮罩，浏览器会报错；归零
  .replace(/ (width|height)="-[\d.]+"/g, ' $1="0"')
  // 阴影滤镜的区域是元素的好几倍，栅格化时很吃内存；画布外本来看不见，收到画布以内
  .replace(/<filter id="([^"]+)" x="[^"]*" y="[^"]*" width="[^"]*" height="[^"]*"/g, `<filter id="$1" filterUnits="userSpaceOnUse" x="0" y="0" width="${w}" height="${h}"`);

/** 找到要放大的节点 */
function findFit(n: any): any {
  if (!n || typeof n !== 'object') return null;
  if (n.key === FIT_KEY) return n;
  const kids = n.props?.children;
  for (const k of Array.isArray(kids) ? kids : [kids]) { const f = findFit(k); if (f) return f; }
  return null;
}

function findSpread(n: any): any {
  if (!n || typeof n !== 'object') return null;
  if (n.fitSpread) return n;
  const kids = n.props?.children;
  for (const k of Array.isArray(kids) ? kids : [kids]) { const f = findSpread(k); if (f) return f; }
  return null;
}

/** 有 fit 节点时渲染两遍：第一遍量出内容多大，第二遍按留白之内的空间等比放大 */
/** 把标了同一组的节点统一成最宽的那个：先按自然宽度排一遍量出来 */
async function equalize(tree: any, w: number, h: number, satori: Satori, fonts: any[]) {
  const nodes: any[] = [];
  const walk = (n: any) => { if (!n || typeof n !== 'object') return; if (n.eqGroup) nodes.push(n); const k = n.props?.children; for (const c of Array.isArray(k) ? k : [k]) walk(c); };
  walk(tree);
  if (!nodes.length) return;
  const widths = new Map<string, number>();
  await satori(tree, { width: w, height: h, fonts, onNodeDetected: (x) => { if (typeof x.key === 'string' && x.key.startsWith('creditia-eq:')) widths.set(x.key, x.width); } });
  const max = new Map<string, number>();
  for (const n of nodes) max.set(n.eqGroup, Math.max(max.get(n.eqGroup) ?? 0, widths.get(n.key) ?? 0));
  for (const n of nodes) n.props.style = { ...n.props.style, width: Math.ceil(max.get(n.eqGroup)!) + 2 };
}

/** seen：最后出图那一遍里每个节点的位置（悬停细节用） */
async function fitted(out: { tree: any; w: number; h: number; fit?: { padX: number; padY: number } }, satori: Satori, fonts: any[], seen?: Seen[]): Promise<string> {
  await equalize(out.tree, out.w, out.h, satori, fonts);
  const node = out.fit ? findFit(out.tree) : null;
  let box: { width: number; height: number } | null = null;
  const pass: Seen[] = [];
  const done = (svg: string, nodes: Seen[]) => { seen?.push(...nodes); return svg; };
  const first = await satori(out.tree, { width: out.w, height: out.h, fonts, onNodeDetected: node || seen ? (x) => { if (x.key === FIT_KEY) box = x; if (seen) pass.push(x); } : undefined });
  if (!node || !box) return done(first, pass);
  const b = box as { width: number; height: number };
  const k = Math.min((out.w - 2 * out.fit!.padX) / b.width, (out.h - 2 * out.fit!.padY) / b.height, node.fitMax ?? 1.6);
  const kk = Math.max(1, k);
  // 宽度先到顶时，竖向剩下的空间分给标了 spread 的节点里各项之间的间距
  const sp = findSpread(node);
  const kids = sp ? (Array.isArray(sp.props.children) ? sp.props.children.filter(Boolean) : [sp.props.children]) : [];
  const spare = (out.h - 2 * out.fit!.padY) / kk - b.height;
  if (sp && kids.length > 1 && spare > 8) {
    const gap = Number(sp.props.style.gap ?? 0);
    const add = Math.min(spare / (kids.length - 1), Math.max(gap, 24) * (sp.fitSpread - 1));
    sp.props.style = { ...sp.props.style, gap: Math.round(gap + add) };
  } else if (k < 1.04) return done(first, pass);
  if (k >= 1.04) node.props.style = { ...node.props.style, transform: `scale(${Math.round(k * 1000) / 1000})` };
  const last: Seen[] = [];
  return done(await satori(out.tree, { width: out.w, height: out.h, fonts, onNodeDetected: seen ? (x) => last.push(x) : undefined }), last);
}

/** 版式：原版自己的在前，追加的新风格版式在后。原来没有版式的作品，原版画法叫 original */
export const variantsOf = (id: string): string[] => {
  const extra = BRIDGED_LAYOUTS[id] ?? [];
  return [...(FORMS[id]?.variants ?? (extra.length ? ['original'] : [])), ...extra];
};
export const defaultVariant = (id: string, t: Theme): string | undefined => FORMS[id]?.def?.(t) ?? (BRIDGED_LAYOUTS[id] ? 'original' : undefined);

export async function classicArt(id: string, m: Model, t: Theme, ctx: Ctx, satori: Satori, fonts: any[]): Promise<Art | null> {
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang)}`;
  // 原版没有的作品、追加的新风格版式：按这个主题的颜色、字体、框线用新风格的画法画
  if (isBridged(id, ctx.variant)) return bridgeArt(id, m, t, ctx);
  if (id === 'badge') {
    const svg = ctx.lang === 'en' ? enSvg(badge(m, t)) : badge(m, t);
    const w = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 200), h = Number(/height="([\d.]+)"/.exec(svg)?.[1] ?? 28);
    return { w, h, svg, hits: [], bg: 'transparent', title };
  }
  const f = FORMS[id];
  if (!f) throw new Error(`未知作品：${id}`);
  const out = f.render(m, t, ctx);
  if (!out) return null;
  if (ctx.lang === 'en') enTree(out.tree);
  const seen: Seen[] = [];
  const svg = tidy(await fitted(out, satori, fonts, seen), out.w, out.h);
  return { w: out.w, h: out.h, svg, hits: hitsOf(seen, m, t, ctx.lang, out.w, out.h), bg: t.bg, title };
}

/** 会动的作品：徽章是单独的小 SVG，不做动画；其余原版作品都能出帧 */
export const animatedClassic = (id: string) => id !== 'badge' && (!!FORMS[id] || BRIDGED.includes(id));

/** 动画帧：t 从 0 到 1。排版（统一宽度、放大）只量一次，每帧在排好的树上改透明度和条形长度，再出一遍图 */
export async function classicFrames(id: string, m: Model, t: Theme, ctx: Ctx, satori: Satori, fonts: any[], n = 24): Promise<Art[]> {
  if (isBridged(id, ctx.variant)) return bridgeFrames(id, m, t, ctx, n);
  const f = FORMS[id];
  if (!f || id === 'badge') return [];
  const out = f.render(m, t, ctx);
  if (!out) return [];
  if (ctx.lang === 'en') enTree(out.tree);
  await fitted(out, satori, fonts);
  const laid = JSON.stringify(out.tree);
  const variant = ctx.variant ?? f.def?.(t);
  const rising = id === 'roll' && RISING.has(variant ?? '');
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang)}`;
  const frames: Art[] = [];
  for (let i = 0; i <= n; i++) {
    const k = i / n;
    let tree = JSON.parse(laid);
    if (rising) tree = rise(tree, k, out.w, out.h, t.bg);
    else reveal(tree, k);
    const svg = tidy(await satori(tree, { width: out.w, height: out.h, fonts }), out.w, out.h);
    frames.push({ w: out.w, h: out.h, svg, hits: [], bg: t.bg, title });
  }
  return frames;
}
