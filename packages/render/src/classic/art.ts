// 原版作品 → 统一的 Art（App、官网、命令行导出都吃这个）。satori 由调用方传进来：Node 用完整版，浏览器用 standalone。
import { FORMS, type Ctx } from './registry.ts';
import { badge } from './forms.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Art } from '../export.ts';
import { pieceName } from './catalog.ts';
import { FIT_KEY } from './ui.ts';
import { enTree, enSvg } from './i18n.ts';

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

async function fitted(out: { tree: any; w: number; h: number; fit?: { padX: number; padY: number } }, satori: Satori, fonts: any[]): Promise<string> {
  await equalize(out.tree, out.w, out.h, satori, fonts);
  const node = out.fit ? findFit(out.tree) : null;
  let box: { width: number; height: number } | null = null;
  const first = await satori(out.tree, { width: out.w, height: out.h, fonts, onNodeDetected: node ? (x) => { if (x.key === FIT_KEY) box = x; } : undefined });
  if (!node || !box) return first;
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
  } else if (k < 1.04) return first;
  if (k >= 1.04) node.props.style = { ...node.props.style, transform: `scale(${Math.round(k * 1000) / 1000})` };
  return satori(out.tree, { width: out.w, height: out.h, fonts });
}

export const variantsOf = (id: string): string[] => FORMS[id]?.variants ?? [];
export const defaultVariant = (id: string, t: Theme): string | undefined => FORMS[id]?.def?.(t);

export async function classicArt(id: string, m: Model, t: Theme, ctx: Ctx, satori: Satori, fonts: any[]): Promise<Art | null> {
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang)}`;
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
  const svg = tidy(await fitted(out, satori, fonts), out.w, out.h);
  return { w: out.w, h: out.h, svg, hits: [], bg: t.bg, title };
}
