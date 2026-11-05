// 原版作品的动画（GIF）：不改原版的排版，只在出帧时改排好版的对象树。条形从零长出来，其余元素按阅读顺序依次淡入；
// 片尾字幕的滚字、长卷整张从画面下方升上来停住。最后一帧就是静态图。
import { r } from '../svg.ts';
import { dataUri, type Node } from './ui.ts';
import { motionProfile, materialProgress } from '../motion-profiles.ts';

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
export function reveal(tree: Node, t: number, style = 'minimal-light', piece = '') {
  if (t >= 1) return;
  const ls = leaves(tree), profile = motionProfile(style), kind = profile.material;
  ls.forEach((n, i) => {
    const e = materialProgress(profile, t, i, ls.length);
    const s = { ...n.props.style };
    const w = pctOf(s.width), h = pctOf(s.height);
    const transform = (value: string) => { s.transform = `${s.transform && s.transform !== 'none' ? s.transform : ''} ${value}`.trim(); };
    const numericW = Number(s.width), numericH = Number(s.height);
    // Cells assemble in reading order without changing their layout footprint.
    const cell = filled(s) && numericW > 0 && numericH > 0 && numericW <= 64 && numericH <= 64;
    const text = kidsOf(n).some(child => typeof child === 'string' || typeof child === 'number');
    const rule = numericW > 160 && numericH <= 3 || numericH > 160 && numericW <= 3;
    const chart = ['mosaic', 'waffle', 'area', 'versions', 'lanes', 'clock', 'settle'].includes(piece);
    // Blank plate/frame/rule nodes are structural. Moving those makes a sound
    // card look broken into shards while its data is still arriving.
    if (!text && n.type !== 'img' && !cell && w === null && h === null && (rule || !chart)) return;
    // 条形：宽（或高）按百分比的色块，从零长出来
    if (filled(s) && (w !== null || h !== null)) {
      if (w !== null && w < 100) { s.width = `${Math.round(w * e * 100) / 100}%`; if (typeof s.minWidth === 'number') s.minWidth = s.minWidth * e; }
      else if (h !== null && h < 100) s.height = `${Math.round(h * e * 100) / 100}%`;
      else s.opacity = (s.opacity ?? 1) * e;
    } else if (cell) {
      transform(`scale(${r(e)})`);
      s.opacity = (s.opacity ?? 1) * (e > 0 ? 1 : 0);
    } else s.opacity = (s.opacity ?? 1) * (profile.steps ? (e > 0 ? 1 : 0) : kind === 'neon' ? e * e : e);
    if (!filled(s) && e < 1 && !['scan', 'pixels', 'slices'].includes(kind)) {
      const distance = profile.distance * (1 - e);
      if (['columns', 'page', 'writing'].includes(kind)) transform(`translateX(${-r(distance)}px)`);
      else if (['blocks', 'panels', 'split', 'folds'].includes(kind)) transform(`translateX(${r(distance * (i % 2 ? 1 : -1))}px)`);
      else if (['iris', 'neon', 'bubbles'].includes(kind)) transform(`scale(${r(.88 + .12 * e)})`);
      else if (kind === 'leaves' || kind === 'glass') transform(`translateY(${r(distance)}px) rotate(${r(Math.sin(e * Math.PI) * (i % 2 ? 1 : -1))}deg)`);
      else transform(`translateY(${r(distance)}px)`);
    }
    if (n.type === 'img' && Number(n.props.width) > 120 && Number(n.props.height) > 100)
      revealChart(n, piece === 'donut' ? 'ring' : 'chronology', e);
    n.props = { ...n.props, style: s };
  });
}

/** Data images reveal around a ring or along the chronological x axis. Logos
 * stay untouched; text and neighboring layout retain their measured positions. */
function revealChart(node: Node, kind: 'ring' | 'chronology', progress: number) {
  const src = node.props.src;
  if (progress >= 1 || typeof src !== 'string' || !src.startsWith('data:image/svg+xml;base64,')) return;
  const source = new TextDecoder().decode(Uint8Array.from(atob(src.slice(src.indexOf(',') + 1)), ch => ch.charCodeAt(0)));
  const w = Number(node.props.width), h = Number(node.props.height);
  let shape: string;
  if (kind === 'ring') {
    const cx = w / 2, cy = h / 2, radius = Math.hypot(w, h), angle = progress * Math.PI * 2 - Math.PI / 2;
    shape = `<path d="M${cx} ${cy}L${cx} ${r(cy - radius)}A${r(radius)} ${r(radius)} 0 ${progress > .5 ? 1 : 0} 1 ${r(cx + Math.cos(angle) * radius)} ${r(cy + Math.sin(angle) * radius)}Z"/>`;
  } else shape = `<rect width="${r(w * progress)}" height="${h}"/>`;
  node.props.src = dataUri(source.replace(/^(<svg\b[^>]*>)/, `$1<defs><clipPath id="creditia-motion-chart">${shape}</clipPath></defs><g clip-path="url(#creditia-motion-chart)">`).replace(/<\/svg>\s*$/, '</g></svg>'));
}

/** 滚字：整张从画面下方升上来，停在原位 */
export function rise(tree: Node, t: number, w: number, h: number, bg: string): Node {
  const k = t >= 1 ? 1 : 1 - Math.pow(1 - t, 2.2);
  const style = { ...tree.props.style, position: 'absolute', left: 0, top: Math.round(h * (1 - k)) };
  return { type: 'div', props: { style: { display: 'flex', position: 'relative', width: w, height: h, overflow: 'hidden', backgroundColor: bg }, children: { ...tree, props: { ...tree.props, style } } } };
}

/** 哪些版式用滚字 */
export const RISING = new Set(['crawl', 'scroll']);
