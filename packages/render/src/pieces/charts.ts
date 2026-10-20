// 统计图：环形、华夫饼、周河流图、会话泳道、作息打卡、版本、留存、文件马赛克、日历、出处、板块。
// 颜色、条形、标记都走风格钩子，所以每张图在每个风格里都是那个风格的样子。
import { Scene, rect, line, path, circle, el, g, r, ease, arc, mix, alpha } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt, pct, compact } from '../data.ts';
import type { Style, Box } from '../style.ts';
import { T, colorOf, tip, canvas, plateCorner, handTip, keyRank, legendRows, type PieceOpts } from './common.ts';
import { segsOf, segTip } from './tier.ts';

/** 图的外框：标签 + 标题 + 图例，返回绘图区 */
function frame(m: Model, st: Style, W: number, plotH: number, kicker: string, title: string, legend = true): { s: Scene; p: Box } {
  const legendH = legend ? 50 : 0;
  // 没有图例时绘图区和下面的刻度一直排到底：底板角上有装饰就留出它的高度，刻度不压在角花上
  const corner = legend ? 0 : plateCorner(st, W);
  const { s, c } = canvas(m, st, W, Math.max(170 + plotH + legendH + 50, 160 + plotH + 36 + corner), 34);
  const x0 = c.x + 50, x1 = c.x + c.w - 50;
  // 底板角上有装饰时，贴角的表头和图例往里让，绘图区不动
  const hx0 = Math.max(x0, c.x + (c.corner ?? 0) + 14), hx1 = Math.min(x1, c.x + c.w - (c.corner ?? 0) - 14);
  st.label(s, kicker, hx0, c.y + 58, { size: 13 });
  st.title(s, title, hx0, c.y + 116, 40, 'start', { maxW: x1 - hx0 });
  st.label(s, m.project, hx1, c.y + 58, { anchor: 'end', size: 13, fill: st.c.faint });
  if (legend) {
    let x = hx0;
    const y = c.y + 170 + plotH + 40;
    for (const a of legendRows(m)) {
      if (x > hx1 - 120) break;
      if (a.key === 'unattributed') s.add(rect(x, y - 11, 14, 14, { fill: st.hatch(s), rx: 3 }));
      else s.add(rect(x, y - 11, 14, 14, { fill: colorOf(st, a), rx: Math.min(3, st.radius) }));
      const w = s.text(a.name, x + 22, y + 1, { font: st.f.body, size: 15, fill: st.c.soft });
      s.hit(x - 4, y - 18, w + 30, 28, tip(m, a));
      x += w + 46;
    }
  }
  return { s, p: { x: x0, y: c.y + 160, w: x1 - x0, h: plotH } };
}
/** 分组 key → 排名里的贡献方（人合并为 human） */
const agentOf = (m: Model, k: string): Agent | undefined => m.rows.find((a) => a.key === k) ?? (m.rows.find((a) => a.key === 'other') && !m.rows.some((a) => a.key === k) && k !== 'human' && k !== 'unattributed' ? m.rows.find((a) => a.key === 'other') : undefined);
const fillKey = (s: Scene, st: Style, m: Model, k: string) => (k === 'unattributed' ? st.hatch(s) : k === 'human' ? st.color('human', 0) : colorOf(st, agentOf(m, k) ?? { key: k, rank: 5 } as Agent));
const tipKey = (m: Model, k: string) => { const a = agentOf(m, k); return a ? tip(m, a) : k === 'human' ? handTip(m) : { k, t: k }; };
/** 一组数据里出现的 key，按排名排序；按模型分组时同一工具的模型挨在一起（堆叠图里成一整块，深浅相邻） */
const orderKeys = (m: Model, keys: Iterable<string>) => [...new Set(keys)].sort((a, b) => keyRank(m, a) - keyRank(m, b));

// ───────────────────────── 环形 ─────────────────────────

export const DONUT_VARIANTS = ['ring', 'nested'];

export function donut(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (o.variant === 'nested') return nested(m, st, o);
  if (!m.rows.length) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1000, 640, T(m, '占比', 'SHARE'), m.group === 'model' ? T(m, '各模型写了多少', 'Who wrote the code') : T(m, '各工具写了多少', 'Who wrote the code'));
  const cx = p.x + p.w * 0.36, cy = p.y + p.h / 2, R1 = 270, R0 = 170;
  let a0 = -Math.PI / 2;
  const cellSegs: { a0: number; a1: number; fill: string }[] = [];
  m.rows.forEach((a, i) => {
    const sweep = (a.pct / 100) * Math.PI * 2 * ease(t, i, m.rows.length, 0.8);
    const gap = Math.min(0.012, sweep / 4);
    if (st.grid && st.cells) cellSegs.push({ a0, a1: a0 + sweep, fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) });
    else if (sweep > 0) s.add(path(arc(cx, cy, R1, R0, a0 + gap, a0 + sweep - gap), { fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) }));
    const mid = a0 + sweep / 2;
    s.hit(cx + Math.cos(mid) * (R0 + R1) / 2 - 30, cy + Math.sin(mid) * (R0 + R1) / 2 - 30, 60, 60, tip(m, a));
    a0 += sweep;
  });
  if (cellSegs.length) gridRing(s, st, cx, cy, R0, R1, cellSegs);
  st.num(s, fmt(m.linesAdded), cx, cy + 6, 46, 'middle');
  s.text(T(m, '行', 'lines'), cx, cy + 44, { font: st.f.body, size: 18, fill: st.c.soft, anchor: 'middle' });
  // 右侧名单
  let y = p.y + 80;
  for (const a of m.rows) {
    st.mark(s, a.key, p.x + p.w * 0.74, y - 8, 26);
    s.text(a.name, p.x + p.w * 0.74 + 24, y, { font: st.f.body, size: 18, fill: st.c.ink, maxW: 150 });
    st.num(s, pct(a.pct), p.x + p.w, y, 18, 'end', { fill: st.c.soft });
    s.hit(p.x + p.w * 0.72, y - 26, p.w * 0.28, 40, tip(m, a));
    y += 62;
  }
  return s;
}

