// 星号：这个项目自己的 *。瓣长是各家写了多少，下面是项目名、一行份额、一行出处——官网页尾 creditia 给自己署名的那一张。
import { Scene, rect, measure, el, path, r } from '../svg.ts';
import { phase, styleProgress, move, drawGroup } from '../motion.ts';
import type { Model } from '../data.ts';
import { pct } from '../data.ts';
import type { Style } from '../style.ts';
import { markPaths, sharePetals } from '../mark.ts';
import { colorOf, tip, canvas, range, summary, type PieceOpts } from './common.ts';

export function asterisk(m: Model, st: Style, o: PieceOpts): Scene | null {
  const t = o.t ?? 1;
  const ag = m.agents.filter((a) => a.kind === 'agent' && a.lines > 0);
  if (!ag.length) return null;
  const W = 1080, M = 40, pad = 64;
  // 全部份额直接列出；小份额保留名字，放不下就换行。
  const items = m.all.filter((a) => a.lines > 0).map((a) => ({ a, name: a.name, p: a.pct, w: 0 }));
  // 先在最宽的内容区里排好几行，再定画布高度
  const FS = 22, GAP = 40, maxW = W - M * 2 - pad * 2 - 40;
  const lines: { w: number; it: typeof items }[] = [{ w: 0, it: [] }];
  for (const it of items) {
    const w = 14 + 12 + measure(it.name, st.f.body, FS) * 1.05 + 12 + measure(pct(it.p), st.f.num, FS) * 1.05;
    const cur = lines[lines.length - 1];
    if (cur.it.length && cur.w + GAP + w > maxW) lines.push({ w, it: [it] });
    else { cur.w += (cur.it.length ? GAP : 0) + w; cur.it.push(it); }
    it.w = w;
  }
  const SZ = 460;
  const innerH = 70 + Math.round(SZ * 0.86) + 104 + 56 + lines.length * 46 + 34 + 30 + 60;
  const { s, c } = canvas(m, st, W, innerH, M);
  const cx = c.x + c.w / 2;
  let y = c.y + 70;
  // 完整瓣形只画一次；材质决定揭墨、描线或纸片落位，不重算份额对应的瓣长。
  const petals = sharePetals(ag.map((a) => ({ tool: a.key, lines: a.lines, color: colorOf(st, a) })), st.c.faint)
    .map((p) => ({ ...p, len: (p.len ?? 1), opacity: (p.opacity ?? 1) }));
  const transform = `translate(${+(cx - SZ / 2).toFixed(2)} ${y}) scale(${+(SZ / 24).toFixed(4)})`;
  const paths = markPaths({ petals });
  if (t >= 1) s.add(`<g transform="${transform}">${paths}</g>`);
  else drawGroup(s, { transform }, () => {
    (paths.match(/<path\b[^>]*\/>/g) ?? []).forEach((p, i) => {
      const k = styleProgress(st, t, i / petals.length * 0.35, 0.62 + i / petals.length * 0.35);
      const a = -Math.PI / 2 + i / petals.length * Math.PI * 2;
      if (k >= 1) { s.add(p); return; }
      if (['ink', 'woodblock'].includes(st.id)) {
        // 沿每瓣自己的轴线，由星心向外铺开；木刻风格沿用分步印出的节奏。
        const points = polygonPoints(p);
        const reach = Math.max(...points.map(([x, y]) => (x - 12) * Math.cos(a) + (y - 12) * Math.sin(a)));
        const clipPath = s.url(`petal-ink-${i}`, (id) => el('clipPath', { id, clipPathUnits: 'userSpaceOnUse' },
          rect(12, -4, reach * k, 32, { transform: `rotate(${r(a * 180 / Math.PI)} 12 12)` })));
        drawGroup(s, { clipPath }, () => s.add(p));
      } else if (['cyber', 'neon'].includes(st.id)) {
        // markPaths 输出的是封闭折线路径，按实际线段求周长，resvg 无须支持 pathLength。
        const points = polygonPoints(p);
        const length = points.reduce((sum, [x, y], j) => {
          const next = points[(j + 1) % points.length];
          return sum + Math.hypot(next[0] - x, next[1] - y);
        }, 0);
        const d = /\bd="([^"]+)"/.exec(p)![1];
        const fill = /\bfill="([^"]+)"/.exec(p)![1];
        drawGroup(s, { opacity: phase(k, 0.56, 0.9) }, () => s.add(p));
        s.add(path(d, { fill: 'none', stroke: fill, strokeWidth: 0.1,
          strokeOpacity: petals[i].opacity * (1 - phase(k, 0.85, 1)),
          strokeDasharray: length, strokeDashoffset: length * (1 - phase(k, 0, 0.64)),
          visibility: k <= 0 ? 'hidden' : undefined }));
      } else if (st.id === 'cutout') {
        const rest = 1 - k;
        // 每瓣是一张完整纸片，绕自身落位，保持轮廓和面积不变。
        const points = polygonPoints(p);
        const px = points.reduce((sum, point) => sum + point[0], 0) / points.length;
        const py = points.reduce((sum, point) => sum + point[1], 0) / points.length;
        drawGroup(s, { transform: `translate(${r(Math.cos(a) * 3.2 * rest)} ${r(-2.2 * rest)}) rotate(${r((i % 2 ? -14 : 14) * rest)} ${r(px)} ${r(py)})`, visibility: k <= 0 ? 'hidden' : undefined }, () => s.add(p));
      } else move(s, k, Math.cos(a) * 2.8, Math.sin(a) * 2.8, () => s.add(p));
    });
  });
  s.hit(cx - SZ / 2, y, SZ, SZ, { t: m.project || 'Untitled', d: ag.map((a) => `${a.name} ${pct(a.pct)}`) });
  y += SZ * 0.86 + 104; // 星号下面两瓣只伸到画框的 86%
  st.title(s, m.project || 'Untitled', cx, y, 60, 'middle', { maxW: c.w - pad * 2 });
  y += 56;
  for (const ln of lines) {
    let x = cx - ln.w / 2;
    for (const it of ln.it) {
      const color = it.a.key === 'unattributed' ? st.c.faint : colorOf(st, it.a);
      if (it.a.key === 'unattributed') s.add(rect(x, y - 15, 14, 14, { fill: st.hatch(s) }));
      else st.bar(s, x, y - 15, 14, 14, color, it.a.rank);
      const nx = x + 14 + 12;
      const nw = s.text(it.name, nx, y, { font: st.f.body, size: FS, fill: st.c.ink });
      st.num(s, pct(it.p), nx + nw + 12, y, FS, 'start', { fill: st.c.soft });
      s.hit(x - 4, y - 30, it.w + 8, 42, tip(m, it.a));
      x += it.w + GAP;
    }
    y += 46;
  }
  y += 34;
  st.label(s, [summary(m), range(m)].filter(Boolean).join(' · '), cx, y, { anchor: 'middle', size: 15, fill: st.c.soft, maxW: c.w - pad * 2 });
  return s;
}

/** markPaths 的 M/L/Z 顶点就是最终轮廓，裁剪距离与描线周长均取自这些真实坐标。 */
function polygonPoints(p: string): [number, number][] {
  const d = /\bd="([^"]+)"/.exec(p)![1];
  return [...d.matchAll(/[ML](-?[\d.]+)\s+(-?[\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]);
}
