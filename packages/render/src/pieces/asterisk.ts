// 星号：这个项目自己的 *。瓣长是各家写了多少，下面是项目名、一行份额、一行出处——官网页尾 creditia 给自己署名的那一张。
import { Scene, rect, ease, measure } from '../svg.ts';
import type { Model } from '../data.ts';
import { pct } from '../data.ts';
import type { Style } from '../style.ts';
import { markPaths, sharePetals } from '../mark.ts';
import { T, colorOf, tip, canvas, range, summary, type PieceOpts } from './common.ts';

export function asterisk(m: Model, st: Style, o: PieceOpts): Scene | null {
  const t = o.t ?? 1;
  const ag = m.rows.filter((a) => a.kind === 'agent' && a.lines > 0);
  if (!ag.length) return null;
  const W = 1080, M = 40, pad = 64;
  // 份额一行：不到 1% 的并进「其他」，和瓣一样的颜色
  const rows = m.rows.filter((a) => a.lines > 0);
  const big = rows.filter((a) => a.pct >= 1), small = rows.filter((a) => a.pct < 1);
  const items = big.map((a) => ({ a, name: a.name, p: a.pct }));
  if (small.length) items.push({ a: null as any, name: T(m, '其他', 'Other'), p: small.reduce((s, a) => s + a.pct, 0) });
  // 先在最宽的内容区里排好几行，再定画布高度
  const FS = 22, GAP = 40, maxW = W - M * 2 - pad * 2 - 40;
  const lines: { w: number; it: typeof items }[] = [{ w: 0, it: [] }];
  for (const it of items) {
    const w = 14 + 12 + measure(it.name, st.f.body, FS) * 1.05 + 12 + measure(pct(it.p), st.f.num, FS) * 1.05;
    const cur = lines[lines.length - 1];
    if (cur.it.length && cur.w + GAP + w > maxW) lines.push({ w, it: [it] });
    else { cur.w += (cur.it.length ? GAP : 0) + w; cur.it.push(it); }
    (it as any).w = w;
  }
  const SZ = 460;
  const innerH = 70 + Math.round(SZ * 0.86) + 104 + 56 + lines.length * 46 + 34 + 30 + 60;
  const { s, c } = canvas(m, st, W, innerH, M);
  const cx = c.x + c.w / 2;
  let y = c.y + 70;
  // 星号：各瓣依次长出来
  const petals = sharePetals(ag.map((a) => ({ tool: a.key, lines: a.lines, color: colorOf(st, a) })), st.c.faint)
    .map((p, i, all) => { const e = ease(t, i, all.length); return { ...p, len: (p.len ?? 1) * (0.3 + 0.7 * e), opacity: (p.opacity ?? 1) * e }; });
  s.add(`<g transform="translate(${+(cx - SZ / 2).toFixed(2)} ${y}) scale(${+(SZ / 24).toFixed(4)})">${markPaths({ petals })}</g>`);
  s.hit(cx - SZ / 2, y, SZ, SZ, { t: m.project || 'Untitled', d: ag.slice(0, 6).map((a) => `${a.name} ${pct(a.pct)}`) });
  y += SZ * 0.86 + 104; // 星号下面两瓣只伸到画框的 86%
  st.title(s, m.project || 'Untitled', cx, y, 60, 'middle', { maxW: c.w - pad * 2 });
  y += 56;
  const k = ease(t, 0, 1);
  for (const ln of lines) {
    let x = cx - ln.w / 2;
    for (const it of ln.it as (typeof items[number] & { w: number })[]) {
      const color = !it.a || it.a.key === 'unattributed' ? st.c.faint : colorOf(st, it.a);
      if (it.a?.key === 'unattributed') s.add(rect(x, y - 15, 14, 14, { fill: st.hatch(s) }));
      else st.bar(s, x, y - 15, 14, 14, color, it.a?.rank ?? 9);
      const nx = x + 14 + 12;
      const nw = s.text(it.name, nx, y, { font: st.f.body, size: FS, fill: st.c.ink });
      st.num(s, pct(it.p * k), nx + nw + 12, y, FS, 'start', { fill: st.c.soft });
      if (it.a) s.hit(x - 4, y - 30, it.w + 8, 42, tip(m, it.a));
      else s.hit(x - 4, y - 30, it.w + 8, 42, { t: T(m, '其他', 'Other'), d: small.map((a) => `${a.name} ${pct(a.pct)}`) });
      x += it.w + GAP;
    }
    y += 46;
  }
  y += 34;
  st.label(s, [summary(m), range(m)].filter(Boolean).join(' · '), cx, y, { anchor: 'middle', size: 15, fill: st.c.soft, maxW: c.w - pad * 2 });
  return s;
}
