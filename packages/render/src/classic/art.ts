// 原版作品 → 统一的 Art（App、官网、命令行导出都吃这个）。satori 由调用方传进来：Node 用完整版，浏览器用 standalone。
import { FORMS, type Ctx } from './registry.ts';
import { badge } from './forms.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Art } from '../export.ts';
import { pieceName } from './catalog.ts';
import { FIT_KEY, BODY_KEY, CONTENT_KEY } from './ui.ts';
import { enTree, enSvg } from './i18n.ts';
import { BRIDGED, BRIDGED_LAYOUTS, isBridged, bridgeArt, bridgeFrames } from './bridge.ts';
import { animate } from './motion.ts';
import { motionDuration } from '../motion.ts';
import { hitsOf, type Seen } from './hits.ts';

export type Satori = (tree: any, o: { width: number; height: number; fonts: any[]; onNodeDetected?: (n: any) => void }) => Promise<string>;

/** 同一页面会内联多张 satori 出的 SVG，它们的 clipPath/mask id 都是 satori_ 开头，会互相顶掉；每张换成独有前缀 */
let seq = 0;
const tidy = (svg: string, w: number, h: number) => svg.replace(/satori_/g, `s${(seq++).toString(36)}_`)
  // satori 给 1px 高的虚线边框算出负高度的遮罩，浏览器会报错；归零
  .replace(/ (width|height)="-[\d.]+"/g, ' $1="0"')
  // 阴影滤镜的区域是元素的好几倍，栅格化时很吃内存；画布外本来看不见，收到画布以内
  .replace(/<filter\b([^>]*)>/g, (_, attrs: string) => `<filter${attrs.replace(/\s(?:x|y|width|height|filterUnits)="[^"]*"/g, '')} filterUnits="userSpaceOnUse" x="0" y="0" width="${w}" height="${h}">`);

/** 按 key 找节点 */
function findKey(n: any, key: string): any {
  if (!n || typeof n !== 'object') return null;
  if (n.key === key) return n;
  const kids = n.props?.children;
  for (const k of Array.isArray(kids) ? kids : [kids]) { const f = findKey(k, key); if (f) return f; }
  return null;
}
/** 找到要放大的节点 */
const findFit = (n: any) => findKey(n, FIT_KEY);

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

/** 标了 oneLine 的文字：折行就收字号，收到底还折行就按字数均分成两行（见 ui.oneLine） */
async function oneLines(out: { tree: any; w: number; h: number }, satori: Satori, fonts: any[]) {
  const nodes: any[] = [];
  const walk = (n: any) => { if (!n || typeof n !== 'object') return; if (n.oneLine) nodes.push(n); const k = n.props?.children; for (const c of Array.isArray(k) ? k : [k]) walk(c); };
  walk(out.tree);
  if (!nodes.length) return;
  const base = new Map(nodes.map((n) => [n, Number(n.props.style.fontSize ?? 16)]));
  let todo = nodes;
  // 离画布左右边至少留 24：贴边的一行也算放不下
  const edge = 24;
  for (let pass = 0; pass < 6 && todo.length; pass++) {
    const at = new Map<string, Seen>();
    await satori(out.tree, { width: out.w, height: out.h, fonts, onNodeDetected: (x) => { if (typeof x.key === 'string' && x.key.startsWith('creditia-line:')) at.set(x.key, x); } });
    todo = todo.filter((n) => { const x = at.get(n.key); return !!x && (x.height > Number(n.props.style.fontSize) * 1.8 || x.left < edge || x.left + x.width > out.w - edge); });
    for (const n of todo) {
      const fs = Number(n.props.style.fontSize), min = base.get(n)! * 0.78;
      if (fs > min + 0.01) n.props.style = { ...n.props.style, fontSize: Math.max(min, Math.round(fs * 0.93 * 100) / 100) };
      else {
        // 字号到底了：拆成两行，断在最接近一半的空格或标点后面
        const text = String(n.props.children);
        const cut = [...text.matchAll(/[\s、，,]/g)].map((x) => x.index! + 1).sort((a, b) => Math.abs(a - text.length / 2) - Math.abs(b - text.length / 2))[0] ?? Math.ceil(text.length / 2);
        n.props = { ...n.props, style: { ...n.props.style, flexDirection: 'column', alignItems: 'center', lineHeight: 1.25 },
          children: [text.slice(0, cut).trim(), text.slice(cut).trim()].map((l) => ({ type: 'div', props: { style: { display: 'flex', whiteSpace: 'nowrap' }, children: l } })) };
        n.oneLine = false;
      }
    }
    todo = todo.filter((n) => n.oneLine);
  }
}

/** 高度随内容的作品：正文比正文区高出多少，画布就加高多少（表头、框线、底部刻度都按实际排版算进去） */
async function grow(out: { tree: any; w: number; h: number }, satori: Satori, fonts: any[]) {
  const node = findKey(out.tree, CONTENT_KEY);
  if (!node) return;
  // 先在足够高的画布上排一遍：正文区撑开，正文是它本来的高度；正文区以下（图例、留白、框线）贴着底边
  // 正文自己是 flex: 1 时（版本柱状图）量的时候不让它撑满，量完还原，出图时照旧铺满加高后的正文区
  const own = { ...node.props.style, flexShrink: 0 };
  node.props.style = { ...own, flexGrow: 0, flexBasis: 'auto' };
  const tall = out.h + 4000, root = out.tree.props.style;
  out.tree.props.style = { ...root, height: tall };
  let body: Seen | undefined, content: Seen | undefined;
  await satori(out.tree, { width: out.w, height: tall, fonts, onNodeDetected: (x) => { if (x.key === BODY_KEY) body = x; else if (x.key === CONTENT_KEY) content = x; } });
  out.tree.props.style = root;
  node.props.style = own;
  if (!body || !content) return;
  const need = Math.ceil(content.top + content.height + (tall - (body.top + body.height)));
  if (need <= out.h) return;
  out.h = need;
  out.tree.props.style = { ...root, height: need };
}

