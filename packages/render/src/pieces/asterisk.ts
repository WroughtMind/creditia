// 星号：这个项目自己的 *。瓣长是各家写了多少，下面是项目名、一行份额、一行出处——官网页尾 creditia 给自己署名的那一张。
import { Scene, rect, ease, measure, path, r } from '../svg.ts';
import { phase, styleProgress, drawGroup } from '../motion.ts';
import { materialEnter, materialReveal, materialOrder } from '../material-motion.ts';
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
  // 各材质独立处理花瓣动作，最终瓣长由份额决定。
  const petals = sharePetals(ag.map((a) => ({ tool: a.key, lines: a.lines, color: colorOf(st, a) })), st.c.faint)
    .map((p) => ({ ...p, len: (p.len ?? 1), opacity: (p.opacity ?? 1) }));
  const transform = `translate(${+(cx - SZ / 2).toFixed(2)} ${y}) scale(${+(SZ / 24).toFixed(4)})`;
  const paths = markPaths({ petals });
  if (t >= 1) s.add(`<g transform="${transform}">${paths}</g>`);
  else if (st.id === 'ink') {
    // 恢复原来的逐瓣生长：圆头随长度一起展开，保留原来的错峰与缓出。
    const growing = petals.map((p, i) => {
      const e = ease(t, i, petals.length);
      return { ...p, len: p.len * (0.3 + 0.7 * e), opacity: p.opacity * e };
    });
    s.add(`<g transform="${transform}">${markPaths({ petals: growing })}</g>`);
  } else {
    (paths.match(/<path\b[^>]*\/>/g) ?? []).forEach((p, i) => {
      // 星号是绕中轴排的瓣，成对揭幕从顶瓣向两侧展开，不把列表的中间项当作画面中心。
      const pair = Math.min(i, petals.length - i) / Math.max(1, Math.floor(petals.length / 2));
      const scan = (1 - Math.cos(i / petals.length * Math.PI * 2)) / 2;
      const order = st.id === 'grimoire' ? i / Math.max(1, petals.length - 1) : st.id === 'crt' ? scan
        : ['cinema', 'playbill', 'gala', 'baroque', 'pastel', 'glass'].includes(st.id) ? pair
        : materialOrder(st, i, petals.length, st.id === 'manuscript' ? 1 : petals.length);
      const k = styleProgress(st, t, order * 0.35, 0.62 + order * 0.35);
      const points = polygonPoints(p), scale = SZ / 24;
      const minX = Math.min(...points.map(([x]) => x)), minY = Math.min(...points.map(([, y]) => y));
      const bounds = { x: cx - SZ / 2 + minX * scale, y: y + minY * scale,
        w: (Math.max(...points.map(([x]) => x)) - minX) * scale, h: (Math.max(...points.map(([, y]) => y)) - minY) * scale };
      const draw = () => drawGroup(s, { transform }, () => s.add(p));
      const a = -Math.PI / 2 + i / petals.length * Math.PI * 2;
      if (k >= 1) { draw(); return; }
      if (['cyber', 'neon', 'grimoire'].includes(st.id)) {
        // markPaths 输出的是封闭折线路径，按实际线段求周长，resvg 无须支持 pathLength。
        const length = points.reduce((sum, [x, y], j) => {
          const next = points[(j + 1) % points.length];
          return sum + Math.hypot(next[0] - x, next[1] - y);
        }, 0);
        const d = /\bd="([^"]+)"/.exec(p)![1];
        const fill = /\bfill="([^"]+)"/.exec(p)![1];
        materialEnter(s, st, k, bounds, () => drawGroup(s, { transform }, () => {
        drawGroup(s, { opacity: phase(k, 0.56, 0.9) }, () => s.add(p));
        s.add(path(d, { fill: 'none', stroke: fill, strokeWidth: 0.1,
          strokeOpacity: petals[i].opacity * (1 - phase(k, 0.85, 1)),
          strokeDasharray: length, strokeDashoffset: length * (1 - phase(k, 0, 0.64)),
          visibility: k <= 0 ? 'hidden' : undefined }));
        }), 'mark');
      } else if (['film', 'broadsheet', 'woodblock'].includes(st.id)) {
        // 乳剂、网线、木纹在每瓣的真实轮廓内成像，不把瓣长当作播放进度。
        materialReveal(s, st, k, bounds, 'up', draw);
      } else if (st.id === 'meadow') {
        // 花瓣从花心轻轻舒展，保留每瓣的完整形状与份额；不是整张卡片上浮。
        const turn = (i % 2 ? -1 : 1) * 7 * (1 - k) ** 2;
        drawGroup(s, { transform }, () => drawGroup(s, { transform: `rotate(${r(turn)} 12 12)`, opacity: phase(k, 0, 0.22), visibility: k <= 0 ? 'hidden' : undefined }, () => s.add(p)));
      } else if (st.id === 'cutout') {
        const rest = 1 - k;
        // 每瓣是一张完整纸片，绕自身落位，保持轮廓和面积不变。
        const px = points.reduce((sum, point) => sum + point[0], 0) / points.length;
        const py = points.reduce((sum, point) => sum + point[1], 0) / points.length;
        drawGroup(s, { transform }, () => drawGroup(s, { transform: `translate(${r(Math.cos(a) * 3.2 * rest)} ${r(-2.2 * rest)}) rotate(${r((i % 2 ? -14 : 14) * rest)} ${r(px)} ${r(py)})`, visibility: k <= 0 ? 'hidden' : undefined }, () => s.add(p)));
      } else materialEnter(s, st, k, bounds, draw, 'mark');
    });
  }
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

/** markPaths 的 M/L/Z 顶点就是最终轮廓，用于描线周长与纸片转轴。 */
function polygonPoints(p: string): [number, number][] {
  const d = /\bd="([^"]+)"/.exec(p)![1];
  return [...d.matchAll(/[ML](-?[\d.]+)\s+(-?[\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]);
}