/** 字符格风格（原版终端）：环落到格子上——格子中心在环里的，按角度归到那一段；同一行里挨着的同色格子合成一段画 */
function gridRing(s: Scene, st: Style, cx: number, cy: number, R0: number, R1: number, segs: { a0: number; a1: number; fill: string }[]) {
  const G = st.grid!;
  for (let ky = Math.floor((cy - R1) / G); ky * G <= cy + R1; ky++) {
    let x0 = 0, n = 0, cur = '';
    const flush = () => { if (n) st.cells!(s, x0, ky * G, n * G, G, cur); n = 0; };
    for (let kx = Math.floor((cx - R1) / G); kx * G <= cx + R1; kx++) {
      const dx = (kx + 0.5) * G - cx, dy = (ky + 0.5) * G - cy, d = Math.hypot(dx, dy);
      let a = Math.atan2(dy, dx);
      if (a < -Math.PI / 2) a += Math.PI * 2;
      const f = d >= R0 && d <= R1 ? segs.find((q) => a >= q.a0 && a < q.a1)?.fill ?? '' : '';
      if (n && f === cur) { n++; continue; }
      flush();
      if (f) { x0 = kx * G; n = 1; cur = f; }
    }
    flush();
  }
}

/** 内外环：内环是工具，外环是各工具下的模型，和内环对齐 */
function nested(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (!m.tools.length) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1000, 640, T(m, '占比', 'SHARE'), T(m, '工具和模型', 'Tools and models'), false);
  const cx = p.x + p.w * 0.31, cy = p.y + p.h / 2;
  // 字符格风格：两环之间至少空出一格，不然格子连成一圈分不出内外
  const grid = !!(st.grid && st.cells);
  const [Ri0, Ri1, Ro0, Ro1] = grid ? [140, 194, 214, 262] : [140, 200, 208, 262];
  const inner: { a0: number; a1: number; fill: string }[] = [], outer: typeof inner = [];
  let a0 = -Math.PI / 2;
  m.tools.forEach((a, i) => {
    const sweep = (a.pct / 100) * Math.PI * 2 * ease(t, i, m.tools.length, 0.8);
    const gap = Math.min(0.012, sweep / 4);
    const unk = a.key === 'unattributed';
    if (grid) inner.push({ a0, a1: a0 + sweep, fill: unk ? st.hatch(s) : colorOf(st, a) });
    else if (sweep > 0) s.add(path(arc(cx, cy, Ri1, Ri0, a0 + gap, a0 + sweep - gap), { fill: unk ? st.hatch(s) : colorOf(st, a) }));
    const mid = a0 + sweep / 2;
    s.hit(cx + Math.cos(mid) * (Ri0 + Ri1) / 2 - 26, cy + Math.sin(mid) * (Ri0 + Ri1) / 2 - 26, 52, 52, tip(m, a));
    // 外环：这一段按模型再切
    let b0 = a0;
    for (const q of segsOf(m, st, a)) {
      const sw = sweep * (q.lines / Math.max(1, a.lines));
      const gg = Math.min(0.01, sw / 4);
      if (grid) outer.push({ a0: b0, a1: b0 + sw, fill: unk ? st.hatch(s) : q.color });
      else if (sw > 0) s.add(path(arc(cx, cy, Ro1, Ro0, b0 + gg, b0 + sw - gg), { fill: unk ? st.hatch(s) : q.color }));
      const qm = b0 + sw / 2;
      if (sw > 0.04) s.hit(cx + Math.cos(qm) * (Ro0 + Ro1) / 2 - 22, cy + Math.sin(qm) * (Ro0 + Ro1) / 2 - 22, 44, 44, segTip(m, a, q));
      b0 += sw;
    }
    a0 += sweep;
  });
  if (grid) { gridRing(s, st, cx, cy, Ri0, Ri1, inner); gridRing(s, st, cx, cy, Ro0, Ro1, outer); }
  st.num(s, fmt(m.linesAdded), cx, cy + 6, 40, 'middle');
  s.text(T(m, '行', 'lines'), cx, cy + 40, { font: st.f.body, size: 17, fill: st.c.soft, anchor: 'middle' });
  // 右侧：工具一行，下面缩进写它的模型
  toolLegend(s, m, st, p.x + p.w * 0.68, p.x + p.w, p.y + 40, p.y + p.h);
  return s;
}

