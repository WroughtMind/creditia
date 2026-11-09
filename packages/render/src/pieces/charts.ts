// 统计图：环形、华夫饼、周河流图、会话泳道、作息打卡、版本、留存、文件马赛克、日历、出处、板块。
// 颜色、条形、标记都走风格钩子，所以每张图在每个风格里都是那个风格的样子。
import { Scene, rect, line, path, circle, el, g, r, ease, arc, mix, alpha } from '../svg.ts';
import { phase, styleProgress, drawGroup, reveal } from '../motion.ts';
import { materialReveal, materialEnter, materialOrder, activityEnter } from '../material-motion.ts';
import type { Model, Agent } from '../data.ts';
import { fmt, pct, compact, nameOf } from '../data.ts';
import type { Style, Box } from '../style.ts';
import { T, colorOf, tip, canvas, plateCorner, handTip, keyRank, legendRows, type PieceOpts } from './common.ts';
import { segsOf, segTip } from './tier.ts';
import { historyLabels, historyEmpty } from '../history.ts';
import { valueBar } from './value-bar.ts';

/** 图的外框：标签 + 标题 + 图例，返回绘图区 */
function frame(m: Model, st: Style, W: number, plotH: number, kicker: string, title: string, legend = true, headerProgress = 1): { s: Scene; p: Box } {
  const legendH = legend ? 50 : 0;
  // 没有图例时绘图区和下面的刻度一直排到底：底板角上有装饰就留出它的高度，刻度不压在角花上
  const corner = legend ? 0 : plateCorner(st, W);
  const { s, c } = canvas(m, st, W, Math.max(170 + plotH + legendH + 50, 160 + plotH + 36 + corner), 34);
  const x0 = c.x + 50, x1 = c.x + c.w - 50;
  // 底板角上有装饰时，贴角的表头和图例往里让，绘图区不动
  const hx0 = Math.max(x0, c.x + (c.corner ?? 0) + 14), hx1 = Math.min(x1, c.x + c.w - (c.corner ?? 0) - 14);
  materialEnter(s, st, headerProgress, { x: hx0, y: c.y + 40, w: hx1 - hx0, h: 82 }, () => {
    st.label(s, kicker, hx0, c.y + 58, { size: 13 });
    st.title(s, title, hx0, c.y + 116, 40, 'start', { maxW: x1 - hx0 });
    st.label(s, m.project, hx1, c.y + 58, { anchor: 'end', size: 13, fill: st.c.faint });
  }, 'title');
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

/** 成对归位与剪纸落贴都搬动完整的真角度弧，不把一份贡献裁成临时小份额。 */
function illustratedArcOffset(st: Style, k: number, mid: number) {
  const rest = 1 - Math.max(0, Math.min(1, k));
  return st.id === 'pastel' ? { x: (Math.cos(mid) < 0 ? -1 : 1) * 18 * rest, y: 0 }
    : st.id === 'cutout' ? { x: Math.cos(mid) * 18 * rest, y: Math.sin(mid) * 18 * rest - 5 * rest }
    : { x: 0, y: 0 };
}

/** 份额编码在角度：沿环带厚度或完整印面成形，不把某个归属切成临时短弧。 */
function ringReveal(s: Scene, st: Style, k: number, cx: number, cy: number, inner: number, outer: number, draw: () => void, midAngle?: number) {
  if (k >= 1) { draw(); return; }
  const bounds = { x: cx - outer, y: cy - outer, w: outer * 2, h: outer * 2 };
  if (midAngle !== undefined && ['pastel', 'cutout'].includes(st.id)) {
    const { x, y } = illustratedArcOffset(st, k, midAngle);
    drawGroup(s, { transform: `translate(${r(x)} ${r(y)})` }, () => materialEnter(s, st, k, bounds, draw, 'data'));
    return;
  }
  if (['mono', 'neon', 'poster', 'manuscript', 'cutout', 'ukiyoe', 'pastel', 'meadow', 'manga', 'cyber', 'ink', 'baroque'].includes(st.id)) {
    materialEnter(s, st, k, bounds, draw, 'data');
    return;
  }
  if (['woodblock', 'grimoire'].includes(st.id)) {
    // 细小归属弧也先接触完整底墨；刀痕/交叉排线只改变表面，不抹掉一份角度。
    const wood = st.id === 'woodblock', pitch = wood ? 5 : 14;
    const marks = wood
      ? path(`M0 0H${r(pitch * k)}L${r(pitch * k * .8)} 9L${r(pitch * k)} 17V24H0Z`, { fill: '#fff' })
      : path(`M${-pitch} ${-pitch}L${pitch * 2} ${pitch * 2}M${-pitch} ${pitch * 2}L${pitch * 2} ${-pitch}`, { fill: 'none', stroke: '#fff', strokeWidth: pitch * k });
    const print = s.url(`ring-print-${inner}-${outer}`, id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width: pitch, height: wood ? 24 : pitch }, marks));
    const contact = wood ? .6 : .45, texture = wood ? .45 : .65;
    const mask = s.url(`ring-ink-${inner}-${outer}`, id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: bounds.x, y: bounds.y, width: bounds.w, height: bounds.h },
      rect(bounds.x, bounds.y, bounds.w, bounds.h, { fill: '#fff', opacity: contact * phase(k, 0, .18) + (1 - contact) * phase(k, .55, 1) })
      + rect(bounds.x, bounds.y, bounds.w, bounds.h, { fill: print, opacity: texture * phase(k, 0, .2) })));
    const first = s.hits.length;
    drawGroup(s, { mask }, draw);
    if (k <= 0) s.hits.splice(first);
    return;
  }
  if (st.id === 'broadsheet') {
    materialReveal(s, st, k, bounds, 'down', draw);
    return;
  }
  // 两层归属环的角度/层数从第一帧即完整。电影按乳剂表面成像，玻璃整层由虚焦清晰。
  if (st.id === 'film') { materialReveal(s, st, k, bounds, 'right', draw); return; }
  if (st.id === 'glass') { materialEnter(s, st, k, bounds, draw, 'data'); return; }
  const progress = ['arcade', 'cyber'].includes(st.id) ? Math.floor(k * 12) / 12 : k;
  const width = outer - inner, middle = (inner + outer) / 2;
  let lo = inner, hi = inner + width * progress, shape = '';
  if (['minimal-dark', 'meadow', 'manga', 'crt'].includes(st.id)) { lo = outer - width * progress; hi = outer; }
  else if (['cinema', 'playbill', 'gala', 'baroque', 'pastel', 'cyber'].includes(st.id)) {
    lo = middle - width * progress / 2; hi = middle + width * progress / 2;
  }
  // 临近完成让裁边退到原图形之外，避免撤掉裁窗时重复抗锯齿的边缘突然补实。
  const bleed = 2 * phase(k, .85, 1);
  const band = (from: number, to: number) => path(arc(cx, cy, to + bleed, Math.max(0, from - bleed), 0, Math.PI * 2));
  shape = band(lo, hi);
  const clipPath = s.url(`ring-${inner}-${outer}`, id => el('clipPath', { id }, shape));
  const first = s.hits.length;
  drawGroup(s, { clipPath }, () => materialEnter(s, st, k, bounds, draw, 'data'));
  if (progress <= 0) s.hits.splice(first);
}

