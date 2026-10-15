// 代码地图：现在的代码一行一道线，按文件顺序排成一列列（像编辑器的缩略图），颜色是写它的工具。
// 动画：先是一片灰，一道光从左扫到右，扫过的行染上颜色。
// 线的颜色是真实归属；线的长短、缩进只是为了像代码（档案里没有代码），按文件路径固定生成，每次画都一样。
import { Scene, rect, path, r } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt } from '../data.ts';
import type { Style } from '../style.ts';
import { T, colorOf, canvas, tip, type PieceOpts } from './common.ts';

const PITCH = 4.5, CW = 56, GAP = 22, FILE_GAP = 2, SHADE = [1, 0.78, 0.6];

function rng(seed: number) {
  return () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const hash = (s: string) => { let h = 2166136261; for (const ch of s) h = Math.imul(h ^ ch.charCodeAt(0), 16777619); return h | 0; };

/** 每个文件按行展开成分组 key；旧档案没有 runs 时按 by 的比例排成连续的几段 */
function linesOf(f: Model['files'][number]): string[] {
  const out: string[] = [];
  if (f.runs?.length) for (const [k, n] of f.runs) for (let i = 0; i < n; i++) out.push(k);
  else {
    for (const [k, n] of Object.entries(f.by).sort((a, b) => b[1] - a[1])) for (let i = 0; i < n; i++) out.push(k);
    while (out.length < f.lines) out.push('human');
  }
  return out;
}

export function codemap(m: Model, st: Style, o: PieceOpts): Scene | null {
  const files = m.files.filter((f) => f.lines > 0).sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const total = files.reduce((a, f) => a + f.lines, 0);
  if (!files.length || total < 50) return null;
  const t = o.t ?? 1;
  const W = 1280, MAXH = 440;
  // 版面：列宽固定；行太多时一道线代表 k 行（取其中最多的那一方）；行少时各列高度拉平，所有列都用上
  const pw0 = W - 2 * (34 + 50); // 估计值：卡片面可能再往里收一点，画之前按实际宽度重算列宽
  const cols = Math.max(1, Math.floor((pw0 + GAP) / (CW + GAP)));
  const maxRows = Math.floor(MAXH / PITCH);
  const k = Math.max(1, Math.ceil(total / Math.max(1, (cols * maxRows - files.length * FILE_GAP) * 0.9)));
  const strokes = files.reduce((a, f) => a + Math.ceil(f.lines / k) + FILE_GAP, 0);
  const rows = Math.min(maxRows, Math.max(24, Math.ceil(strokes / cols) + 3));
  const plotH = rows * PITCH;
  const { s, c } = canvas(m, st, W, 160 + plotH + 110, 34);
  const x0 = c.x + 50, x1 = c.x + c.w - 50, py = c.y + 150, pw = x1 - x0, cw = (pw - (cols - 1) * GAP) / cols;
  st.label(s, T(m, '代码地图', 'CODE MAP'), x0, c.y + 58, { size: 13 });
  st.title(s, T(m, '每一行，是谁写的', 'Who wrote every line'), x0, c.y + 116, 40, 'start', { maxW: pw * 0.66 });
  st.label(s, m.project, x1, c.y + 58, { anchor: 'end', size: 13, fill: st.c.faint });
  s.text(T(m, `${fmt(total)} 行 · ${fmt(files.length)} 个文件`, `${fmt(total)} lines · ${fmt(files.length)} files`), x1, c.y + 112, { font: st.f.mono, size: 15, fill: st.c.soft, anchor: 'end' });

  const color = (key: string) => (key === 'unattributed' || key === 'human' ? st.c.soft : colorOf(st, m.rows.find((a) => a.key === key) ?? m.rows.find((a) => a.key === 'other') ?? ({ key, rank: 5 } as Agent)));
  const neutral = (key: string) => key === 'unattributed' || key === 'human';
  // 按颜色 × 深浅归并成少数几条 path，文件小
  const layers = new Map<string, string[]>();
  const base: string[] = [];
  const sweepX = x0 - 2 + (pw + 4) * Math.min(1, t / 0.85);
  let col = 0, row = 0;
  for (const f of files) {
    const all = linesOf(f);
    const n = Math.ceil(all.length / k);
    if (row > 0 && row + Math.min(n, 6) > rows) { col++; row = 0; }
    else if (row > 0 && col < cols - 1 && row + n > rows && rows - row < n / 3) { col++; row = 0; } // 剩下的空间放不下三分之一：另起一列
    if (col >= cols) break;
    const R = rng(hash(f.path || String(f.lines)));
    let ind = 0, segStart = row, segCol = col;
    const counts: Record<string, number> = {};
    const flushHit = () => {
      if (row <= segStart) return;
      const fx = x0 + segCol * (cw + GAP);
      const by = Object.entries(f.by).sort((a, b) => b[1] - a[1]).slice(0, 4);
      s.hit(fx - 4, py + segStart * PITCH - 3, cw + 8, (row - segStart) * PITCH + 4, { t: f.path || T(m, '文件', 'file'), d: [T(m, `${fmt(f.lines)} 行`, `${fmt(f.lines)} lines`), ...by.map(([q, v]) => `${m.rows.find((a) => a.key === q)?.name ?? (q === 'unattributed' ? T(m, '未确认', 'Unattributed') : q)} ${Math.round((v / f.lines) * 100)}%`)] });
    };
    for (let i = 0; i < n; i++) {
      if (row >= rows) { flushHit(); col++; row = 0; segStart = 0; segCol = col; if (col >= cols) break; }
      // 这一道线代表的几行里，最多的那一方
      for (const q in counts) delete counts[q];
      let key = all[i * k], best = 0;
      for (let j = i * k; j < Math.min(all.length, (i + 1) * k); j++) { const v = (counts[all[j]] = (counts[all[j]] ?? 0) + 1); if (v > best) { best = v; key = all[j]; } }
      const skip = i > 0 && R() < 0.13; // 空行
      if (!skip) {
        ind = i === 0 ? 0 : Math.max(0, Math.min(4, ind + (R() < 0.22 ? 1 : R() < 0.25 ? -1 : 0)));
        const step = cw / 11, lx = x0 + col * (cw + GAP) + ind * step, end = lx + Math.max(3, (cw - ind * step) * (0.25 + R() * 0.75));
        const y = r(py + row * PITCH + 1);
        for (let x = lx, first = true; x < end - 1.5; first = false) {
          const w = Math.min(end - x, cw / 7 + R() * (cw / 2.2));
          const shade = first ? 0 : R() < 0.45 ? 1 : R() < 0.5 ? 2 : 0;
          const d = `M${r(x)} ${y}h${r(w)}`;
          base.push(d);
          if (x < sweepX) {
            const lk = `${key}|${shade}`;
            (layers.get(lk) ?? layers.set(lk, []).get(lk)!).push(d);
          }
          x += w + 3;
        }
      }
      row++;
    }
    flushHit();
    row += FILE_GAP;
  }
  // 底：全部的线，淡淡一层；扫过的部分盖上颜色
  s.add(path(base.join(''), { fill: 'none', stroke: st.c.ink, strokeOpacity: 0.14, strokeWidth: 2 }));
  for (const [lk, ds] of layers) {
    const [key, sh] = lk.split('|');
    s.add(path(ds.join(''), { fill: 'none', stroke: color(key), strokeOpacity: r(SHADE[+sh] * (neutral(key) ? 0.55 : 1)), strokeWidth: 2 }));
  }
  // 扫描线
  if (t > 0 && t < 0.85) s.add(rect(sweepX - 1, py - 10, 2, plotH + 14, { fill: st.c.ink, opacity: 0.8 }));

  // 占比条与图例：按图里画的这些行（现在的代码）算，只在 AI 和未确认之间分
  const cur: Record<string, number> = {};
  for (const f of files) for (const [q, v] of Object.entries(f.by)) cur[q] = (cur[q] ?? 0) + v;
  const rowOf = (q: string) => m.rows.find((a) => a.key === q) ?? m.rows.find((a) => a.key === 'other');
  const tally = new Map<Agent, number>();
  for (const [q, v] of Object.entries(cur)) { const a = rowOf(q); if (a) tally.set(a, (tally.get(a) ?? 0) + v); }
  const denom = [...tally.values()].reduce((x, y) => x + y, 0) || 1;
  const shown = [...tally].filter(([, v]) => v > 0).sort((a, b) => a[0].rank - b[0].rank).map(([a, v]) => ({ a, pct: (v / denom) * 100 }));
  const by = py + plotH + 26, ly = by + 52;
  s.add(rect(x0, by, pw, 5, { fill: st.c.ink, fillOpacity: 0.1 }));
  let bx = x0;
  const reveal = Math.min(1, Math.max(0, (t - 0.1) / 0.75));
  for (const { a, pct } of shown) {
    const w = (pct / 100) * pw;
    const vis = Math.max(0, Math.min(w, x0 + pw * reveal - bx));
    if (vis > 0) s.add(rect(bx, by, Math.max(0, vis - 3), 5, { fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) }));
    bx += w;
  }
  let lx = x0;
  for (const { a, pct } of shown) {
    if (lx > x1 - 140) break;
    if (a.key === 'unattributed') s.add(rect(lx, ly - 13, 15, 15, { fill: st.hatch(s), rx: 3 }));
    else st.mark(s, a.key, lx + 8, ly - 6, 18, colorOf(st, a));
    let w = s.text(a.name, lx + 26, ly, { font: st.f.body, size: 16, fill: st.c.ink });
    w += 14 + s.text(`${pct.toFixed(1)}%`, lx + 26 + w + 14, ly, { font: st.f.mono, size: 14, fill: st.c.soft });
    s.hit(lx - 4, ly - 20, w + 34, 30, tip(m, a));
    lx += w + 26 + 34;
  }
  if (k > 1) st.label(s, T(m, `一道线 ≈ ${k} 行`, `one stroke ≈ ${k} lines`), x1, ly, { anchor: 'end', size: 12, fill: st.c.faint });
  return s;
}