/** 工具和模型的名单：工具一行，下面缩进写它的各个模型（内外环、分块华夫饼共用） */
function toolLegend(s: Scene, m: Model, st: Style, lx: number, rx: number, y: number, maxY: number) {
  for (const a of m.tools) {
    if (y > maxY) break;
    st.mark(s, a.key, lx + 10, y - 8, 24);
    s.text(a.name, lx + 32, y, { font: st.f.body, size: 18, fill: st.c.ink, maxW: rx - lx - 110 });
    st.num(s, pct(a.pct), rx, y, 18, 'end', { fill: st.c.soft });
    s.hit(lx - 4, y - 24, rx - lx + 8, 34, tip(m, a));
    y += 34;
    const segs = segsOf(m, st, a);
    if (segs.length > 1) for (const q of segs) {
      s.add(rect(lx + 32, y - 11, 10, 10, { fill: q.color, rx: Math.min(2, st.radius) }));
      s.text(q.name, lx + 50, y, { font: st.f.body, size: 14, fill: st.c.soft, maxW: rx - lx - 120 });
      s.text(pct(q.pct), rx, y, { font: st.f.body, size: 14, fill: st.c.soft, anchor: 'end' });
      s.hit(lx + 28, y - 18, rx - lx - 28, 26, segTip(m, a, q));
      y += 26;
    }
    y += 16;
  }
}

// ───────────────────────── 华夫饼：一格 1% ─────────────────────────

export const WAFFLE_VARIANTS = ['grid', 'blocks'];

export function waffle(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (o.variant === 'blocks') return blocks(m, st, o);
  if (!m.rows.length) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1000, 560, T(m, '一格 = 1%', 'ONE CELL = 1%'), T(m, '一百格', 'A hundred cells'));
  // 最大余数法分配 100 格
  const raw = m.rows.map((a) => a.pct);
  const base = raw.map(Math.floor);
  let left = 100 - base.reduce((x, y) => x + y, 0);
  raw.map((v, i) => [v - base[i], i]).sort((a, b) => b[0] - a[0]).forEach(([, i]) => { if (left-- > 0) base[i]++; });
  const cells: Agent[] = [];
  m.rows.forEach((a, i) => { for (let k = 0; k < base[i]; k++) cells.push(a); });
  const size = Math.min(p.h / 10, p.w / 10) - 6, gap = 6;
  const gx = p.x + (p.w - 10 * (size + gap)) / 2;
  cells.forEach((a, i) => {
    const col = Math.floor(i / 10), row = i % 10;
    const x = gx + col * (size + gap), y = p.y + row * (size + gap);
    const k = ease(t, i, 100, 0.3);
    if (k <= 0) return;
    const ss = size * k;
    if (a.key === 'unattributed') s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill: st.hatch(s), rx: Math.min(st.radius, ss / 4) }));
    else s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill: colorOf(st, a), rx: Math.min(st.radius, ss / 4) }));
    s.hit(x, y, size, size, tip(m, a));
  });
  return s;
}

/** 分块：一百格按工具连成一块，块里按模型分深浅，块与块之间空一格；右边是工具和模型的名单 */
function blocks(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (!m.tools.length) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1000, 560, T(m, '一格 = 1%', 'ONE CELL = 1%'), T(m, '工具和模型', 'Tools and models'), false);
  // 先按段（工具下的模型）用最大余数法分 100 格
  const segs = m.tools.flatMap((a) => segsOf(m, st, a).map((q) => ({ a, q })));
  const raw = segs.map((x) => x.q.pct);
  const base = raw.map(Math.floor);
  let left = 100 - base.reduce((x, y) => x + y, 0);
  raw.map((v, i) => [v - base[i], i]).sort((a, b) => b[0] - a[0]).forEach(([, i]) => { if (left-- > 0) base[i]++; });
  // 一列 10 格，从上往下、从左往右排；每个工具从新的一列开始，和前一个工具隔开一列的缝
  type Cell = { a: Agent; q: (typeof segs)[number]['q'] } | null;
  const rows = 10;
  const cells: Cell[] = [];
  const breaks: number[] = [];
  segs.forEach((x, i) => {
    if (i > 0 && segs[i - 1].a !== x.a && base[i] > 0 && cells.length) {
      while (cells.length % rows) cells.push(null);
      breaks.push(cells.length / rows);
    }
    for (let k = 0; k < base[i]; k++) cells.push(x);
  });
  const cols = Math.ceil(cells.length / rows);
  const gw = p.w * 0.62, gap = 5, split = 14;
  const size = Math.min(p.h / rows - gap, (gw - breaks.length * split) / cols - gap);
  const gx = p.x, gy = p.y + (p.h - rows * (size + gap)) / 2;
  const colX = (col: number) => gx + col * (size + gap) + breaks.filter((b) => b <= col).length * split;
  let n = 0;
  cells.forEach((c, i) => {
    if (!c) return;
    const col = Math.floor(i / rows), row = i % rows;
    const x = colX(col), y = gy + row * (size + gap);
    const k = ease(t, n++, 100, 0.3);
    if (k <= 0) return;
    const ss = size * k;
    const fill = c.a.key === 'unattributed' ? st.hatch(s) : c.q.color;
    s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill, rx: Math.min(st.radius, ss / 4) }));
    s.hit(x, y, size, size, segsOf(m, st, c.a).length > 1 ? segTip(m, c.a, c.q) : tip(m, c.a));
  });
  toolLegend(s, m, st, p.x + p.w * 0.68, p.x + p.w, p.y + 40, p.y + p.h);
  return s;
}