export function donut(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (o.variant === 'nested') return nested(m, st, o);
  if (!m.rows.length) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1000, 640, T(m, '代码占比', 'SHARE'), m.group === 'model' ? T(m, '各模型占比', 'Shares by model') : T(m, '工具与模型占比', 'Shares by tool & model'));
  const cx = p.x + p.w * 0.36, cy = p.y + p.h / 2, R1 = 270, R0 = 170;
  let a0 = -Math.PI / 2;
  const cellSegs: { a0: number; a1: number; fill: string }[] = [];
  const progress = styleProgress(st, t, 0.05, 0.88);
  const ring = () => {
  m.rows.forEach((a, i) => {
    const sweep = (a.pct / 100) * Math.PI * 2;
    const gap = Math.min(0.012, sweep / 4);
    const mid = a0 + sweep / 2;
    if (st.grid && st.cells) cellSegs.push({ a0, a1: a0 + sweep, fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) });
    else if (sweep > 0) {
      const draw = () => s.add(path(arc(cx, cy, R1, R0, a0 + gap, a0 + sweep - gap), { fill: a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a) }));
      // 浮世绘按真实归属套完整色版，任何一版的角度都已经是最终份额。
      const at = i / Math.max(1, m.rows.length - 1);
      if (st.id === 'ukiyoe') materialEnter(s, st, styleProgress(st, t, at * .55, .35 + at * .55), { x: cx - R1, y: cy - R1, w: R1 * 2, h: R1 * 2 }, draw, 'data');
      else if (['pastel', 'cutout'].includes(st.id)) ringReveal(s, st, progress, cx, cy, R0, R1, draw, mid);
      else draw();
    }
    const shift = illustratedArcOffset(st, progress, mid);
    if (progress > 0 || !['pastel', 'cutout'].includes(st.id)) s.hit(cx + Math.cos(mid) * (R0 + R1) / 2 - 30 + shift.x, cy + Math.sin(mid) * (R0 + R1) / 2 - 30 + shift.y, 60, 60, tip(m, a));
    a0 += sweep;
  });
  if (cellSegs.length) gridRing(s, st, cx, cy, R0, R1, cellSegs, progress);
  };
  if (st.grid && st.cells || ['ukiyoe', 'pastel', 'cutout'].includes(st.id)) ring(); else ringReveal(s, st, progress, cx, cy, R0, R1, ring);
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
function gridRing(s: Scene, st: Style, cx: number, cy: number, R0: number, R1: number, segs: { a0: number; a1: number; fill: string }[], progress = 1) {
  const G = st.grid!;
  for (let ky = Math.floor((cy - R1) / G); ky * G <= cy + R1; ky++) {
    let x0 = 0, n = 0, cur = '';
    const flush = () => { if (n) st.cells!(s, x0, ky * G, n * G, G, cur); n = 0; };
    for (let kx = Math.floor((cx - R1) / G); kx * G <= cx + R1; kx++) {
      const dx = (kx + 0.5) * G - cx, dy = (ky + 0.5) * G - cy, d = Math.hypot(dx, dy);
      let a = Math.atan2(dy, dx);
      if (a < -Math.PI / 2) a += Math.PI * 2;
      const index = segs.findIndex(q => a >= q.a0 && a < q.a1);
      // 一份归属是一组完整字格弧，不能先印出半份再把它冒充完整份额。
      const appeared = progress >= 1 || index >= 0 && progress > materialOrder(st, index, segs.length, segs.length) * .8;
      const f = appeared && d >= R0 && d <= R1 && index >= 0 ? segs[index].fill : '';
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
    const at = i / Math.max(1, m.tools.length - 1);
    const sweep = (a.pct / 100) * Math.PI * 2;
    const gap = Math.min(0.012, sweep / 4);
    const unk = a.key === 'unattributed';
    const mid = a0 + sweep / 2;
    const innerK = styleProgress(st, t, st.id === 'ukiyoe' ? at * .28 : 0, st.id === 'ukiyoe' ? .35 + at * .28 : .55);
    if (grid) inner.push({ a0, a1: a0 + sweep, fill: unk ? st.hatch(s) : colorOf(st, a) });
    else if (sweep > 0) ringReveal(s, st, innerK, cx, cy, Ri0, Ri1, () => s.add(path(arc(cx, cy, Ri1, Ri0, a0 + gap, a0 + sweep - gap), { fill: unk ? st.hatch(s) : colorOf(st, a) })), mid);
    const innerShift = illustratedArcOffset(st, innerK, mid);
    if (innerK > 0 || !['pastel', 'cutout'].includes(st.id)) s.hit(cx + Math.cos(mid) * (Ri0 + Ri1) / 2 - 26 + innerShift.x, cy + Math.sin(mid) * (Ri0 + Ri1) / 2 - 26 + innerShift.y, 52, 52, tip(m, a));
    // 外环：这一段按模型再切
    let b0 = a0;
    for (const q of segsOf(m, st, a)) {
      const sw = sweep * (q.lines / Math.max(1, a.lines));
      const gg = Math.min(0.01, sw / 4);
      const qm = b0 + sw / 2;
      const outerK = styleProgress(st, t, st.id === 'ukiyoe' ? .3 + at * .24 : .3, st.id === 'ukiyoe' ? .7 + at * .24 : .95);
      if (grid) outer.push({ a0: b0, a1: b0 + sw, fill: unk ? st.hatch(s) : q.color });
      else if (sw > 0) ringReveal(s, st, outerK, cx, cy, Ro0, Ro1, () => s.add(path(arc(cx, cy, Ro1, Ro0, b0 + gg, b0 + sw - gg), { fill: unk ? st.hatch(s) : q.color })), qm);
      const outerShift = illustratedArcOffset(st, outerK, qm);
      if (sw > 0.04 && (outerK > 0 || !['pastel', 'cutout'].includes(st.id))) s.hit(cx + Math.cos(qm) * (Ro0 + Ro1) / 2 - 22 + outerShift.x, cy + Math.sin(qm) * (Ro0 + Ro1) / 2 - 22 + outerShift.y, 44, 44, segTip(m, a, q));
      b0 += sw;
    }
    a0 += sweep;
  });
  if (grid) {
    gridRing(s, st, cx, cy, Ri0, Ri1, inner, styleProgress(st, t, 0, 0.55));
    gridRing(s, st, cx, cy, Ro0, Ro1, outer, styleProgress(st, t, 0.3, 0.95));
  }
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

// 百分比格是可计数的完整单位。纸片、排字和像素可以刚性入位，绝不缩小成不足 1% 的格子。
const placedCells = new Set(['minimal', 'minimal-light', 'cutout', 'arcade', 'pastel', 'playbill', 'manga', 'editorial', 'baroque', 'meadow']);

function waffleCell(s: Scene, st: Style, k: number, t: number, b: Box, draw: () => void) {
  if (st.id === 'minimal-light' || st.id === 'minimal-dark') {
    if (t >= 1) { draw(); return; }
    // 100 个真单位共同对齐网格；没有逐格淡色、缩小或相互覆盖。
    const shift = 18 * (1 - Math.max(0, Math.min(1, t)));
    const dx = st.id === 'minimal-dark' ? -shift : 0, dy = st.id === 'minimal-light' ? shift : 0;
    const filter = st.id === 'minimal-dark' ? s.url('minimal-grid-edge', id => el('filter', { id },
      el('feMorphology', { in: 'SourceAlpha', operator: 'erode', radius: r(2.4 * (1 - t)), result: 'core' }),
      el('feComposite', { in: 'SourceAlpha', in2: 'core', operator: 'out', result: 'edge' }),
      el('feFlood', { floodColor: st.c.ink, floodOpacity: .53 * (1 - t), result: 'color' }),
      el('feComposite', { in: 'color', in2: 'edge', operator: 'in', result: 'rim' }),
      el('feMerge', {}, el('feMergeNode', { in: 'SourceGraphic' }), el('feMergeNode', { in: 'rim' })))) : undefined;
    const first = s.hits.length;
    drawGroup(s, { transform: `translate(${r(dx)} ${r(dy)})`, filter }, draw);
    for (let i = first; i < s.hits.length; i++) { s.hits[i].x = r(s.hits[i].x + dx); s.hits[i].y = r(s.hits[i].y + dy); }
    return;
  }
  materialEnter(s, st, k, b, draw, ['pastel', 'meadow', 'manga'].includes(st.id) ? 'mark' : st.id === 'baroque' ? 'data' : placedCells.has(st.id) ? 'paper' : 'data');
}

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
    // 一格始终等于 1%；材质决定铺版顺序，不把格子缩成另一种面积编码。
    const at = st.id === 'ukiyoe' ? m.rows.indexOf(a) / Math.max(1, m.rows.length - 1) : materialOrder(st, row * 10 + col, 100, 10);
    const k = st.id === 'ukiyoe' ? phase(styleProgress(st, t), at * .55, at * .55 + .45)
      : phase(styleProgress(st, t), at * 0.84, at * 0.84 + 0.16);
    if (k <= 0) return;
    const ss = size;
    waffleCell(s, st, k, t, st.id === 'ukiyoe' ? p : { x, y, w: size, h: size }, () => {
      if (a.key === 'unattributed') s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill: st.hatch(s), rx: Math.min(st.radius, ss / 4) }));
      else s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill: colorOf(st, a), rx: Math.min(st.radius, ss / 4) }));
      s.hit(x, y, size, size, tip(m, a));
    });
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
  const toolCells = m.tools.map((a) => cells.filter((c) => c?.a === a).length);
  const placed = m.tools.map(() => 0);
  // 偶数个工具/格子的对称次序也从最靠近中轴的单元开始，不留一段空白等待。
  const normalizeOrder = (orders: number[]) => { const lo = Math.min(...orders), span = Math.max(...orders) - lo; return orders.map((n) => span > 0 ? (n - lo) / span : 0); };
  const toolOrders = normalizeOrder(m.tools.map((_, i) => materialOrder(st, i, m.tools.length)));
  const cellOrders = toolCells.map((count) => normalizeOrder(Array.from({ length: count }, (_, i) => materialOrder(st, i, count))));
  cells.forEach((c, i) => {
    if (!c) return;
    const col = Math.floor(i / rows), row = i % rows;
    const x = colX(col), y = gy + row * (size + gap);
    // 分块版先认出工具，再铺开其内部模型，节奏按工具块走。
    const toolAt = m.tools.indexOf(c.a);
    const toolOrder = toolOrders[toolAt];
    const k = phase(styleProgress(st, t), toolOrder * 0.65, toolOrder * 0.65 + 0.35);
    const cellAt = placed[toolAt]++;
    const cellOrder = cellOrders[toolAt][cellAt];
    const plate = segs.indexOf(c) / Math.max(1, segs.length - 1);
    const cellProgress = st.id === 'ukiyoe' ? phase(styleProgress(st, t), plate * .6, plate * .6 + .4)
      : phase(k, cellOrder * 0.8, cellOrder * 0.8 + 0.2);
    if (cellProgress <= 0) return;
    const ss = size;
    const fill = c.a.key === 'unattributed' ? st.hatch(s) : c.q.color;
    waffleCell(s, st, cellProgress, t, st.id === 'ukiyoe' ? p : { x, y, w: size, h: size }, () => {
      s.add(rect(x + (size - ss) / 2, y + (size - ss) / 2, ss, ss, { fill, rx: Math.min(st.radius, ss / 4) }));
      s.hit(x, y, size, size, segsOf(m, st, c.a).length > 1 ? segTip(m, c.a, c.q) : tip(m, c.a));
    });
  });
  toolLegend(s, m, st, p.x + p.w * 0.68, p.x + p.w, p.y + 40, p.y + p.h);
  return s;
}

