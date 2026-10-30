// 代码地图：现在的代码一行一道线，按文件顺序排成一列列（像编辑器的缩略图），颜色是写它的工具。
// 动画：每行保持完整，依材质的铺版顺序染色；终端输出、扫描余辉和纸面压印各有自己的动作。
// 线的颜色是真实归属；线的长短、缩进只是为了像代码（档案里没有代码），按文件路径固定生成，每次画都一样。
import { Scene, rect, path, r, el, type Attrs } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt } from '../data.ts';
import type { Style } from '../style.ts';
import { T, colorOf, canvas, tip, type PieceOpts } from './common.ts';
import { styleProgress, phase, drawGroup } from '../motion.ts';
import { materialEnter, materialOrder } from '../material-motion.ts';

const PITCH = 4.5, CW = 56, GAP = 22, FILE_GAP = 2, SHADE = [1, 0.78, 0.6];

function rng(seed: number) {
  return () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const hash = (s: string) => { let h = 2166136261; for (const ch of s) h = Math.imul(h ^ ch.charCodeAt(0), 16777619); return h | 0; };

interface CodeStroke { d: string; key: string; shade: number; file: number; col: number; row: number; x: number; y: number; w: number }

/** 文件缩略图有自己的运动单元：排版/拼贴用文件，打印用整行，套色用归属色版。 */
function codeUnits(s: Scene, st: Style, progress: number, strokes: CodeStroke[], columns: number, rows: number, color: (key: string) => string, neutral: (key: string) => boolean) {
  const id = st.id === 'minimal' ? 'minimal-light' : st.id === 'noir' ? 'minimal-dark' : st.id;
  const mode = id === 'ukiyoe' ? 'plate' : id === 'broadsheet' ? 'column'
    : ['terminal', 'crt', 'ink', 'manuscript', 'neon'].includes(id) ? 'line' : 'file';
  const units = new Map<string, CodeStroke[]>();
  for (const q of strokes) {
    const key = mode === 'plate' ? color(q.key) : mode === 'column' ? `${q.col}` : mode === 'line' ? `${q.col}/${q.row}` : `${q.file}/${q.col}`;
    (units.get(key) ?? units.set(key, []).get(key)!).push(q);
  }
  const draw = (part: CodeStroke[]) => {
    const layers = new Map<string, string[]>();
    for (const q of part) { const key = `${q.key}|${q.shade}`; (layers.get(key) ?? layers.set(key, []).get(key)!).push(q.d); }
    for (const [key, ds] of layers) { const [who, shade] = key.split('|'); s.add(path(ds.join(''), { fill: 'none', stroke: color(who), strokeOpacity: r(SHADE[+shade] * (neutral(who) ? 0.55 : 1)), strokeWidth: 2 })); }
  };
  const settled: CodeStroke[] = [];
  const parts = [...units.values()];
  const orders = parts.map((part, i) => mode === 'plate' ? i / Math.max(1, parts.length - 1)
    : materialOrder(st, Math.round((part[0].row + part.at(-1)!.row) / 2) * columns + part[0].col, rows * columns, columns));
  const firstOrder = Math.min(...orders), orderSpan = Math.max(...orders) - firstOrder;
  let index = 0;
  for (const part of parts) {
    const first = part[0];
    // 文件可能跨越大半列；按实际存在的单元归一，避免前半段一直只有底稿。
    const order = orderSpan > 0 ? (orders[index] - firstOrder) / orderSpan : 0;
    const k = phase(progress, order * 0.68, order * 0.68 + 0.32), unit = index++;
    if (k <= 0) continue;
    if (k >= 1) { settled.push(...part); continue; }
    const x = Math.min(...part.map(q => q.x)), y = Math.min(...part.map(q => q.y));
    const w = Math.max(...part.map(q => q.x + q.w)) - x, h = Math.max(...part.map(q => q.y)) - y + PITCH;
    const rest = 1 - k, side = first.col < columns / 2 ? -1 : 1;
    const a: Attrs = { 'data-code-unit': mode };
    let dx = 0, dy = 0, angle = 0, optics = '';
    // 真实笔画整组入位；不借助图表矩形裁窗，也不把行长/归属随进度重算。
    switch (id) {
      case 'minimal-light': dx = -4 * rest; break;
      case 'minimal-dark': optics = 'edge'; break;
      case 'glass': optics = 'focus'; dy = -2 * rest; break;
      case 'film': optics = 'exposure'; dy = k < 0.3 ? 2 * (1 - k / 0.3) : 0; break;
      case 'editorial': dx = -Math.min(12, w / 5) * rest; break;
      case 'mono': optics = 'edge'; dx = -2 * rest; break;
      case 'cinema': dy = 6 * rest; a.opacity = Math.min(1, k * 2); break;
      case 'playbill': dx = side * 8 * rest; break;
      case 'gala': optics = 'light'; dy = 3 * rest; break;
      case 'broadsheet': optics = 'press'; break;
      case 'crt': optics = 'phosphor'; break;
      case 'arcade': dy = -Math.ceil(rest * 3) * 2; break;
      case 'neon': optics = 'tube'; break;
      case 'poster': dy = -3 * rest; angle = -0.7 * rest; break;
      case 'ink': optics = 'wet'; break;
      case 'ukiyoe': dx = (unit % 2 ? 1 : -1) * 2.5 * rest; optics = 'press'; break;
      case 'woodblock': optics = 'press'; dy = -1.5 * rest; break;
      case 'baroque': dx = side * 4 * rest; angle = side * 0.6 * rest; break;
      case 'grimoire': optics = 'engrave'; break;
      case 'manuscript': dx = -2 * rest; break;
      case 'pastel': dx = side * 4 * rest; dy = -Math.sin(k * Math.PI); break;
      case 'cutout': dx = side * 4 * rest; dy = -3 * rest; angle = side * 1.4 * rest; break;
      case 'meadow': angle = side * Math.min(1.3, 120 / h) * rest; break;
      case 'manga': dx = 8 * rest; break;
      case 'cyber': dx = (unit % 2 ? 1 : -1) * Math.ceil(rest * 3) * 2; break;
    }
    if (dx || dy || angle) a.transform = `translate(${r(dx)} ${r(dy)})${angle ? ` rotate(${r(angle)} ${r(x)} ${r(y + h)})` : ''}`;
    if (optics) {
      const blur = optics === 'focus' ? 1.8 * rest * rest : optics === 'wet' ? 0.35 * rest : 0;
      let effect = blur ? el('feGaussianBlur', { stdDeviation: r(blur) }) : '';
      if (optics === 'press') effect = el('feMorphology', { operator: 'dilate', radius: r(0.32 * rest) });
      if (optics === 'edge') effect = el('feMorphology', { operator: 'erode', radius: r(0.68 * rest) });
      if (optics === 'engrave') effect = el('feMorphology', { in: 'SourceGraphic', operator: 'erode', radius: 0.7, result: 'inside' })
        + el('feComposite', { in: 'SourceGraphic', in2: 'inside', operator: 'out', result: 'edge' })
        + el('feComponentTransfer', { in: 'SourceGraphic', result: 'ink' }, el('feFuncA', { type: 'linear', slope: r(k) }))
        + el('feMerge', {}, el('feMergeNode', { in: 'edge' }), el('feMergeNode', { in: 'ink' }));
      if (optics === 'exposure' || optics === 'light') effect = el('feComponentTransfer', {}, ...['R', 'G', 'B'].map(c => el(`feFunc${c}`, { type: 'linear', slope: r(optics === 'light' ? 1 + 0.5 * Math.sin(Math.PI * k) : 0.55 + 0.45 * k) })));
      if (optics === 'phosphor' || optics === 'tube') effect = el('feGaussianBlur', { stdDeviation: r((optics === 'tube' ? 2 : 0.8) * rest), result: 'halo' }) + el('feMerge', {}, el('feMergeNode', { in: 'halo' }), el('feMergeNode', { in: 'SourceGraphic' }));
      a.filter = s.url(`code-${unit}`, fid => el('filter', { id: fid, filterUnits: 'userSpaceOnUse', x: x - 6, y: y - 6, width: w + 12, height: h + 12 }, effect));
    }
    drawGroup(s, a, () => draw(part));
  }
  draw(settled);
}

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
  st.title(s, T(m, '代码归属地图', 'Code ownership map'), x0, c.y + 116, 40, 'start', { maxW: pw * 0.66 });
  st.label(s, m.project, x1, c.y + 58, { anchor: 'end', size: 13, fill: st.c.faint });
  s.text(T(m, `${fmt(total)} 行 · ${fmt(files.length)} 个文件`, `${fmt(total)} lines · ${fmt(files.length)} files`), x1, c.y + 112, { font: st.f.mono, size: 15, fill: st.c.soft, anchor: 'end' });

  const color = (key: string) => (key === 'unattributed' || key === 'human' ? st.c.soft : colorOf(st, m.rows.find((a) => a.key === key) ?? m.rows.find((a) => a.key === 'other') ?? ({ key, rank: 5 } as Agent)));
  const neutral = (key: string) => key === 'unattributed' || key === 'human';
  // 按颜色 × 深浅归并成少数几条 path，文件小
  const layers = new Map<string, string[]>();
  const code: CodeStroke[] = [];
  const base: string[] = [];
  const progress = styleProgress(st, t, 0, 0.85);
  let cursor = { col: 0, row: 0, distance: Infinity };
  let col = 0, row = 0;
  for (const [file, f] of files.entries()) {
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
        const order = materialOrder(st, row * cols + col, rows * cols, cols);
        const distance = Math.abs(progress - order);
        if (distance < cursor.distance) cursor = { col, row, distance };
        for (let x = lx, first = true; x < end - 1.5; first = false) {
          const w = Math.min(end - x, cw / 7 + R() * (cw / 2.2));
          const shade = first ? 0 : R() < 0.45 ? 1 : R() < 0.5 ? 2 : 0;
          const d = `M${r(x)} ${y}h${r(w)}`;
          base.push(d);
          code.push({ d, key, shade, file, col, row, x, y, w });
          if (t >= 1) {
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
  // 底：全部的线，淡淡一层。已出现的整行保持原来的长度和归属。
  s.add(path(base.join(''), { fill: 'none', stroke: st.c.ink, strokeOpacity: 0.14, strokeWidth: 2 }));
  if (t < 1) codeUnits(s, st, progress, code, cols, rows, color, neutral);
  else materialEnter(s, st, 1, { x: x0 - 2, y: py - 2, w: pw + 4, h: plotH + 4 }, () => {
    for (const [lk, ds] of layers) {
      const [key, sh] = lk.split('|');
      s.add(path(ds.join(''), { fill: 'none', stroke: color(key), strokeOpacity: r(SHADE[+sh] * (neutral(key) ? 0.55 : 1)), strokeWidth: 2 }));
    }
  }, 'data');
  // 游标属于屏幕/书写语义，不给所有纸面作品套同一道扫描线。
  if (t > 0 && t < 0.85) {
    const x = x0 + cursor.col * (cw + GAP), y = py + cursor.row * PITCH;
    if (['terminal', 'arcade'].includes(st.id)) s.add(rect(x, y, Math.min(8, cw), 3, { fill: st.c.accent, opacity: 0.8 }));
    if (st.id === 'crt') s.add(rect(x0, y, pw, 2, { fill: st.c.accent, opacity: 0.65 }));
    if (['manuscript', 'editorial'].includes(st.id)) s.add(rect(x, y + 2, cw, 0.8, { fill: st.c.ink, opacity: 0.65 }));
    if (st.id === 'cyber') s.add(rect(x, y - 2, cw, PITCH + 2, { fill: 'none', stroke: st.c.accent, strokeWidth: 0.8, opacity: 0.7 }));
  }

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
  const reveal = styleProgress(st, t, 0.1, 0.85);
  // 这是 5px 高的真实份额刻度，不是代码铺版的扫描窗。
  // 整条保留所有归属的完整宽度；各风格的行/文件/色版动作在上方代码主体里。
  materialEnter(s, st, reveal, { x: x0, y: by, w: pw, h: 5 }, () => {
    for (const { a, pct } of shown) {
      const w = (pct / 100) * pw;
      if (w > 0) s.add(rect(bx, by, Math.max(0, w - 3), 5, { fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) }));
      bx += w;
    }
  }, 'data');
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