// ───────────────────────── 周河流图 ─────────────────────────

export function stream(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.weeks.length < 3) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1400, 520, T(m, '每周新增', 'PER WEEK'), T(m, '每周新增的代码', 'Lines added per week'));
  const keys = orderKeys(m, m.weeks.flatMap((w) => Object.keys(w.by)));
  const tot = m.weeks.map((w) => keys.reduce((s2, k) => s2 + (w.by[k] ?? 0), 0));
  const max = Math.max(...tot, 1);
  const n = m.weeks.length;
  const X = (i: number) => p.x + (i / (n - 1)) * p.w;
  const mid = p.y + p.h / 2;
  // 河流：以中线对称
  const base = m.weeks.map((_, i) => mid - (tot[i] / max) * (p.h / 2) * ease(t, i, n, 0.5));
  const acc = [...base];
  const smooth = (pts: [number, number][]) => pts.map(([x, y], i) => {
    if (i === 0) return `M${r(x)} ${r(y)}`;
    const [px, py] = pts[i - 1];
    const cxm = (px + x) / 2;
    return `C${r(cxm)} ${r(py)} ${r(cxm)} ${r(y)} ${r(x)} ${r(y)}`;
  }).join('');
  // 字符格风格（原版终端）：每一列按两周之间插值的数堆起来，落到格子上，一格一个方块字；算法和平滑的河流一样，只是笔触不同
  const G = st.grid;
  if (G && st.cells) {
    const r0 = Math.ceil(p.y / G), nr = Math.floor((p.y + p.h) / G) - r0;
    for (let c = Math.ceil((p.x - G / 2) / G); (c + 0.5) * G <= p.x + p.w; c++) {
      const u = Math.min(n - 1, Math.max(0, (((c + 0.5) * G - p.x) / p.w) * (n - 1))), i = Math.min(n - 2, Math.floor(u)), f = u - i;
      const e = ease(t, u, n, 0.5);
      let cum = 0;
      const b = keys.map((k) => (cum += (m.weeks[i].by[k] ?? 0) * (1 - f) + (m.weeks[i + 1].by[k] ?? 0) * f, Math.round((cum / max) * nr * e)));
      const top = r0 + Math.floor((nr - b[b.length - 1]) / 2);
      keys.forEach((k, j) => {
        const a = j ? b[j - 1] : 0;
        if (b[j] > a) st.cells!(s, c * G, (top + a) * G, G, (b[j] - a) * G, fillKey(s, st, m, k));
      });
    }
  }
  for (const k of G && st.cells ? [] : keys) {
    const top = acc.map((y, i) => [X(i), y] as [number, number]);
    const bot = acc.map((y, i) => [X(i), y + (((m.weeks[i].by[k] ?? 0) / max) * p.h) * ease(t, i, n, 0.5)] as [number, number]);
    bot.forEach(([, y], i) => (acc[i] = y));
    const d = smooth(top) + smooth([...bot].reverse()).replace(/^M/, 'L') + 'Z';
    s.add(path(d, { fill: fillKey(s, st, m, k), opacity: 0.92, stroke: st.c.bg, strokeWidth: 0.6 }));
  }
  // 每周的悬停区
  m.weeks.forEach((w, i) => {
    const lines = keys.filter((k) => w.by[k]).sort((a, b) => w.by[b] - w.by[a]).slice(0, 4).map((k) => `${agentOf(m, k)?.name ?? (k === 'human' ? T(m, '人', 'People') : k)} ${fmt(w.by[k])}`);
    s.hit(X(i) - p.w / n / 2, p.y, p.w / n, p.h, { t: w.week, d: [T(m, `${fmt(tot[i])} 行`, `${fmt(tot[i])} lines`), ...lines] });
  });
  s.text(m.weeks[0].week, p.x, p.y + p.h + 26, { font: st.f.label, size: 13, fill: st.c.faint });
  s.text(m.weeks[n - 1].week, p.x + p.w, p.y + p.h + 26, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end' });
  return s;
}

// ───────────────────────── 会话泳道 ─────────────────────────