/** seen：最后出图那一遍里每个节点的位置（悬停细节用） */
async function fitted(out: { tree: any; w: number; h: number; fit?: { padX: number; padY: number }; grow?: boolean }, satori: Satori, fonts: any[], seen?: Seen[]): Promise<string> {
  await equalize(out.tree, out.w, out.h, satori, fonts);
  await oneLines(out, satori, fonts);
  if (out.grow) await grow(out, satori, fonts);
  const node = out.fit ? findFit(out.tree) : null;
  let box: { width: number; height: number } | null = null;
  let body: Seen | null = null;
  const pass: Seen[] = [];
  const done = (svg: string, nodes: Seen[]) => { seen?.push(...nodes); return svg; };
  const first = await satori(out.tree, { width: out.w, height: out.h, fonts, onNodeDetected: node || seen ? (x) => { if (x.key === FIT_KEY) box = x; if (x.key === BODY_KEY) body = x; if (seen) pass.push(x); } : undefined });
  if (!node || !box) return done(first, pass);
  const b = box as { width: number; height: number };
  const bounds = body as Seen | null;
  const width = bounds?.width ?? out.w - 2 * out.fit!.padX;
  const height = bounds?.height ?? out.h - 2 * out.fit!.padY;
  const k = Math.min(width / b.width, height / b.height, node.fitMax ?? 1.6);
  const kk = bounds ? k : Math.max(1, k);
  // 宽度先到顶时，竖向剩下的空间分给标了 spread 的节点里各项之间的间距
  const sp = findSpread(node);
  const kids = sp ? (Array.isArray(sp.props.children) ? sp.props.children.filter(Boolean) : [sp.props.children]) : [];
  const spare = height / kk - b.height;
  if (sp && kids.length > 1 && spare > 8) {
    const gap = Number(sp.props.style.gap ?? 0);
    const add = Math.min(spare / (kids.length - 1), Math.max(gap, 24) * (sp.fitSpread - 1));
    sp.props.style = { ...sp.props.style, gap: Math.round(gap + add) };
  } else if (Math.abs(kk - 1) < 0.04) return done(first, pass);
  if (Math.abs(kk - 1) >= 0.04) node.props.style = { ...node.props.style, transform: `scale(${Math.round(kk * 1000) / 1000})` };
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
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang, m.raw)}`;
  // 原版没有的作品、追加的新风格版式：按这个主题的颜色、字体、框线用新风格的画法画
  if (isBridged(id, ctx.variant)) return bridgeArt(id, m, t, ctx);
  if (id === 'badge') {
    const svg = ctx.lang === 'en' ? enSvg(badge(m, t)) : badge(m, t);
    const w = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 200), h = Number(/height="([\d.]+)"/.exec(svg)?.[1] ?? 28);
    return { w, h, svg, hits: [], bg: 'transparent', title };
  }
  const f = FORMS[id];
  if (!f) throw new Error(`未知卡片：${id}`);
  const out = f.render(m, t, ctx);
  if (!out) return null;
  if (ctx.lang === 'en') enTree(out.tree);
  const seen: Seen[] = [];
  const svg = tidy(await fitted(out, satori, fonts, seen), out.w, out.h);
  return { w: out.w, h: out.h, svg, hits: hitsOf(seen, m, t, ctx.lang, out.w, out.h), bg: t.bg, title };
}

/** 原版形式、原生 SVG 徽章和桥接作品都能出帧。 */
export const animatedClassic = (id: string) => id === 'badge' || !!FORMS[id] || BRIDGED.includes(id);

/** 动画帧：t 从 0 到 1。排版（统一宽度、放大）只量一次，每帧在排好的树上按作品标出的内容组改绘制属性，再出一遍图 */
export async function classicFrames(id: string, m: Model, t: Theme, ctx: Ctx, satori: Satori, fonts: any[], n = 24): Promise<Art[]> {
  if (isBridged(id, ctx.variant)) return bridgeFrames(id, m, t, ctx, n);
  if (id === 'badge') {
    const still = (await classicArt(id, m, t, ctx, satori, fonts))!;
    const frameDelay = motionDuration(id, ctx.variant, t.id) / n;
    return Array.from({ length: n + 1 }, (_, i) => ({ ...still, svg: ctx.lang === 'en' ? enSvg(badge(m, t, i / n)) : badge(m, t, i / n), frameDelay }));
  }
  const f = FORMS[id];
  if (!f) return [];
  const out = f.render(m, t, ctx);
  if (!out) return [];
  if (ctx.lang === 'en') enTree(out.tree);
  await fitted(out, satori, fonts);
  const laid = JSON.stringify(out.tree);
  const title = `${m.project || 'credits'} · ${pieceName(id, ctx.lang, m.raw)}`;
  const frames: Art[] = [];
  const frameDelay = motionDuration(id, ctx.variant ?? f.def?.(t), t.id) / n;
  for (let i = 0; i <= n; i++) {
    const k = i / n;
    const tree = JSON.parse(laid);
    animate(tree, k, t);
    const svg = tidy(await satori(tree, { width: out.w, height: out.h, fonts }), out.w, out.h);
    frames.push({ w: out.w, h: out.h, svg, hits: [], bg: t.bg, title, frameDelay });
  }
  return frames;
}
