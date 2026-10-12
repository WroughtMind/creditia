// 原版作品 → 统一的 Art（App、官网、命令行导出都吃这个）。satori 由调用方传进来：Node 用完整版，浏览器用 standalone。
import { FORMS, type Ctx } from './registry.ts';
import { badge } from './forms.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Art } from '../export.ts';
import { pieceName } from './catalog.ts';

export type Satori = (tree: any, o: { width: number; height: number; fonts: any[] }) => Promise<string>;

/** 同一页面会内联多张 satori 出的 SVG，它们的 clipPath/mask id 都是 satori_ 开头，会互相顶掉；每张换成独有前缀 */
let seq = 0;
const tidy = (svg: string, w: number, h: number) => svg.replace(/satori_/g, `s${(seq++).toString(36)}_`)
  // satori 给 1px 高的虚线边框算出负高度的遮罩，浏览器会报错；归零
  .replace(/ (width|height)="-[\d.]+"/g, ' $1="0"')
  // 阴影滤镜的区域是元素的好几倍，栅格化时很吃内存；画布外本来看不见，收到画布以内
  .replace(/<filter id="([^"]+)" x="[^"]*" y="[^"]*" width="[^"]*" height="[^"]*"/g, `<filter id="$1" filterUnits="userSpaceOnUse" x="0" y="0" width="${w}" height="${h}"`);

export const variantsOf = (id: string): string[] => FORMS[id]?.variants ?? [];
export const defaultVariant = (id: string, t: Theme): string | undefined => FORMS[id]?.def?.(t);

export async function classicArt(id: string, m: Model, t: Theme, ctx: Ctx, satori: Satori, fonts: any[]): Promise<Art | null> {
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang)}`;
  if (id === 'badge') {
    const svg = badge(m, t);
    const w = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 200), h = Number(/height="([\d.]+)"/.exec(svg)?.[1] ?? 28);
    return { w, h, svg, hits: [], bg: 'transparent', title };
  }
  const f = FORMS[id];
  if (!f) throw new Error(`未知作品：${id}`);
  const out = f.render(m, t, ctx);
  if (!out) return null;
  const svg = tidy(await satori(out.tree, { width: out.w, height: out.h, fonts }), out.w, out.h);
  return { w: out.w, h: out.h, svg, hits: [], bg: t.bg, title };
}