export function lanes(m: Model, st: Style, o: PieceOpts): Scene | null {
  // 结束早于开始的会话（自动 review 的记录里有）按开始那一刻画一下，时间轴不被它撑出绘图区
  const ss = m.timeline.filter((x) => x.start && x.end).map((x) => (x.end < x.start ? { ...x, end: x.start } : x));
  if (ss.length < 3) return null;
  const t = o.t ?? 1;
  const keys = orderKeys(m, ss.map((x) => x.key));
  const laneH = 64;
  const { s, p } = frame(m, st, 1400, keys.length * laneH, T(m, '会话', 'SESSIONS'), m.group === 'model' ? T(m, '各模型的会话', 'Sessions by model') : T(m, '各工具的会话', 'Sessions by tool'), false);
  const t0 = Math.min(...ss.map((x) => x.start)), t1 = Math.max(...ss.map((x) => x.end));
  const X = (v: number) => p.x + 170 + ((v - t0) / Math.max(1, t1 - t0)) * (p.w - 170);
  keys.forEach((k, li) => {
    const y = p.y + li * laneH;
    const a = m.agents.find((x) => x.key === k);
    st.mark(s, k, p.x + 16, y + laneH / 2, 24);
    s.text(a?.name ?? k, p.x + 38, y + laneH / 2 + 6, { font: st.f.body, size: 16, fill: st.c.ink, maxW: 125 });
    st.track(s, p.x + 170, y + laneH / 2 - 1, p.w - 170, 2);
    const mine = ss.filter((x) => x.key === k);
    mine.forEach((x, i) => {
      if (ease(t, i, mine.length) <= 0) return;
      const x0 = X(x.start), w = Math.max(3, X(x.end) - x0);
      const col = a ? colorOf(st, m.rows.find((q) => q.key === k) ?? a) : st.c.soft;
      s.add(rect(x0, y + 16, w, laneH - 32, { fill: x.review ? 'none' : col, stroke: x.review ? col : 'none', strokeWidth: 1.4, rx: Math.min(st.radius, 4), opacity: 0.9 }));
    });
    s.hit(p.x, y, p.w, laneH, a ? tip(m, a) : { t: k });
  });
  const d = (v: number) => new Date(v * 1000).toISOString().slice(0, 10);
  s.text(d(t0), p.x + 170, p.y + p.h + 28, { font: st.f.label, size: 13, fill: st.c.faint });
  s.text(d(t1), p.x + p.w, p.y + p.h + 28, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end' });
  return s;
}

// ───────────────────────── 作息打卡 ─────────────────────────

export function clock(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.clock.length !== 7) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1400, 420, T(m, '星期 × 小时', 'DAY × HOUR'), T(m, '提交时间', 'When commits happen'), false);
  const max = Math.max(...m.clock.flat(), 1);
  const days = T(m, '一二三四五六日', 'MTWTFSS');
  const cw = (p.w - 60) / 24, ch = p.h / 7;
  m.clock.forEach((row, d) => {
    s.text(m.lang === 'zh' ? `周${days[d]}` : days[d], p.x, p.y + d * ch + ch / 2 + 6, { font: st.f.label, size: 14, fill: st.c.soft });
    row.forEach((v, hr) => {
      const cx = p.x + 60 + hr * cw + cw / 2, cy = p.y + d * ch + ch / 2;
      const rr = (Math.sqrt(v / max) * Math.min(cw, ch) * 0.46) * ease(t, d * 24 + hr, 168, 0.3);
      if (v) s.add(circle(cx, cy, Math.max(1.5, rr), { fill: st.c.accent, opacity: 0.25 + 0.75 * (v / max) }));
      else s.add(circle(cx, cy, 1.2, { fill: st.c.faint, opacity: 0.5 }));
      if (v) s.hit(cx - cw / 2, cy - ch / 2, cw, ch, { t: `${m.lang === 'zh' ? '周' + days[d] : days[d]} ${hr}:00`, d: [T(m, `${v} 次提交`, `${v} commits`)] });
    });
  });
  for (const hr of [0, 6, 12, 18, 23]) s.text(`${hr}:00`, p.x + 60 + hr * cw + cw / 2, p.y + p.h + 24, { font: st.f.label, size: 12, fill: st.c.faint, anchor: 'middle' });
  return s;
}

// ───────────────────────── 版本 ─────────────────────────

export function versions(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.versions.length < 2) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1200, 520, T(m, '版本', 'RELEASES'), T(m, '各版本由谁写', 'Who wrote each release'));
  const keys = orderKeys(m, m.versions.flatMap((v) => Object.keys(v.by)));
  const max = Math.max(...m.versions.map((v) => v.total), 1);
  const n = m.versions.length, bw = Math.min(120, (p.w / n) * 0.55);
  m.versions.forEach((v, i) => {
    const x = p.x + (i + 0.5) * (p.w / n) - bw / 2;
    let y = p.y + p.h;
    const k = ease(t, i, n);
    for (const key of keys) {
      const h = ((v.by[key] ?? 0) / max) * p.h * k;
      if (h <= 0) continue;
      y -= h;
      s.add(rect(x, y, bw, Math.max(0, h - 1.5), { fill: fillKey(s, st, m, key) }));
    }
    s.text(v.tag === 'unreleased' ? T(m, '未发布', 'unreleased') : v.tag, x + bw / 2, p.y + p.h + 28, { font: st.f.label, size: 14, fill: st.c.soft, anchor: 'middle' });
    st.num(s, compact(v.total), x + bw / 2, y - 12, 16, 'middle', { fill: st.c.soft });
    s.hit(x - 10, p.y, bw + 20, p.h + 40, { t: v.tag, d: [v.date, T(m, `${fmt(v.total)} 行`, `${fmt(v.total)} lines`), ...keys.filter((q) => v.by[q]).slice(0, 4).map((q) => `${agentOf(m, q)?.name ?? q} ${Math.round((v.by[q] / v.total) * 100)}%`)] });
  });
  st.rule(s, p.x, p.y + p.h, p.w, 'thin');
  return s;
}

