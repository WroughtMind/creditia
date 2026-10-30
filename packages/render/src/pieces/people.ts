// 贡献者：按贡献者看。每位提交者一行，条形按他的提交里各工具、没有 AI 记录、未确认分段。
// 只说是谁提交的，不说是谁手写的；只有一位提交者时没有这张（和署名榜一样）
import { Scene, rect } from '../svg.ts';
import { styleProgress } from '../motion.ts';
import { materialReveal, materialOrder } from '../material-motion.ts';
import type { Model, Agent } from '../data.ts';
import { pct, fmt } from '../data.ts';
import type { Style } from '../style.ts';
import { T, canvas, colorOf, range, summary, type PieceOpts } from './common.ts';

export function people(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.authors.length < 2) return null;
  const t = o.t ?? 1;
  const W = 1080, M = 34, P = 50, rowH = 96;
  const { s, c } = canvas(m, st, W, 190 + m.authors.length * rowH + 60, M);
  const x0 = c.x + P, x1 = c.x + c.w - P;
  st.label(s, T(m, '贡献者', 'CONTRIBUTORS'), x0, c.y + 58, { size: 13 });
  st.title(s, m.project || 'Untitled', x0, c.y + 122, 52, 'start', { maxW: x1 - x0 - 220 });
  st.label(s, range(m), x1, c.y + 58, { size: 12, anchor: 'end', fill: st.c.faint });
  const fill = (key: string) => {
    if (key === 'unattributed') return st.hatch(s);
    if (key === 'noai') return st.color('human', 0);
    const a = m.agents.find((x) => x.key === key) ?? m.rows.find((x) => x.key === key);
    return colorOf(st, a ?? ({ key, rank: 5, kind: 'agent' } as Agent));
  };
  // 条长按占全部新增行的比例，最长的那位占满一行
  const max = Math.max(...m.authors.map((a) => a.pct), 1e-9);
  let y = c.y + 196;
  m.authors.forEach((a, i) => {
    const order = materialOrder(st, i, m.authors.length);
    const k = styleProgress(st, t, order * 0.35, 0.6 + order * 0.35);
    s.text(a.name, x0, y, { font: st.f.body, size: 22, fill: st.c.ink, maxW: (x1 - x0) * 0.62 });
    st.num(s, pct(a.pct), x1, y, 22, 'end');
    st.track(s, x0, y + 16, x1 - x0, 14);
    const bw = (x1 - x0) * (a.pct / max);
    let bx = x0;
    materialReveal(s, st, k, { x: x0, y: y + 16, w: bw, h: 14 }, 'right', () => {
    for (const p of a.parts) {
      const w = a.lines ? (bw * p.lines) / a.lines : 0;
      if (w > 0) s.add(rect(bx, y + 16, w, 14, { fill: fill(p.key) }));
      bx += w;
    }
    });
    const parts = a.parts.map((p) => `${p.name} ${pct(p.pct)}`);
    s.text(parts.join(' · '), x0, y + 56, { font: st.f.mono, size: 13, fill: st.c.soft, maxW: x1 - x0, overflow: 'clip' });
    s.hit(x0, y - 28, x1 - x0, rowH - 8, { k: a.key, t: a.name, d: [T(m, `${fmt(a.lines)} 行 · ${pct(a.pct)}`, `${fmt(a.lines)} lines · ${pct(a.pct)}`), ...parts] });
    y += rowH;
  });
  s.text(summary(m), x0, y + 12, { font: st.f.mono, size: 13, fill: st.c.soft });
  return s;
}
