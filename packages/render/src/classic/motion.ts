// 原版作品的动画（GIF）：不改原版的排版，只在出帧时改排好版的对象树。条形从零长出来，其余元素按阅读顺序依次淡入；
// 片尾字幕的滚字、长卷整张从画面下方升上来停住。最后一帧就是静态图。
import { ease } from '../svg.ts';
import type { Node } from './ui.ts';

const isNode = (k: any): k is Node => !!k && typeof k === 'object' && !!k.props;
const kidsOf = (n: Node): any[] => { const k = n.props?.children; return k === undefined || k === null ? [] : Array.isArray(k) ? k : [k]; };

/** 叶子：文字、图片、没有子节点的色块。按树的先序就是阅读顺序（上到下、左到右） */
function leaves(n: Node, out: Node[] = []): Node[] {
  if (!isNode(n)) return out;
  const kids = kidsOf(n);
  const sub = kids.filter(isNode);
  if (n.type === 'img' || sub.length === 0 || sub.length < kids.length) { out.push(n); return out; }
  for (const k of sub) leaves(k, out);
  return out;
}

const pctOf = (v: unknown) => (typeof v === 'string' && /^[\d.]+%$/.test(v) ? parseFloat(v) : null);
const filled = (s: Record<string, any>) => !!(s.backgroundColor || s.backgroundImage);

/** t∈[0,1]：就地改树 */
export function reveal(tree: Node, t: number) {
  if (t >= 1) return;
  const ls = leaves(tree);
  ls.forEach((n, i) => {
    const e = ease(t, i, ls.length);
    const s = { ...n.props.style };
    const w = pctOf(s.width), h = pctOf(s.height);
    // 条形：宽（或高）按百分比的色块，从零长出来
    if (filled(s) && (w !== null || h !== null)) {
      if (w !== null && w < 100) { s.width = `${Math.round(w * e * 100) / 100}%`; if (typeof s.minWidth === 'number') s.minWidth = s.minWidth * e; }
      else if (h !== null && h < 100) s.height = `${Math.round(h * e * 100) / 100}%`;
      else s.opacity = (s.opacity ?? 1) * e;
    } else s.opacity = (s.opacity ?? 1) * e;
    n.props = { ...n.props, style: s };
  });
}

/** 滚字：整张从画面下方升上来，停在原位 */
export function rise(tree: Node, t: number, w: number, h: number, bg: string): Node {
  const k = t >= 1 ? 1 : 1 - Math.pow(1 - t, 2.2);
  const style = { ...tree.props.style, position: 'absolute', left: 0, top: Math.round(h * (1 - k)) };
  return { type: 'div', props: { style: { display: 'flex', position: 'relative', width: w, height: h, overflow: 'hidden', backgroundColor: bg }, children: { ...tree, props: { ...tree.props, style } } } };
}

/** 哪些版式用滚字 */
export const RISING = new Set(['crawl', 'scroll']);