// ───────────────────────── 留存：写下的留下多少 ─────────────────────────

export function survival(m: Model, st: Style, o: PieceOpts): Scene | null {
  const list = m.rows.filter((a) => a.kind === 'agent' && a.lines > 0);
  if (!list.length || !m.linesSurviving) return null;
  const t = o.t ?? 1;
  const rowH = 74;
  const { s, p } = frame(m, st, 1200, list.length * rowH, T(m, '留存', 'RETENTION'), T(m, '写的代码现在还剩多少', 'How much of it is still here'), false);
  const max = Math.max(...list.map((a) => a.lines));
  list.forEach((a, i) => {
    const y = p.y + i * rowH;
    const k = ease(t, i, list.length);
    st.mark(s, a.key, p.x + 14, y + 22, 24);
    s.text(a.name, p.x + 36, y + 30, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 170 });
    const x0 = p.x + 230, w = p.w - 330;
    const wa = (a.lines / max) * w * k, ws = (a.surviving / max) * w * k;
    // 字符格风格：写过、已经不在的那一截是一排点（原版的 ···）
    if (st.cells) st.track(s, x0, y + 10, wa, 26);
    else s.add(rect(x0, y + 10, wa, 26, { fill: colorOf(st, a), opacity: 0.25, rx: Math.min(st.radius, 6) }));
    st.bar(s, x0, y + 10, Math.max(2, ws), 26, colorOf(st, a), i);
    st.num(s, `${Math.round(a.survival * 100)}%`, p.x + p.w, y + 32, 22, 'end');
    s.hit(p.x, y, p.w, rowH - 6, { ...tip(m, a), d: [T(m, `写了 ${fmt(a.lines)} 行，现在还在 ${fmt(a.surviving)} 行`, `${fmt(a.lines)} written, ${fmt(a.surviving)} remain`), ...tip(m, a).d.slice(1)] });
  });
  return s;
}

// ───────────────────────── 文件马赛克（矩形树图） ─────────────────────────

export function mosaic(m: Model, st: Style, o: PieceOpts): Scene | null {
  const files = m.files.filter((f) => f.lines > 0).sort((a, b) => b.lines - a.lines).slice(0, 60);
  if (files.length < 3) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1200, 640, T(m, '文件', 'FILES'), T(m, '每个文件是谁写的', 'Who wrote each file'));
  const total = files.reduce((x, f) => x + f.lines, 0);
  const rects = squarify(files.map((f) => f.lines / total * p.w * p.h), { ...p });
  rects.forEach((b, i) => {
    const f = files[i];
    const k = ease(t, i, rects.length, 0.4);
    if (k <= 0) return;
    // 每个文件内部按贡献方竖切
    const keys = orderKeys(m, Object.keys(f.by));
    let x = b.x;
    for (const key of keys) {
      const w = ((f.by[key] ?? 0) / f.lines) * b.w;
      if (w > 0) s.add(rect(x, b.y, w, b.h, { fill: fillKey(s, st, m, key), opacity: 0.35 + 0.6 * k }));
      x += w;
    }
    s.add(rect(b.x, b.y, b.w, b.h, { fill: 'none', stroke: st.c.bg, strokeWidth: 2 }));
    if (b.w > 120 && b.h > 34 && f.path) s.text(f.path.split('/').pop()!, b.x + 10, b.y + 22, { font: st.f.mono, size: 13, fill: st.on(fillKey(s, st, m, keys[0])), maxW: b.w - 20, overflow: 'clip' });
    s.hit(b.x, b.y, b.w, b.h, { t: f.path || T(m, '文件', 'file'), d: [T(m, `${fmt(f.lines)} 行`, `${fmt(f.lines)} lines`), ...keys.filter((q) => f.by[q]).slice(0, 4).map((q) => `${agentOf(m, q)?.name ?? (q === 'human' ? T(m, '人', 'People') : q)} ${Math.round((f.by[q] / f.lines) * 100)}%`)] });
  });
  return s;
}
function squarify(areas: number[], b: Box): Box[] {
  const out: Box[] = [];
  let box = { ...b };
  let i = 0;
  while (i < areas.length) {
    const short = Math.min(box.w, box.h);
    let row: number[] = [areas[i]];
    let worst = Infinity;
    const score = (rw: number[]) => { const s2 = rw.reduce((x, y) => x + y, 0); return Math.max(...rw.map((a) => Math.max((short * short * a) / (s2 * s2), (s2 * s2) / (short * short * a)))); };
    worst = score(row);
    let j = i + 1;
    while (j < areas.length) { const nr = [...row, areas[j]]; const sc = score(nr); if (sc > worst) break; row = nr; worst = sc; j++; }
    const sum = row.reduce((x, y) => x + y, 0);
    if (box.w >= box.h) {
      const w = sum / box.h; let y = box.y;
      for (const a of row) { const h = a / w; out.push({ x: box.x, y, w, h }); y += h; }
      box = { x: box.x + w, y: box.y, w: box.w - w, h: box.h };
    } else {
      const h = sum / box.w; let x = box.x;
      for (const a of row) { const w = a / h; out.push({ x, y: box.y, w, h }); x += w; }
      box = { x: box.x, y: box.y + h, w: box.w, h: box.h - h };
    }
    i = j;
  }
  return out;
}

