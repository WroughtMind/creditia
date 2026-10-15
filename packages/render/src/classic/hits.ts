// 原版作品的悬停细节。satori 排版时报告每个节点的位置；写着某个工具名字的那一行、填着它颜色的色块，
// 就在那里放一个悬停区，内容和新风格一样（tipOf：行数、模型、留存、会话、证据）。不改原版的排版。
import { tipOf } from '../data.ts';
import type { Hit } from '../svg.ts';
import { famColor } from './brand.ts';
import { v2ModelOf } from './bridge.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import { FIT_KEY } from './ui.ts';

/** satori 的 onNodeDetected 报来的节点（坐标是变换之前的） */
export interface Seen { left: number; top: number; width: number; height: number; key?: unknown; textContent?: string; props?: { style?: Record<string, any> } }

const inside = (a: Seen, b: Seen, slack = 1) =>
  a.left >= b.left - slack && a.top >= b.top - slack && a.left + a.width <= b.left + b.width + slack && a.top + a.height <= b.top + b.height + slack;

export function hitsOf(nodes: Seen[], m: Model, t: Theme, lang: 'zh' | 'en', W: number, H: number): Hit[] {
  const v2 = v2ModelOf(m.raw, lang, m.group);
  const tips = new Map<string, ReturnType<typeof tipOf>>();
  for (const a of v2.rows) tips.set(a.key, tipOf(v2, a));
  const byName = new Map<string, string>();
  for (const r of m.rows) if (tips.has(r.key)) byName.set(r.name.trim(), r.key);
  const byColor = new Map<string, string>();
  // 和底色、卡片面同色的不认（那是底，不是某家的色块）
  const own = new Set([t.bg, t.surface].map((c) => c.toLowerCase()));
  m.rows.forEach((r, i) => { if (tips.has(r.key) && r.key !== 'unattributed' && r.key !== 'other') { const c = famColor(r.key, t, i).toLowerCase(); if (!byColor.has(c) && !own.has(c)) byColor.set(c, r.key); } });

  // 放大：fit 节点按自己的中心缩放，里面的节点跟着变
  const fit = nodes.find((n) => n.key === FIT_KEY);
  const k = Number(/scale\(([\d.]+)\)/.exec(String(fit?.props?.style?.transform ?? ''))?.[1] ?? 1);
  const place = (n: Seen) => {
    if (!fit || k === 1 || !inside(n, fit)) return { x: n.left, y: n.top, w: n.width, h: n.height };
    const cx = fit.left + fit.width / 2, cy = fit.top + fit.height / 2;
    return { x: cx + (n.left - cx) * k, y: cy + (n.top - cy) * k, w: n.width * k, h: n.height * k };
  };
  const big = (n: Seen) => n.width >= W * 0.9 && n.height >= H * 0.4;

  const named = nodes.flatMap((n) => { const key = n.textContent ? byName.get(n.textContent.trim()) : undefined; return key ? [{ n, key }] : []; });
  const rows: { n: Seen; key: string }[] = [];
  for (const { n, key } of named) {
    // 名字所在的那一行：包含名字、不比名字高太多、不含别家名字的节点里最宽的（横排图例不会整条算成一家）
    let row = n;
    for (const c of nodes) {
      if (c === n || big(c) || !inside(n, c) || c.height > Math.max(n.height * 3.5, n.height + 48) || c.width <= row.width) continue;
      if (named.some((o) => o.key !== key && inside(o.n, c))) continue;
      row = c;
    }
    rows.push({ n: row, key });
  }
  const blocks: { n: Seen; key: string }[] = [];
  for (const n of nodes) {
    const st = n.props?.style ?? {};
    if (n.textContent || big(n) || n.width < 3 || n.height < 3) continue;
    const key = typeof st.backgroundColor === 'string' ? byColor.get(st.backgroundColor.toLowerCase())
      : typeof st.backgroundImage === 'string' && st.backgroundImage.includes('repeating-linear-gradient') && st.backgroundImage.includes(t.stripeA) && tips.has('unattributed') ? 'unattributed' : undefined;
    // 里面有字的是标题条、徽记这类容器，不是数据色块
    if (!key || rows.some((r) => inside(n, r.n)) || named.some((o) => inside(o.n, n)) || nodes.some((o) => o !== n && o.textContent && inside(o, n))) continue;
    blocks.push({ n, key });
  }
  const out: Hit[] = [];
  const seen = new Set<string>();
  for (const { n, key } of [...rows, ...blocks]) {
    const p = place(n);
    const id = `${Math.round(p.x)},${Math.round(p.y)},${Math.round(p.w)},${Math.round(p.h)}`;
    if (seen.has(id) || p.w <= 0 || p.h <= 0) continue;
    seen.add(id);
    out.push({ x: Math.round(p.x), y: Math.round(p.y), w: Math.round(p.w), h: Math.round(p.h), ...tips.get(key)! });
  }
  return out;
}