// ───────────────────────── 周河流图 ─────────────────────────

export function stream(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.weeks.length < 3) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1400, 520, T(m, '每周新增', 'PER WEEK'), T(m, '每周新增', 'Lines added per week'));
  const keys = orderKeys(m, m.weeks.flatMap((w) => Object.keys(w.by)));
  const tot = m.weeks.map((w) => keys.reduce((s2, k) => s2 + (w.by[k] ?? 0), 0));
  const max = Math.max(...tot, 1);
  const n = m.weeks.length;
  const X = (i: number) => p.x + (i / (n - 1)) * p.w;
  const mid = p.y + p.h / 2;
  // 水墨和草木的流线从中线生长；印版、屏幕与投影保留完整曲线，按各自的载体成形。
  // 不再让所有材质都使用同一套“河面变高”，仅调换各周的进度。
  const organic = ['ink', 'meadow'].includes(st.id);
  const flowAt = (i: number) => st.id === 'ink' ? ease(t, i, n, 0.5)
    : st.id === 'meadow' ? ease(styleProgress(st, t), materialOrder(st, i, n, n) * (n - 1), n, 0.5) : 1;
  const flow = m.weeks.map((_, i) => flowAt(i));
  const base = m.weeks.map((_, i) => mid - (tot[i] / max) * (p.h / 2) * flow[i]);
  const acc = [...base];
  const smooth = (pts: [number, number][]) => pts.map(([x, y], i) => {
    if (i === 0) return `M${r(x)} ${r(y)}`;
    const [px, py] = pts[i - 1];
    const cxm = (px + x) / 2;
    return `C${r(cxm)} ${r(py)} ${r(cxm)} ${r(y)} ${r(x)} ${r(y)}`;
  }).join('');
  // 字符格风格（原版终端）：每一列按两周之间插值的数堆起来，落到格子上，一格一个方块字；算法和平滑的河流一样，只是笔触不同
  const G = st.grid;
  const drawRiver = () => {
  if (G && st.cells) {
    const r0 = Math.ceil(p.y / G), nr = Math.floor((p.y + p.h) / G) - r0;
    for (let c = Math.ceil((p.x - G / 2) / G); (c + 0.5) * G <= p.x + p.w; c++) {
      // 终端按完整字符列提交；上下所有真实归属同时出现，不把一列的末几层裁掉。
      if (st.id === 'terminal' && t < 1 && (c + 0.5) * G >= p.x + p.w * styleProgress(st, t)) continue;
      const u = Math.min(n - 1, Math.max(0, (((c + 0.5) * G - p.x) / p.w) * (n - 1))), i = Math.min(n - 2, Math.floor(u)), f = u - i;
      const e = flowAt(u);
      let cum = 0;
      const b = keys.map((k) => (cum += (m.weeks[i].by[k] ?? 0) * (1 - f) + (m.weeks[i + 1].by[k] ?? 0) * f, Math.round((cum / max) * nr * e)));
      const top = r0 + Math.floor((nr - b[b.length - 1]) / 2);
      keys.forEach((k, j) => {
        const a = j ? b[j - 1] : 0;
        if (b[j] > a) st.cells!(s, c * G, (top + a) * G, G, (b[j] - a) * G, fillKey(s, st, m, k));
      });
    }
  }
  const colorPlates = ['ukiyoe', 'woodblock', 'poster', 'broadsheet', 'manuscript', 'grimoire'].includes(st.id);
  const opticalLayers = ['glass', 'neon', 'mono', 'pastel'].includes(st.id);
  for (const [index, k] of (G && st.cells ? [] : keys).entries()) {
    const top = acc.map((y, i) => [X(i), y] as [number, number]);
    const bot = acc.map((y, i) => [X(i), y + (((m.weeks[i].by[k] ?? 0) / max) * p.h) * flow[i]] as [number, number]);
    bot.forEach(([, y], i) => (acc[i] = y));
    const d = smooth(top) + smooth([...bot].reverse()).replace(/^M/, 'L') + 'Z';
    const draw = () => s.add(path(d, { fill: fillKey(s, st, m, k), opacity: 0.92, stroke: st.c.bg, strokeWidth: 0.6 }));
    // 色版的单元是贡献方的整条河带；玻璃/灯管的单元也是曲线自身，而非整张图淡入。
    // 柔粉色让相邻的真实色带成对显现；不以两个抽象圆泡割裂同一条河带。
    const order = st.id === 'pastel' ? Math.floor(index / 2) / Math.max(1, Math.ceil(keys.length / 2) - 1)
      : index / Math.max(1, keys.length - 1);
    const layer = styleProgress(st, t, order * 0.3, 0.7 + order * 0.3);
    if (t < 1 && st.id === 'woodblock') {
      // 整块底墨先接触真实河带，细刀纹只改变印面密度；不把河面切成粗竖条。
      const pitch = 5;
      const marks = path(`M0 0H${r(pitch * layer)}L${r(pitch * layer * .8)} 9L${r(pitch * layer)} 17V24H0Z`, { fill: '#fff' });
      const print = s.url(`stream-print-${index}`, id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width: pitch, height: 24 }, marks));
      const mask = s.url(`stream-ink-${index}`, id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: p.x, y: p.y, width: p.w, height: p.h },
        rect(p.x, p.y, p.w, p.h, { fill: '#fff', opacity: .6 * phase(layer, 0, .18) + .4 * phase(layer, .55, 1) })
        + rect(p.x, p.y, p.w, p.h, { fill: print, opacity: .45 * phase(layer, 0, .2) })));
      drawGroup(s, { mask }, draw);
    }
    else if (t < 1 && st.id === 'ukiyoe') materialEnter(s, st, layer, p, draw, 'data');
    else if (t < 1 && st.id === 'cutout') {
      // 河带本身是已切好的完整纸片；各片刚性落定，不用一张整图只改变纸影。
      const ys = [...top, ...bot].map(([, y]) => y), y = Math.min(...ys);
      materialEnter(s, st, layer, { x: p.x, y, w: p.w, h: Math.max(...ys) - y }, draw, 'paper');
    }
    else if (t < 1 && colorPlates) materialReveal(s, st, layer, p, 'right', draw);
    else if (t < 1 && opticalLayers) materialEnter(s, st, layer, p, draw, 'data');
    else draw();
  }
  };
  if (['cyber', 'manga'].includes(st.id) && t < 1) {
    // 电路格/网点在完整的周截面上成像，不拆成隔周黑栅栏或缺少下层的河带。
    materialEnter(s, st, styleProgress(st, t), p, drawRiver, 'data');
  }
  else if (st.id === 'editorial' && t < 1) {
    // 排版逐栏落下真实周列的完整截面，不用斜切边割掉同一周的某个贡献方。
    const at = phase(t, 0, 0.92), half = p.w / (n - 1) / 2;
    const windows: [number, number][] = [];
    m.weeks.forEach((_, i) => {
      if (at <= i / n) return;
      const left = Math.max(p.x, X(i) - half), right = Math.min(p.x + p.w, X(i) + half);
      const last = windows[windows.length - 1];
      // 邻列使用一个连续裁窗，避免每个矩形抗锯齿在完整河面上留下细竖缝。
      if (last && left <= last[1] + 0.02) last[1] = right;
      else windows.push([left, right]);
    });
    const columns = windows.map(([left, right]) => rect(left, p.y, right - left, p.h));
    const clipPath = s.url('stream-columns', (id) => el('clipPath', { id }, columns.join('')));
    drawGroup(s, { clipPath }, drawRiver);
  }
  else if (st.id === 'terminal') {
    if (G && st.cells) drawRiver();
    else reveal(s, styleProgress(st, t), p, 'right', drawRiver);
  }
  else if (st.id === 'crt') {
    // 时间轴是横向的；扫描前缘必须带着该时刻的全部上下层一起走。
    const k = styleProgress(st, t);
    reveal(s, k, p, 'right', () => materialEnter(s, st, k, p, drawRiver, 'data'));
  }
  else if (st.id === 'gala' && t < 1) {
    // 舞台柔光沿时间轴由中间向两侧展开；同一时刻上下所有归属接受相同曝光。
    // 椭圆遮罩会把上/下层吃掉，不能用于堆叠河流的数值截面。
    const k = styleProgress(st, t);
    if (k <= 0) drawGroup(s, { visibility: 'hidden' }, drawRiver);
    else {
      const core = Math.min(.5, Math.max(0, k * .8 - .12)), edge = Math.min(.5, k * .8 + .04);
      const light = s.url('stream-time-light', id => el('linearGradient', { id, x1: 0, y1: 0, x2: 1, y2: 0 },
        ...[[0, 0], [.5 - edge, 0], [.5 - core, 1], [.5 + core, 1], [.5 + edge, 0], [1, 0]].map(([offset, opacity]) => el('stop', { offset, stopColor: '#fff', stopOpacity: opacity }))));
      const mask = s.url('stream-time-mask', id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: p.x, y: p.y, width: p.w, height: p.h }, rect(p.x, p.y, p.w, p.h, { fill: light })));
      drawGroup(s, { mask }, drawRiver);
    }
  }
  else if (organic || ['ukiyoe', 'woodblock', 'poster', 'broadsheet', 'manuscript', 'glass', 'neon', 'mono', 'grimoire', 'pastel', 'cutout', 'manga'].includes(st.id)) drawRiver();
  else materialReveal(s, st, styleProgress(st, t), p, 'right', drawRiver);
  // 每周的悬停区
  m.weeks.forEach((w, i) => {
    const lines = keys.filter((k) => w.by[k]).sort((a, b) => w.by[b] - w.by[a]).slice(0, 4).map((k) => `${agentOf(m, k)?.name ?? (k === 'human' ? nameOf('people', m.lang) : k)} ${fmt(w.by[k])}`);
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
  const { s, p } = frame(m, st, 1400, keys.length * laneH, T(m, '会话', 'SESSIONS'), m.group === 'model' ? T(m, '各模型会话时间线', 'Session timeline by model') : T(m, '各工具会话时间线', 'Session timeline by tool'), false);
  const t0 = Math.min(...ss.map((x) => x.start)), t1 = Math.max(...ss.map((x) => x.end));
  const X = (v: number) => p.x + 170 + ((v - t0) / Math.max(1, t1 - t0)) * (p.w - 170);
  keys.forEach((k, li) => {
    const y = p.y + li * laneH;
    const a = m.agents.find((x) => x.key === k);
    const mine = ss.filter((x) => x.key === k);
    const illustrated = ['pastel', 'cutout', 'meadow', 'manga'].includes(st.id) && t < 1;
    const connected = st.id === 'cyber' && t < 1;
    const progress = styleProgress(st, t, .02, .9);
    const drawLane = () => {
      st.mark(s, k, p.x + 16, y + laneH / 2, 24);
      s.text(a?.name ?? k, p.x + 38, y + laneH / 2 + 6, { font: st.f.body, size: 16, fill: st.c.ink, maxW: 125 });
      st.track(s, p.x + 170, y + laneH / 2 - 1, p.w - 170, 2);
      mine.forEach((x) => {
        const x0 = X(x.start), w = Math.max(3, X(x.end) - x0);
        const col = a ? colorOf(st, m.rows.find((q) => q.key === k) ?? a) : st.c.soft;
        const draw = () => s.add(rect(x0, y + 16, w, laneH - 32, { fill: x.review ? 'none' : col, stroke: x.review ? col : 'none', strokeWidth: 1.4, rx: Math.min(st.radius, 4), opacity: 0.9 }));
        const at = (x.start - t0) / Math.max(1, t1 - t0);
        // 插画版把真实泳道作为整张排版单元；数字版沿现有时间轴寻址后接通完整区间。
        if (illustrated) draw();
        else if (connected) { if (progress > 0 && progress >= at) draw(); }
        else {
          const start = at * .65;
          materialEnter(s, st, styleProgress(st, t, start, start + .35), { x: x0, y: y + 16, w, h: laneH - 32 }, draw, 'data');
        }
      });
      if (connected && progress > 0 && progress < 1) {
        // 读头只点亮原本的泳道线，位置对应上方真实开始时刻，不另加连接线或假区间。
        const left = p.x + 170, width = p.w - 170, head = left + progress * width;
        s.add(rect(Math.max(left, head - 12), y + laneH / 2 - 1, Math.min(12, head - left), 2, { fill: st.c.accent }));
      }
    };
    if (illustrated) {
      const order = st.id === 'pastel' ? Math.floor(li / 2) / Math.max(1, Math.ceil(keys.length / 2) - 1)
        : li / Math.max(1, keys.length - 1);
      const formed = styleProgress(st, t, order * .52, order * .52 + .48), rest = 1 - formed;
      let dy = 0, angle = 0;
      if (st.id === 'pastel') dy = (li % 2 ? 1 : -1) * 12 * rest;
      if (st.id === 'cutout') { dy = -6 * rest; angle = -1 * rest; }
      if (st.id === 'meadow') { dy = 16 * rest * rest; angle = -1.1 * rest; }
      if (st.id === 'manga') dy = formed < .35 ? -14 : formed < .7 ? -4 : 0;
      drawGroup(s, { transform: `translate(0 ${r(dy)})${angle ? ` rotate(${r(angle)} ${r(p.x)} ${r(y + laneH / 2)})` : ''}` },
        () => materialEnter(s, st, formed, { x: p.x, y: y + 12, w: p.w, h: laneH - 24 }, drawLane, 'data'));
    } else drawLane();
    s.hit(p.x, y, p.w, laneH, a ? tip(m, a) : { t: k });
  });
  const d = (v: number) => new Date(v * 1000).toISOString().slice(0, 10);
  s.text(d(t0), p.x + 170, p.y + p.h + 28, { font: st.f.label, size: 13, fill: st.c.faint });
  s.text(d(t1), p.x + p.w, p.y + p.h + 28, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end' });
  return s;
}

// ───────────────────────── 作息打卡 ─────────────────────────

/** 屏幕标记先寻址/扫描/接通轮廓；完整真值随后落定，不缩放或稀释活动量。
 * 纸面小热力标记保留整枚落定：额外定位角、压印边会变成无意义的装饰。 */
function activityCue(s: Scene, st: Style, k: number, b: Box, round: boolean) {
  if (k <= 0 || k >= 1 || !['terminal', 'crt', 'neon'].includes(st.id)) return;
  const x = b.x - 1.5, y = b.y - 1.5, w = b.w + 3, h = b.h + 3;
  const cx = x + w / 2, a = Math.sin(Math.PI * k) * 0.42;
  let d = '', dash: string | undefined;
  if (st.id === 'terminal') d = `M${r(x)} ${r(y + h)}h${r(Math.min(w, 5))}`;
  if (st.id === 'crt') d = `M${r(x)} ${r(y + h * k)}h${r(w)}`;
  if (st.id === 'neon') {
    d = round ? `M${r(cx)} ${r(y)}A${r(w / 2)} ${r(h / 2)} 0 1 1 ${r(cx - 0.001)} ${r(y)}Z`
      : `M${r(x)} ${r(y)}h${r(w)}v${r(h)}h${r(-w)}Z`;
    const length = round ? Math.PI * w : 2 * (w + h);
    dash = `${r(length * k)} ${r(length)}`;
  }
  s.add(path(d, { fill: 'none', stroke: st.c.accent, strokeWidth: 0.8, strokeOpacity: a, strokeDasharray: dash }));
}
function activityPhase(st: Style, t: number, at: number): number {
  const progress = styleProgress(st, t);
  return phase(progress, at * 0.82, at * 0.82 + 0.18);
}

export function clock(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (m.clock.length !== 7) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1400, 420, T(m, '星期 × 小时', 'DAY × HOUR'), T(m, '活跃时段', 'Active hours'), false, styleProgress(st, t, 0, .44));
  const max = Math.max(...m.clock.flat(), 1);
  const days = T(m, '一二三四五六日', 'MTWTFSS');
  const cw = (p.w - 60) / 24, ch = p.h / 7;
  const grouped = t < 1 && ['pastel', 'cutout', 'meadow', 'manga', 'cyber', 'ukiyoe'].includes(st.id);
  const bands = new Map<number, { b: Box; at: number; side: -1 | 1; draw: (() => void)[] }>();
  const bandFor = (d: number, hr: number) => {
    const half = Math.floor(hr / 12), panel = Math.floor(hr / 6);
    const key = st.id === 'pastel' ? d * 2 + half : st.id === 'manga' ? panel : d;
    if (!bands.has(key)) {
      const b = st.id === 'pastel' ? { x: p.x + 60 + half * 12 * cw, y: p.y + d * ch, w: 12 * cw, h: ch }
        : st.id === 'manga' ? { x: p.x + 60 + panel * 6 * cw, y: p.y, w: 6 * cw, h: p.h }
        : { x: p.x + 60, y: p.y + d * ch, w: p.w - 60, h: ch };
      bands.set(key, { b, at: st.id === 'manga' ? (3 - panel) / 3 : d / 6,
        side: st.id === 'pastel' ? half ? 1 : -1 : d % 2 ? 1 : -1, draw: [] });
    }
    return bands.get(key)!;
  };
  m.clock.forEach((row, d) => {
    s.text(m.lang === 'zh' ? `周${days[d]}` : days[d], p.x, p.y + d * ch + ch / 2 + 6, { font: st.f.label, size: 14, fill: st.c.soft });
    row.forEach((v, hr) => {
      const cx = p.x + 60 + hr * cw + cw / 2, cy = p.y + d * ch + ch / 2;
      const rr = Math.sqrt(v / max) * Math.min(cw, ch) * 0.46;
      // 屏幕材质先准备外缘，其余整枚落定；大小和浓度不参与插值。
      const at = materialOrder(st, d * 24 + hr, 7 * 24, 24);
      const k = activityPhase(st, t, at), radius = Math.max(1.5, rr);
      if (grouped) {
        // 半日成对、完整星期行或六小时分镜一起归位；空网点也跟随它所属的时间组。
        bandFor(d, hr).draw.push(() => {
          s.add(v ? circle(cx, cy, radius, { fill: st.c.accent, opacity: 0.25 + 0.75 * (v / max) })
            : circle(cx, cy, 1.2, { fill: st.c.faint, opacity: 0.5 }));
          if (v) s.hit(cx - cw / 2, cy - ch / 2, cw, ch, { t: `${m.lang === 'zh' ? '周' + days[d] : days[d]} ${hr}:00`, d: [T(m, `${v} 次提交`, `${v} commits`)] });
        });
        return;
      }
      if (v) activityCue(s, st, k, { x: cx - radius, y: cy - radius, w: radius * 2, h: radius * 2 }, true);
      if (v && !['terminal', 'crt', 'neon'].includes(st.id)) activityEnter(s, st, k, { x: cx - radius, y: cy - radius, w: radius * 2, h: radius * 2 },
        () => s.add(circle(cx, cy, radius, { fill: st.c.accent, opacity: 0.25 + 0.75 * (v / max) })), true);
      else if (v && (t >= 1 || k >= 0.4)) s.add(circle(cx, cy, radius, { fill: st.c.accent, opacity: 0.25 + 0.75 * (v / max) }));
      else s.add(circle(cx, cy, 1.2, { fill: st.c.faint, opacity: 0.5 }));
      if (v) s.hit(cx - cw / 2, cy - ch / 2, cw, ch, { t: `${m.lang === 'zh' ? '周' + days[d] : days[d]} ${hr}:00`, d: [T(m, `${v} 次提交`, `${v} commits`)] });
    });
  });
  for (const band of bands.values()) {
    const k = phase(styleProgress(st, t), band.at * .62, band.at * .62 + .36);
    if (st.id === 'ukiyoe') {
      // 每个真实星期是一张完整的时段墨版；原点径、强度与点间距不参与压印插值。
      // 整行轻套位，避免退回单点的斜向点亮顺序，也不跨到相邻日期行。
      if (k <= 0) continue;
      const dx = r(-14 * (1 - k) ** 2), dy = r(-2 * (1 - k) ** 2), firstHit = s.hits.length;
      drawGroup(s, { transform: `translate(${dx} ${dy})` }, () => band.draw.forEach(draw => draw()));
      for (const hit of s.hits.slice(firstHit)) { hit.x += dx; hit.y += dy; }
    } else activityEnter(s, st, k, band.b, () => band.draw.forEach(draw => draw()), true, band.side);
  }
  for (const hr of [0, 6, 12, 18, 23]) s.text(`${hr}:00`, p.x + 60 + hr * cw + cw / 2, p.y + p.h + 24, { font: st.f.label, size: 12, fill: st.c.faint, anchor: 'middle' });
  return s;
}

// ───────────────────────── 版本 ─────────────────────────

export function versions(m: Model, st: Style, o: PieceOpts): Scene | null {
  const { buckets, basis } = m.history;
  const labels = historyLabels(basis, m.lang);
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, Math.max(1200, buckets.length * 170 + 168), 520, labels.kicker, labels.caption, buckets.length > 0);
  if (!buckets.length) {
    s.text(historyEmpty(m.history, m.lang), p.x + p.w / 2, p.y + p.h / 2, { font: st.f.body, size: 22, fill: st.c.soft, anchor: 'middle', maxW: p.w });
    return s;
  }
  const keys = orderKeys(m, buckets.flatMap((v) => Object.keys(v.by)));
  const max = Math.max(...buckets.map((v) => v.total), 1);
  const n = buckets.length, bw = Math.min(120, (p.w / n) * 0.55);
  buckets.forEach((v, i) => {
    const x = p.x + (i + 0.5) * (p.w / n) - bw / 2;
    let y = p.y + p.h;
    const unit = st.id === 'pastel' ? Math.floor(i / 2) : i, units = st.id === 'pastel' ? Math.ceil(n / 2) : n;
    const k = styleProgress(st, t, unit / (units + 1), (unit + 2) / (units + 1));
    // 一列的上下边界和内部份额都是真值；显现只沿厚度，不能从统一绘图区底部截出等高的临时柱。
    const columnH = keys.reduce((sum, key) => sum + (v.by[key] ?? 0), 0) / max * p.h;
    valueBar(s, st, k, { x, y: p.y + p.h - columnH, w: bw, h: columnH }, 'y', () => {
    for (const key of keys) {
      const h = ((v.by[key] ?? 0) / max) * p.h;
      if (h <= 0) continue;
      y -= h;
      s.add(rect(x, y, bw, Math.max(0, h - 1.5), { fill: fillKey(s, st, m, key) }));
    }
    });
    const label = v.tag === 'unreleased' ? T(m, '未发布', 'Unreleased') : v.tag;
    s.text(label, x + bw / 2, p.y + p.h + 28, { font: st.f.label, size: 14, fill: st.c.soft, anchor: 'middle', maxW: p.w / n - 16 });
    if (k >= 1) st.num(s, compact(v.total), x + bw / 2, y - 12, 16, 'middle', { fill: st.c.soft });
    s.hit(x - 10, p.y, bw + 20, p.h + 40, { t: label, d: [v.date, T(m, `${fmt(v.total)} 行`, `${fmt(v.total)} lines`), ...keys.filter((q) => v.by[q]).slice(0, 4).map((q) => `${agentOf(m, q)?.name ?? q} ${Math.round((v.by[q] / v.total) * 100)}%`)].filter(Boolean) });
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
  const { s, p } = frame(m, st, 1200, list.length * rowH, T(m, '留存', 'RETENTION'), T(m, '新增与留存对比', 'Retention comparison'), false);
  const max = Math.max(...list.map((a) => a.lines));
  list.forEach((a, i) => {
    const y = p.y + i * rowH;
    st.mark(s, a.key, p.x + 14, y + 22, 24);
    s.text(a.name, p.x + 36, y + 30, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 170 });
    const x0 = p.x + 230, w = p.w - 330;
    const wa = (a.lines / max) * w, ws = (a.surviving / max) * w;
    // 字符格风格：写过、已经不在的那一截是一排点（原版的 ···）
    valueBar(s, st, styleProgress(st, t, 0, 0.45), { x: x0, y: y + 8, w: wa + 2, h: 30 }, 'x', () => {
    if (st.cells) st.track(s, x0, y + 10, wa, 26);
    else s.add(rect(x0, y + 10, wa, 26, { fill: colorOf(st, a), opacity: 0.25, rx: Math.min(st.radius, 6) }));
    });
    // 先说明写入总量，再显现真实留存量；条长始终由数据决定，不插入虚假的留存比例。
    const retained = styleProgress(st, t, 0.45, 0.95);
    valueBar(s, st, retained, { x: x0, y: y + 8, w: ws + 2, h: 30 }, 'x', () => st.bar(s, x0, y + 10, Math.max(2, ws), 26, colorOf(st, a), i));
    if (retained >= 1) st.num(s, `${Math.round(a.survival * 100)}%`, p.x + p.w, y + 32, 22, 'end');
    s.hit(p.x, y, p.w, rowH - 6, { ...tip(m, a), d: [T(m, `新增 ${fmt(a.lines)} 行，留存 ${fmt(a.surviving)} 行`, `${fmt(a.lines)} added, ${fmt(a.surviving)} retained`), ...tip(m, a).d.slice(1)] });
  });
  return s;
}

// ───────────────────────── 文件马赛克（矩形树图） ─────────────────────────

export function mosaic(m: Model, st: Style, o: PieceOpts): Scene | null {
  const files = m.files.filter((f) => f.lines > 0).sort((a, b) => b.lines - a.lines);
  if (files.length < 3) return null;
  const t = o.t ?? 1;
  const { s, p } = frame(m, st, 1200, 640, T(m, '文件', 'FILES'), T(m, '各文件代码归属', 'Code by file'));
  const total = files.reduce((x, f) => x + f.lines, 0);
  const rects = squarify(files.map((f) => f.lines / total * p.w * p.h), { ...p });
  rects.forEach((b, i) => {
    const f = files[i];
    // 文件从空间中的所属位置拼合，保留各文件面积；不是按排名同时闪入。
    const at = materialOrder(st, i, rects.length);
    const k = styleProgress(st, t, at * 0.55, at * 0.55 + 0.45);
    const keys = orderKeys(m, Object.keys(f.by));
    materialEnter(s, st, k, b, () => {
    // 每个文件内部按贡献方竖切
    let x = b.x;
    for (const key of keys) {
      const w = ((f.by[key] ?? 0) / f.lines) * b.w;
      if (w > 0) s.add(rect(x, b.y, w, b.h, { fill: fillKey(s, st, m, key), opacity: 0.95 }));
      x += w;
    }
    // 文件全画：很小的格子描边跟着变细，不然整格被边线盖住
    s.add(rect(b.x, b.y, b.w, b.h, { fill: 'none', stroke: st.c.bg, strokeWidth: Math.min(2, b.w / 4, b.h / 4) }));
    if (b.w > 120 && b.h > 34 && f.path) s.text(f.path.split('/').pop()!, b.x + 10, b.y + 22, { font: st.f.mono, size: 13, fill: st.on(fillKey(s, st, m, keys[0])), maxW: b.w - 20, overflow: 'clip' });
    s.hit(b.x, b.y, b.w, b.h, { t: f.path || T(m, '文件', 'file'), d: [T(m, `${fmt(f.lines)} 行`, `${fmt(f.lines)} lines`), ...keys.filter((q) => f.by[q]).slice(0, 4).map((q) => `${agentOf(m, q)?.name ?? (q === 'human' ? nameOf('people', m.lang) : q)} ${Math.round((f.by[q] / f.lines) * 100)}%`)] });
    }, ['neon', 'mono', 'grimoire'].includes(st.id) ? 'data' : 'content');
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
  const { s, p } = frame(m, st, Math.max(900, weeks * (cell + 4) + 230), 7 * (cell + 4), T(m, '日历', 'CALENDAR'), T(m, '活跃日历', 'Activity calendar'), true, styleProgress(st, t, 0, .44));
  const byDate = new Map(m.days.map((d) => [d.date, d.by]));
  const plateKeys = st.id === 'ukiyoe' ? [...new Set(m.days.map(d => Object.entries(d.by).sort((a, b) => b[1] - a[1])[0]?.[0]).filter(Boolean))] : [];
  const max = Math.max(...m.days.map((d) => Object.values(d.by).reduce((x, y) => x + y, 0)), 1);
  // 剪纸已有清楚的整枚纸格倾转；其余插画材质把真实整周作为可读的活动单位。
  const edition = ['editorial', 'mono', 'cinema', 'playbill', 'broadsheet', 'gala', 'crt', 'arcade'].includes(st.id);
  const weekday = ['editorial', 'crt'].includes(st.id);
  const grouped = t < 1 && (edition || ['pastel', 'meadow', 'manga', 'cyber'].includes(st.id));
  const weeksDrawn: (() => void)[][] = Array.from({ length: weekday ? 7 : weeks }, () => []);
  for (let w = 0; w < weeks; w++) for (let d = 0; d < 7; d++) {
    const day = new Date(start.getTime() + (w * 7 + d) * 864e5).toISOString().slice(0, 10);
    const by = byDate.get(day);
    const x = p.x + w * (cell + 4), y = p.y + d * (cell + 4);
    const tot = by ? Object.values(by).reduce((a, b) => a + b, 0) : 0;
    if (!tot) {
      const draw = () => s.add(rect(x, y, cell, cell, { fill: st.c.ink, opacity: 0.05, rx: Math.min(st.radius, 4) }));
      if (grouped) weeksDrawn[weekday ? d : w].push(draw); else draw();
      continue;
    }
    const top = Object.entries(by!).sort((a, b) => b[1] - a[1])[0][0];
    if (grouped) {
      weeksDrawn[weekday ? d : w].push(() => {
        s.add(rect(x, y, cell, cell, { fill: fillKey(s, st, m, top), opacity: 0.3 + 0.7 * Math.sqrt(tot / max), rx: Math.min(st.radius, 4) }));
        s.hit(x, y, cell, cell, { k: top, t: day, d: [T(m, `${fmt(tot)} 行`, `${fmt(tot)} lines`), ...Object.entries(by!).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([q, v]) => `${agentOf(m, q)?.name ?? (q === 'human' ? nameOf('people', m.lang) : q === 'unattributed' ? T(m, '未确认', 'Unattributed') : q)} ${fmt(v)}`)] });
      });
      continue;
    }
    // 屏幕日期先寻址/扫描/接通边线，其余整枚落定；格子始终承载真实活动量。
    const at = st.id === 'ukiyoe' ? plateKeys.indexOf(top) / Math.max(1, plateKeys.length - 1)
      : materialOrder(st, d * weeks + w, weeks * 7, weeks);
    const k = activityPhase(st, t, at);
    activityCue(s, st, k, { x, y, w: cell, h: cell }, false);
    if (!['terminal', 'crt', 'neon'].includes(st.id)) activityEnter(s, st, k, { x, y, w: cell, h: cell },
      () => s.add(rect(x, y, cell, cell, { fill: fillKey(s, st, m, top), opacity: 0.3 + 0.7 * Math.sqrt(tot / max), rx: Math.min(st.radius, 4) })), false);
    else if (t >= 1 || k >= 0.4) s.add(rect(x, y, cell, cell, { fill: fillKey(s, st, m, top), opacity: 0.3 + 0.7 * Math.sqrt(tot / max), rx: Math.min(st.radius, 4) }));
    s.hit(x, y, cell, cell, { k: top, t: day, d: [T(m, `${fmt(tot)} 行`, `${fmt(tot)} lines`), ...Object.entries(by!).sort((a, b) => b[1] - a[1]).slice(0, 3).map(([q, v]) => `${agentOf(m, q)?.name ?? (q === 'human' ? nameOf('people', m.lang) : q === 'unattributed' ? T(m, '未确认', 'Unattributed') : q)} ${fmt(v)}`)] });
  }
  if (grouped) weeksDrawn.forEach((draws, w) => {
    const at = weekday ? w / 6
      : st.id === 'cinema' ? Math.abs(w - (weeks - 1) / 2) / Math.max(1, (weeks - 1) / 2)
      : ['pastel', 'playbill', 'gala'].includes(st.id) ? Math.floor(w / 2) / Math.max(1, Math.ceil(weeks / 2) - 1)
      : st.id === 'manga' ? (weeks - w - 1) / Math.max(1, weeks - 1) : w / Math.max(1, weeks - 1);
    activityEnter(s, st, phase(styleProgress(st, t), at * .62, at * .62 + .36),
      weekday ? { x: p.x, y: p.y + w * (cell + 4), w: weeks * (cell + 4) - 4, h: cell }
        : { x: p.x + w * (cell + 4), y: p.y, w: cell, h: 7 * (cell + 4) - 4 },
      () => draws.forEach(draw => draw()), false, w % 2 ? 1 : -1);
  });
  return s;
}

void line; void el; void g; void mix;

// ───────────────────────── 出处 ─────────────────────────

/** 三种证据各用同一个颜色的三种浓度：逐行证据最实，推断最淡 */
const PROOF: { k: 'verified' | 'declared' | 'inferred'; o: number; zh: string; en: string }[] = [
  { k: 'verified', o: 1, zh: '日志证据', en: 'Line logs' },
  { k: 'declared', o: 0.55, zh: '明确声明', en: 'Declared' },
  { k: 'inferred', o: 0.25, zh: '推断', en: 'Inferred' },
];

export function evidence(m: Model, st: Style, o: PieceOpts): Scene | null {
  const list = m.rows.filter((a) => a.lines > 0 && (a.key === 'unattributed' || (a.proof && a.proof.verified + a.proof.declared + a.proof.inferred > 0)));
  if (!list.some((a) => a.key !== 'unattributed')) return null;
  const t = o.t ?? 1;
  const rowH = 74;
  const head = 150;
  const { s, p } = frame(m, st, 1200, head + list.length * rowH + 40, T(m, '出处', 'EVIDENCE'), T(m, '归因依据', 'Attribution evidence'), false);
  // 头条：全部 AI 行里有逐行证据的比例
  // 核验结论整枚落位，避免扫到一半时只留下半个数字。
  materialEnter(s, st, styleProgress(st, t, 0.72, 1), { x: p.x, y: p.y, w: 220, h: 78 }, () => st.num(s, `${Math.round(m.verifiedShare * 100)}%`, p.x, p.y + 64, 64, 'start'));
  s.text(T(m, '的代码行有完整日志记录', 'of lines have matching session logs'), p.x, p.y + 104, { font: st.f.body, size: 17, fill: st.c.soft, maxW: p.w });
  const max = Math.max(...list.map((a) => a.lines));
  const x0 = p.x + 230, w = p.w - 330;
  list.forEach((a, i) => {
    const y = p.y + head + i * rowH;
    st.mark(s, a.key, p.x + 14, y + 22, 24);
    s.text(a.name, p.x + 36, y + 30, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 170 });
    const full = (a.lines / max) * w;
    if (a.key === 'unattributed') {
      valueBar(s, st, styleProgress(st, t, 0.05, 0.4), { x: x0, y: y + 8, w: full + 2, h: 30 }, 'x', () => {
      if (st.cells) st.bar(s, x0, y + 10, Math.max(2, full), 26, st.hatch(s), i);
      else s.add(rect(x0, y + 10, Math.max(2, full), 26, { fill: st.hatch(s), rx: Math.min(st.radius, 6) }));
      });
      s.text(T(m, '无日志证据', 'no evidence'), p.x + p.w, y + 30, { font: st.f.body, size: 15, fill: st.c.faint, anchor: 'end' });
      s.hit(p.x, y, p.w, rowH - 6, tip(m, a));
      return;
    }
    const pf = a.proof!;
    const tot = pf.verified + pf.declared + pf.inferred || 1;
    if (t < .95 && ['minimal-light', 'minimal-dark', 'film', 'glass'].includes(st.id)) {
      // 各来源仍按真实段宽、固定位置核验；完整总条底面提供量程语境，避免早期短段悬空。
      s.add(rect(x0, y + 10, full, 26, { fill: st.c.ink, opacity: .09 * (1 - phase(t, .8, .95)), rx: Math.min(st.radius, 6) }));
    }
    let x = x0;
    PROOF.forEach((q, j) => {
      const ww = (pf[q.k] / tot) * full;
      if (ww <= 0) return;
      // 从较弱的依据到有日志的证据，最后才揭示总的核验比例。
      const start = j === 0 ? 0.45 : j === 1 ? 0.2 : 0;
      valueBar(s, st, styleProgress(st, t, start, start + 0.5), { x, y: y + 8, w: ww + 2, h: 30 }, 'x', () => {
      if (j === 0) st.bar(s, x, y + 10, Math.max(2, ww), 26, colorOf(st, a), i);
      // 字符格风格：浅的两层也是方块字，按浓淡调透明
      else if (st.cells) st.bar(s, x, y + 10, ww, 26, alpha(colorOf(st, a), q.o), i);
      else s.add(rect(x, y + 10, ww, 26, { fill: colorOf(st, a), opacity: q.o, rx: Math.min(st.radius, 6) }));
      });
      x += ww;
    });
    if (t >= 0.95) st.num(s, `${Math.round((pf.verified / tot) * 100)}%`, p.x + p.w, y + 32, 22, 'end');
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

// ───────────────────────── 板块 ─────────────────────────

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
  const { s, p } = frame(m, st, 1200, rows.length * rowH, T(m, '板块', 'AREAS'), T(m, '各模块代码归属', 'Code by module'));
  const x0 = p.x + 200, w = p.w - 200;
  const plateKeys = orderKeys(m, rows.flatMap(row => row.by.map(([key]) => key)));
  rows.forEach((r, i) => {
    const y = p.y + i * rowH;
    // 同一模块的各份额一起出现；柔粉色按成对模块排入，完整横条不被圆窗切成碎块。
    const unit = st.id === 'pastel' ? Math.floor(i / 2) : i, units = st.id === 'pastel' ? Math.ceil(rows.length / 2) : rows.length;
    const k = styleProgress(st, t, unit / (units + 1), (unit + 2) / (units + 1));
    s.text(r.name, p.x, y + 30, { font: st.f.body, size: 19, fill: st.c.ink, maxW: 180 });
    s.text(T(m, `${fmt(r.tot)} 行`, `${fmt(r.tot)} lines`), p.x, y + 54, { font: st.f.body, size: 14, fill: st.c.faint });
    let x = x0;
    for (const key of orderKeys(m, r.by.map(([key]) => key))) {
      const v = r.by.find(([kk]) => kk === key)![1];
      const ww = (v / r.tot) * w;
      if (ww < 0.5) continue;
      // 浮世绘按真实归属色版套印：同一贡献方跨模块同时整版接触纸面。
      // 每一份额从出现起就是完整宽度，不让通用扫描窗伪造不断增长的条。
      const plate = plateKeys.indexOf(key) / Math.max(1, plateKeys.length - 1);
      const progress = st.id === 'ukiyoe' ? styleProgress(st, t, plate * 0.6, 0.4 + plate * 0.6) : k;
      const draw = () => {
      if (st.block && (key !== 'unattributed' || st.cells)) st.block(s, x, y + 12, Math.max(1, ww - 2), 38, fillKey(s, st, m, key));
      else s.add(rect(x, y + 12, Math.max(1, ww - 2), 38, { fill: fillKey(s, st, m, key), rx: Math.min(st.radius, 6) }));
      const tk = tipKey(m, key);
      s.hit(x, y + 8, ww, 46, { ...tk, d: [T(m, `${r.name}里 ${fmt(v)} 行，占 ${Math.round((v / r.tot) * 100)}%`, `${fmt(v)} lines in ${r.name}, ${Math.round((v / r.tot) * 100)}%`), ...(tk.d ?? []).slice(1)] });
      };
      const rowBox = { x: x0, y: y + 8, w, h: 46 };
      if (st.id === 'ukiyoe') materialEnter(s, st, progress, { x, y: y + 8, w: ww, h: 46 }, draw, 'data');
      else if (st.id === 'playbill') materialEnter(s, st, progress, rowBox, draw, 'paper');
      // 份额编码在横轴：中轴幕口、聚光和像素窗不能暂时遮掉某一整份数据。
      else if (st.cells || ['terminal', 'arcade', 'cyber', 'baroque', 'gala', 'woodblock'].includes(st.id)) materialEnter(s, st, progress, rowBox, draw, 'data');
      // 这些材质已有全幅印面/横向印压/完整图形显影，不会沿份额轴裁去一端。
      else if (['film', 'broadsheet', 'grimoire', 'crt', 'editorial', 'mono', 'neon', 'poster', 'manuscript', 'pastel', 'cutout'].includes(st.id)) materialReveal(s, st, progress, rowBox, 'down', draw);
      // 其余从同一基线显出整条厚度；湿墨/聚焦仍由原材质承担，不能各段各自长宽。
      else reveal(s, progress, rowBox, 'down', () => materialEnter(s, st, progress, rowBox, draw, 'data'));
      // 材质可能从中轴或分栏展开，完整落版后再报百分比，不假定边界总是左扫。
      if (ww > 52 && progress >= 1) s.text(`${Math.round((v / r.tot) * 100)}%`, x + 2, y + 70, { font: st.f.body, size: 13, fill: st.c.soft });
      x += ww;
    }
  });
  return s;
}