// ───────────────────────── 日历 ─────────────────────────

export function calendar(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.days.length < 5) return null;
  const t = o.t ?? 1;
  const first = new Date(m.days[0].date + 'T00:00:00Z'), last = new Date(m.days[m.days.length - 1].date + 'T00:00:00Z');
  const start = new Date(first); start.setUTCDate(start.getUTCDate() - ((start.getUTCDay() + 6) % 7));
  const weeks = Math.ceil(((last.getTime() - start.getTime()) / 864e5 + 1) / 7);
  const cell = Math.min(26, 1100 / weeks - 4);
  const { s, p } = frame(m, st, Math.max(900, weeks * (cell + 4) + 230), 7 * (cell + 4), T(m, '日历', 'CALENDAR'), T(m, '每天主要是谁在写', 'Who wrote most each day'));
  const byDate = new Map(m.days.map((d) => [d.date, d.by]));
  const max = Math.max(...m.days.map((d) => Object.values(d.by).reduce((x, y) => x + y, 0)), 1);
  for (let w = 0; w < weeks; w++) for (let d = 0; d < 7; d++) {
    const day = new Date(start.getTime() + (w * 7 + d) * 864e5).toISOString().slice(0, 10);
    const by = byDate.get(day);
    const x = p.x + w * (cell + 4), y = p.y + d * (cell + 4);
    const tot = by ? Object.values(by).reduce((a, b) => a + b, 0) : 0;
    if (!tot) { s.add(rect(x, y, cell, cell, { fill: st.c.ink, opacity: 0.05, rx: Math.min(st.radius, 4) })); continue; }
    const top = Object.entries(by!).sort((a, b) => b[1] - a[1])[0][0];
    const k = ease(t, w, weeks);
    s.add(rect(x, y, cell, cell, { fill: fillKey(s, st, m, top), opacity: (0.3 + 0.7 * Math.sqrt(tot / max)) * k, rx: Math.min(st.radius, 4) }));
    s.hit(x, y, cell, cell, { k: top, t: day, d: [T(m, `${fmt(tot)} 行`, `${fmt(tot)} lines`), ...Object.entries(by!).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([q, v]) => `${agentOf(m, q)?.name ?? (q === 'human' ? T(m, '人', 'People') : q === 'unattributed' ? T(m, '未确认', 'Unattributed') : q)} ${fmt(v)}`)] });
  }
  return s;
}

void line; void el; void g; void mix;

// ───────────────────────── 出处：每一行凭什么算它的 ─────────────────────────

/** 三种证据各用同一个颜色的三种浓度：逐行证据最实，推断最淡 */
const PROOF: { k: 'verified' | 'declared' | 'inferred'; o: number; zh: string; en: string }[] = [
  { k: 'verified', o: 1, zh: '逐行证据', en: 'Line evidence' },
  { k: 'declared', o: 0.55, zh: '明确声明', en: 'Declared' },
  { k: 'inferred', o: 0.25, zh: '推断', en: 'Inferred' },
];

export function evidence(m: Model, st: Style, o: PieceOpts): Scene | null {
  const list = m.rows.filter((a) => a.lines > 0 && (a.key === 'unattributed' || (a.proof && a.proof.verified + a.proof.declared + a.proof.inferred > 0)));
  if (!list.some((a) => a.key !== 'unattributed')) return null;
  const t = o.t ?? 1;
  const rowH = 74;
  const head = 150;
  const { s, p } = frame(m, st, 1200, head + list.length * rowH + 40, T(m, '出处', 'EVIDENCE'), T(m, '每一行凭什么算它的', 'What each line rests on'), false);
  // 头条：全部 AI 行里有逐行证据的比例
  st.num(s, `${Math.round(m.verifiedShare * 100 * ease(t))}%`, p.x, p.y + 64, 64, 'start');
  s.text(T(m, '的行有逐行证据：日志里记着这一行，提交里也有这一行', 'of the lines have line evidence: a log wrote the line, and a commit has it'), p.x, p.y + 104, { font: st.f.body, size: 17, fill: st.c.soft, maxW: p.w });
  const max = Math.max(...list.map((a) => a.lines));
  const x0 = p.x + 230, w = p.w - 330;
  list.forEach((a, i) => {
    const y = p.y + head + i * rowH;
    const k = ease(t, i, list.length);
    st.mark(s, a.key, p.x + 14, y + 22, 24);
    s.text(a.name, p.x + 36, y + 30, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 170 });
    const full = (a.lines / max) * w * k;
    if (a.key === 'unattributed') {
      if (st.cells) st.bar(s, x0, y + 10, Math.max(2, full), 26, st.hatch(s), i);
      else s.add(rect(x0, y + 10, Math.max(2, full), 26, { fill: st.hatch(s), rx: Math.min(st.radius, 6) }));
      s.text(T(m, '没有证据', 'no evidence'), p.x + p.w, y + 30, { font: st.f.body, size: 15, fill: st.c.faint, anchor: 'end' });
      s.hit(p.x, y, p.w, rowH - 6, tip(m, a));
      return;
    }
    const pf = a.proof!;
    const tot = pf.verified + pf.declared + pf.inferred || 1;
    let x = x0;
    PROOF.forEach((q, j) => {
      const ww = (pf[q.k] / tot) * full;
      if (ww <= 0) return;
      if (j === 0) st.bar(s, x, y + 10, Math.max(2, ww), 26, colorOf(st, a), i);
      // 字符格风格：浅的两层也是方块字，按浓淡调透明
      else if (st.cells) st.bar(s, x, y + 10, ww, 26, alpha(colorOf(st, a), q.o), i);
      else s.add(rect(x, y + 10, ww, 26, { fill: colorOf(st, a), opacity: q.o, rx: Math.min(st.radius, 6) }));
      x += ww;
    });
    st.num(s, `${Math.round((pf.verified / tot) * 100)}%`, p.x + p.w, y + 32, 22, 'end');
    s.hit(p.x, y, p.w, rowH - 6, { ...tip(m, a), d: PROOF.filter((q) => pf[q.k] > 0).map((q) => T(m, `${q.zh} ${fmt(pf[q.k])} 行`, `${q.en}: ${fmt(pf[q.k])} lines`)) });
  });
  // 图例：浓淡说明
  let lx = x0;
  const ly = p.y + head + list.length * rowH + 20;
  for (const q of [...PROOF.map((q) => ({ t: T(m, q.zh, q.en), o: q.o, h: false })), ...(list.some((a) => a.key === 'unattributed') ? [{ t: T(m, '未确认', 'Unattributed'), o: 1, h: true }] : [])]) {
    s.add(rect(lx, ly - 11, 14, 14, { fill: q.h ? st.hatch(s) : st.c.ink, opacity: q.o, rx: Math.min(3, st.radius) }));
    lx += s.text(q.t, lx + 22, ly + 1, { font: st.f.body, size: 15, fill: st.c.soft }) + 46;
  }
  return s;
}

// ───────────────────────── 板块：哪一块是谁写的 ─────────────────────────

const AREAS: [string, string, string][] = [
  ['core', '核心', 'Core'], ['frontend', '界面', 'Interface'], ['tests', '测试', 'Tests'], ['docs', '文档', 'Docs'], ['build', '构建与发布', 'Build & release'], ['assets', '素材', 'Assets'],
];

export function areas(m: Model, st: Style, o: PieceOpts): Scene | null {
  // 只写 AI：人写的行不进图，每块的占比按其余的行算
  const rows = AREAS.map(([k, z, e]) => {
    const by = Object.entries(m.areas[k] ?? {}).filter(([key, v]) => key !== 'human' && v > 0);
    return { k, name: T(m, z, e), by, tot: by.reduce((s, [, v]) => s + v, 0) };
  }).filter((r) => r.tot > 0);
  if (rows.length < 2) return null;
  const t = o.t ?? 1;
  const rowH = 86;
  const { s, p } = frame(m, st, 1200, rows.length * rowH, T(m, '板块', 'AREAS'), T(m, '哪一块是谁写的', 'Who wrote which part'));
  const x0 = p.x + 200, w = p.w - 200;
  rows.forEach((r, i) => {
    const y = p.y + i * rowH;
    const k = ease(t, i, rows.length);
    s.text(r.name, p.x, y + 30, { font: st.f.body, size: 19, fill: st.c.ink, maxW: 180 });
    s.text(T(m, `${fmt(r.tot)} 行`, `${fmt(r.tot)} lines`), p.x, y + 54, { font: st.f.body, size: 14, fill: st.c.faint });
    let x = x0;
    for (const key of orderKeys(m, r.by.map(([key]) => key))) {
      const v = r.by.find(([kk]) => kk === key)![1];
      const ww = (v / r.tot) * w * k;
      if (ww < 0.5) continue;
      if (st.block && (key !== 'unattributed' || st.cells)) st.block(s, x, y + 12, Math.max(1, ww - 2), 38, fillKey(s, st, m, key));
      else s.add(rect(x, y + 12, Math.max(1, ww - 2), 38, { fill: fillKey(s, st, m, key), rx: Math.min(st.radius, 6) }));
      // 百分比写在色块下面：色块的颜色由风格定，字压在上面不一定看得清
      if (ww > 52 && k >= 1) s.text(`${Math.round((v / r.tot) * 100)}%`, x + 2, y + 70, { font: st.f.body, size: 13, fill: st.c.soft });
      const tk = tipKey(m, key);
      s.hit(x, y + 8, ww, 46, { ...tk, d: [T(m, `${r.name}里 ${fmt(v)} 行，占 ${Math.round((v / r.tot) * 100)}%`, `${fmt(v)} lines in ${r.name}, ${Math.round((v / r.tot) * 100)}%`), ...(tk.d ?? []).slice(1)] });
      x += ww;
    }
  });
  return s;
}
