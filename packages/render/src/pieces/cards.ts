// 小票、分享图、徽章、署名行、人的部分。
import { Scene, rect, line, path, circle, el, g, r, ease, measure, rng, hash, mix, alpha } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt, compact, pct, modelName } from '../data.ts';
import type { Style } from '../style.ts';
import { T, zh, colorOf, tip, handFacts, handTip, peopleNames, canvas, range, summary, modelLine, type PieceOpts } from './common.ts';
import { segsOf, segTip } from './tier.ts';

// ───────────────────────── 精力小票 ─────────────────────────

export const RECEIPT_VARIANTS = ['slip', 'ticket', 'itemized'];

function effort(m: Model): { k: string; v: string; d?: string }[] {
  const h = m.hand;
  const items: { k: string; v: string; d?: string }[] = [];
  if (m.spanDays) items.push({ k: T(m, '历时', 'Span'), v: T(m, `${m.spanDays} 天`, `${m.spanDays} days`), d: range(m) });
  if (m.activeDays) items.push({ k: T(m, '活跃天数', 'Active days'), v: T(m, `${m.activeDays} 天`, `${m.activeDays} days`) });
  if (m.sessions) items.push({ k: T(m, '会话', 'Sessions'), v: fmt(m.sessions) });
  if (m.tokens) items.push({ k: 'Token', v: compact(m.tokens) });
  if (h.prompts) items.push({ k: T(m, '提示词', 'Prompts'), v: fmt(h.prompts) });
  items.push({ k: T(m, '提交', 'Commits'), v: fmt(m.commits) });
  if (h.written) items.push({ k: T(m, '新增', 'Added'), v: T(m, `${fmt(h.written)} 行`, `${fmt(h.written)} lines`) });
  items.push({ k: T(m, '提交', 'Committed'), v: T(m, `${fmt(m.linesAdded)} 行`, `${fmt(m.linesAdded)} lines`) });
  if (m.linesSurviving) items.push({ k: T(m, '留存', 'Retained'), v: T(m, `${fmt(m.linesSurviving)} 行`, `${fmt(m.linesSurviving)} lines`) });
  return items;
}

export function receipt(m: Model, st: Style, o: PieceOpts): Scene | null {
  const v = o.variant ?? st.prefer.receipt;
  if (v === 'ticket') return ticket(m, st, o);
  return slip(m, st, v === 'itemized');
}

/** 热敏小票。itemized（明细）：分项按工具列，每个工具下面再逐个列它的模型 */
function slip(m: Model, st: Style, itemized: boolean): Scene {
  const items = effort(m);
  const W = 720, P = 70, lh = 46;
  const list = itemized ? m.tools : m.rows;
  const subs = (a: Agent) => { if (!itemized) return []; const q = segsOf(m, st, a); return q.length > 1 ? q : []; };
  const inner = 250 + items.length * lh + 60 + list.reduce((h, a) => h + 40 + subs(a).length * 30, 0) + 250;
  const { s, c } = canvas(m, st, W, inner, 40);
  // 热敏纸：上下锯齿边
  const px = c.x + 30, pw = c.w - 60, py = c.y + 10, ph = c.h - 20;
  const paper = st.dark ? mix(st.c.paper, '#FFFFFF', 0.04) : st.c.paper;
  let d = `M${r(px)} ${r(py)}`;
  for (let x = px; x < px + pw - 1; x += 14) d += `L${r(Math.min(x + 7, px + pw))} ${r(py - 8)}L${r(Math.min(x + 14, px + pw))} ${r(py)}`;
  d += `L${r(px + pw)} ${r(py + ph)}`;
  for (let x = px + pw; x > px + 1; x -= 14) d += `L${r(Math.max(x - 7, px))} ${r(py + ph + 8)}L${r(Math.max(x - 14, px))} ${r(py + ph)}`;
  s.add(path(d + 'Z', { fill: paper, opacity: st.dark ? 0.92 : 0.96 }));
  const x0 = px + P - 30, x1 = px + pw - P + 30, cx = W / 2;
  let y = py + 70;
  st.label(s, T(m, '小票', 'RECEIPT'), cx, y, { anchor: 'middle', size: 14 });
  st.title(s, m.project || 'Untitled', cx, y + 66, 46, 'middle', { maxW: pw - 80 });
  y += 108;
  const dash = (yy: number) => s.add(line(x0, yy, x1, yy, { stroke: st.c.ink, strokeOpacity: 0.35, strokeDasharray: '5 5' }));
  dash(y); y += 44;
  const mono = st.f.mono;
  for (const it of items) {
    s.text(it.k, x0, y, { font: st.f.body, size: 19, fill: st.c.soft });
    st.num(s, it.v, x1, y, 22, 'end');
    if (it.d) s.text(it.d, x1, y + 22, { font: mono, size: 12, fill: st.c.faint, anchor: 'end' });
    s.hit(x0 - 8, y - 28, x1 - x0 + 16, 40, { t: it.k, d: [it.v, ...(it.d ? [it.d] : [])] });
    y += it.d ? lh + 16 : lh;
  }
  dash(y - 14); y += 30;
  // 分项：各工具
  for (const a of list) {
    st.mark(s, a.key, x0 + 12, y - 7, 20);
    s.text(a.name, x0 + 34, y, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 260 });
    s.text(`× ${fmt(a.lines)}`, x1 - 110, y, { font: mono, size: 15, fill: st.c.soft, anchor: 'end' });
    st.num(s, pct(a.pct), x1, y, 17, 'end');
    s.hit(x0 - 8, y - 24, x1 - x0 + 16, 36, tip(m, a));
    y += 40;
    // 明细：模型一行，缩进，小一号
    for (const q of subs(a)) {
      s.text(`· ${q.name}`, x0 + 34, y - 6, { font: mono, size: 13, fill: st.c.soft, maxW: 300, overflow: 'clip' });
      s.text(`× ${fmt(q.lines)}`, x1 - 110, y - 6, { font: mono, size: 13, fill: st.c.faint, anchor: 'end' });
      s.text(pct(q.pct), x1, y - 6, { font: mono, size: 13, fill: st.c.soft, anchor: 'end' });
      s.hit(x0 + 24, y - 26, x1 - x0 - 16, 28, segTip(m, a, q));
      y += 30;
    }
  }
  dash(y - 8); y += 40;
  s.text(T(m, '合计', 'TOTAL'), x0, y, { font: st.f.display, size: 24, fill: st.c.ink });
  st.num(s, T(m, `${fmt(m.linesAdded)} 行`, `${fmt(m.linesAdded)} lines`), x1, y, 26, 'end');
  y += 50;
  // 条形码：由项目名和数字决定，不是装饰性的随机
  const rnd = rng(hash(m.project + m.commits + m.linesAdded));
  let bx = cx - 150;
  const bars: string[] = [];
  while (bx < cx + 150) { const w = 1 + Math.floor(rnd() * 3.2); bars.push(rect(bx, y, w, 54, { fill: st.c.ink })); bx += w + 1 + Math.floor(rnd() * 3); }
  s.add(g({ opacity: 0.85 }, ...bars));
  s.text(`${m.project.toUpperCase()}-${m.commits}-${m.linesAdded}`, cx, y + 76, { font: mono, size: 12, fill: st.c.soft, anchor: 'middle', track: 0.2 });
  const hf = handFacts(m);
  if (hf.length) { s.text(hf.slice(0, 2).map((f) => f.text).join(T(m, '，', ', ')), cx, y + 112, { font: st.f.body, size: 14, fill: st.c.soft, anchor: 'middle', maxW: pw - 80 }); s.hit(x0, y + 92, x1 - x0, 28, handTip(m)); }
  return s;
}

/** 电影票：横版，右边可撕的票根 */
function ticket(m: Model, st: Style, _o: PieceOpts): Scene {
  const W = 1240;
  const { s, c } = canvas(m, st, W, 480, 40);
  const x = c.x + 20, y = c.y + 20, w = c.w - 40, h = c.h - 40, stub = 300;
  const paper = st.dark ? mix(st.c.paper, '#FFFFFF', 0.05) : st.c.paper;
  // 票面 + 两侧打孔半圆
  const notch = 22;
  s.add(path(`M${r(x)} ${r(y)}H${r(x + w - stub - notch)}a${notch} ${notch} 0 0 0 ${notch * 2} 0H${r(x + w)}V${r(y + h)}H${r(x + w - stub + notch)}a${notch} ${notch} 0 0 0 ${-notch * 2} 0H${r(x)}Z`, { fill: paper, stroke: st.c.ink, strokeOpacity: 0.2 }));
  s.add(line(x + w - stub, y + notch + 6, x + w - stub, y + h - notch - 6, { stroke: st.c.ink, strokeOpacity: 0.4, strokeDasharray: '6 6' }));
  st.label(s, 'ADMIT ONE', x + 50, y + 62, { size: 14 });
  st.title(s, m.project || 'Untitled', x + 50, y + 150, 76, 'start', { maxW: w - stub - 100 });
  const lead = m.rows.filter((a) => a.kind === 'agent').slice(0, 3);
  s.text(T(m, `参与　${lead.map((a) => a.name).join('　')}`, `WITH  ${lead.map((a) => a.name).join('  ')}`), x + 50, y + 210, { font: st.f.body, size: 22, fill: st.c.ink, maxW: w - stub - 100 });
  lead.forEach((a, i) => s.hit(x + 40, y + 182, w - stub - 80, 40, tip(m, a)).hits.length && i);
  const facts = [[T(m, '历时', 'DAYS'), T(m, `${m.spanDays} 天`, `${m.spanDays} days`)], [T(m, '会话', 'SESSIONS'), fmt(m.sessions)], [T(m, '提交', 'COMMITS'), fmt(m.commits)], [T(m, '行', 'LINES'), fmt(m.linesAdded)]];
  facts.forEach(([k, v], i) => {
    const fx = x + 50 + i * ((w - stub - 100) / 4);
    st.label(s, k, fx, y + h - 78, { size: 12 });
    st.num(s, v, fx, y + h - 40, 28, 'start');
  });
  // 票根
  const sx = x + w - stub / 2;
  st.label(s, 'STUB', sx, y + 62, { anchor: 'middle', size: 12 });
  st.num(s, range(m).split(' — ')[1] ?? '', sx, y + 110, 22, 'middle');
  const rnd = rng(hash(m.project));
  const bars: string[] = [];
  for (let k = 0; k < 44; k++) { const bw = 1 + Math.floor(rnd() * 3); bars.push(rect(sx - 110 + k * 5, y + 150, bw, 90, { fill: st.c.ink })); }
  s.add(g({ opacity: 0.8 }, ...bars));
  const hf = handFacts(m);
  if (hf[0]) { s.text(hf[0].text, sx, y + h - 46, { font: st.f.body, size: 16, fill: st.c.soft, anchor: 'middle' }); s.hit(sx - 120, y + h - 70, 240, 36, handTip(m)); }
  return s;
}

// ───────────────────────── 分享图（1200×630） ─────────────────────────

export function share(m: Model, st: Style, o: PieceOpts): Scene | null {
  const t = o.t ?? 1;
  const W = 1200;
  const { s, c } = canvas(m, st, W, 550, 40, true);
  const x0 = c.x + 56, x1 = c.x + c.w - 56;
  // 顶上的小标签和底下一行贴着四角：底板角上有装饰（巴洛克的卷草）时往里让开
  const kx = Math.max(0, (c.corner ?? 0) + 14 - 56);
  st.label(s, T(m, '署名', 'CREDITS'), x0 + kx, c.y + 70, { size: 14 });
  st.title(s, m.project || 'Untitled', x0, c.y + 170, 96, 'start', { maxW: x1 - x0 });
  // 工具数、模型数不随分组方式变
  const ag = [...new Set(m.agents.filter((a) => a.kind === 'agent').map((a) => a.tool))];
  const models = [...new Set(m.agents.filter((a) => a.kind === 'agent').flatMap((a) => (a.model ? [modelName(a.model)] : a.models.map((q) => q.name))))];
  s.text(T(m, `${ag.length} 个 AI 工具 · ${models.length} 个模型 · ${fmt(m.linesAdded)} 行${m.spanDays ? ` · ${m.spanDays} 天` : ''}`, `${ag.length} AI tools · ${models.length} models · ${fmt(m.linesAdded)} lines${m.spanDays ? ` · ${m.spanDays} days` : ''}`), x0, c.y + 228, { font: st.f.body, size: 24, fill: st.c.soft, maxW: x1 - x0 });
  s.hit(x0, c.y + 200, x1 - x0, 40, { t: T(m, '模型', 'Models'), d: models });
  // 一条贯穿的堆叠条
  const by = c.y + c.h - 196, bw = x1 - x0, bh = 30;
  let x = x0;
  m.rows.forEach((a, i) => {
    const w = (bw * a.pct) / 100 * ease(t, i, m.rows.length);
    if (a.key === 'unattributed') s.add(rect(x, by, w, bh, { fill: st.hatch(s) }));
    else st.bar(s, x, by, Math.max(0, w - 2), bh, colorOf(st, a), i);
    s.hit(x, by - 6, w, bh + 12, tip(m, a));
    x += w;
  });
  // 图例：色块 + 标志 + 名字 + 占比，按各自的宽度从左往右排，色块和上面条里的颜色一一对应
  const top = m.rows.filter((a) => a.kind === 'agent').slice(0, 4);
  let lx = x0;
  const ly = c.y + c.h - 110;
  for (const a of top) {
    // 有的风格字体比度量表宽一点，留出余量
    const nw = measure(a.name, st.f.display, 22) * 1.06, pw = measure(pct(a.pct), st.f.num, 22) * 1.06;
    const w = 14 + 10 + 26 + 8 + nw + 14 + pw;
    if (lx + w > x1) break;
    st.bar(s, lx, ly - 15, 14, 14, colorOf(st, a), a.rank);
    st.mark(s, a.key, lx + 24 + 13, ly - 8, 24);
    s.text(a.name, lx + 24 + 26 + 8, ly, { font: st.f.display, size: 22, fill: st.c.ink });
    st.num(s, pct(a.pct), lx + w, ly, 22, 'end', { fill: st.c.soft });
    s.hit(lx, ly - 30, w, 44, tip(m, a));
    lx += w + 40;
  }
  s.text(summary(m), x0 + kx, c.y + c.h - 50, { font: st.f.body, size: 17, fill: st.c.soft });
  const hf = handFacts(m)[0];
  if (hf) s.text(hf.text, x1 - kx, c.y + c.h - 50, { font: st.f.body, size: 17, fill: st.c.soft, anchor: 'end' });
  return s;
}

// ───────────────────────── 徽章 ─────────────────────────

export const BADGE_VARIANTS = ['flat', 'stack', 'emblem', 'split'];

export function badge(m: Model, st: Style, o: PieceOpts): Scene | null {
  const v = o.variant ?? 'flat';
  const lead = m.rows.filter((a) => a.kind === 'agent').slice(0, 3);
  if (!lead.length) return null;
  if (v === 'emblem' && st.grid) {
    // 字符格风格（原版终端）：圆章换成一个方框，框顶嵌一行字，像终端里的面板标题
    const S = 260;
    const s = new Scene(S, S);
    s.bg = st.c.bg;
    s.add(rect(4, 4, S - 8, S - 8, { fill: st.c.paper }));
    const head = T(m, ` ${m.project} · AI 参与编写 `, ` ${m.project} · written with AI `);
    const hw = Math.min(measure(head, st.f.mono, 14), S - 56);
    s.add(path(`M${r(S / 2 - hw / 2 - 4)} 18H18V${S - 18}H${S - 18}V18H${r(S / 2 + hw / 2 + 4)}`, { fill: 'none', stroke: st.c.ink, strokeOpacity: 0.55, strokeWidth: 1.5 }));
    s.text(head, S / 2, 23, { font: st.f.mono, size: 14, fill: st.c.ink, anchor: 'middle', maxW: S - 56 });
    st.mark(s, lead[0].key, S / 2, S / 2 - 6, 48);
    st.num(s, pct(lead[0].pct), S / 2, S / 2 + 48, 18, 'middle');
    st.track(s, 48, S / 2 + 70, S - 96, 9);
    st.bar(s, 48, S / 2 + 70, ((S - 96) * lead[0].pct) / 100, 9, colorOf(st, lead[0]), 0);
    s.hit(18, 18, S - 36, S - 36, tip(m, lead[0]));
    return s;
  }
  if (v === 'emblem') {
    const S = 260;
    const s = new Scene(S, S);
    s.bg = st.c.bg;
    s.add(circle(S / 2, S / 2, S / 2 - 4, { fill: st.c.paper, stroke: st.c.ink, strokeWidth: 3 }));
    s.add(circle(S / 2, S / 2, S / 2 - 16, { fill: 'none', stroke: st.c.ink, strokeWidth: 1, strokeOpacity: 0.5 }));
    // 外圈一圈字
    const txt = T(m, `· ${m.project} · AI 参与编写 `, `· ${m.project.toUpperCase()} · WRITTEN WITH AI `);
    const R0 = S / 2 - 46;
    const id = s.def('ring', (id2) => el('path', { id: id2, d: `M${S / 2} ${S / 2}m-${R0} 0a${R0} ${R0} 0 1 1 ${2 * R0} 0a${R0} ${R0} 0 1 1 ${-2 * R0} 0` }));
    s.add(el('text', { fontFamily: st.f.label.family, fontWeight: st.f.label.weight ?? 500, fontSize: 15, fill: st.c.ink, letterSpacing: 3 }, el('textPath', { href: `#${id}`, startOffset: '0' }, txt)));
    st.mark(s, lead[0].key, S / 2, S / 2 - 10, 48);
    st.num(s, pct(lead[0].pct), S / 2, S / 2 + 40, 18, 'middle');
    s.hit(40, 40, S - 80, S - 80, tip(m, lead[0]));
    return s;
  }
  if (v === 'split') return splitBadge(m, st);
  if (v === 'stack') {
    const W = 420, H = 76;
    const s = new Scene(W, H);
    s.bg = st.c.paper;
    s.add(rect(0.5, 0.5, W - 1, H - 1, { fill: st.c.paper, stroke: st.c.ink, strokeOpacity: 0.25, rx: Math.min(st.radius, 14) }));
    s.text(T(m, 'AI 参与编写', 'Written with AI'), 18, 28, { font: st.f.label, size: 13, fill: st.c.soft });
    let x = 18;
    m.rows.forEach((a, i) => {
      const w = ((W - 36) * a.pct) / 100;
      // 字符格风格：条是一排方块字
      if (st.cells) st.bar(s, x, 42, w, 14, a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a), i);
      else if (a.key === 'unattributed') s.add(rect(x, 42, w, 14, { fill: st.hatch(s) }));
      else s.add(rect(x, 42, Math.max(0, w - 1.5), 14, { fill: colorOf(st, a) }));
      s.hit(x, 36, w, 26, tip(m, a));
      x += w;
      void i;
    });
    s.text(lead.map((a) => `${a.name} ${Math.round(a.pct)}%`).join('  '), W - 18, 28, { font: st.f.body, size: 13, fill: st.c.ink, anchor: 'end', maxW: W - 160 });
    return s;
  }
  // flat：shields 式
  const left = T(m, 'AI 参与', 'written with');
  const right = lead.slice(0, 2).map((a) => `${a.name} ${Math.round(a.pct)}%`).join(' · ');
  const f = { family: 'Inter, Noto Sans SC', weight: 600 };
  const lw = measure(left, f, 13) + 44, rw = measure(right, f, 13) + 24;
  const W = Math.ceil(lw + rw), H = 30;
  const s = new Scene(W, H);
  s.bg = st.c.bg;
  const dark = st.dark ? st.c.paper : st.c.ink;
  s.add(rect(0, 0, W, H, { fill: dark, rx: 6 }), rect(lw, 0, rw, H, { fill: colorOf(st, lead[0]) }), rect(W - 6, 0, 6, H, { fill: colorOf(st, lead[0]), rx: 0 }));
  s.add(rect(lw, 0, rw, H, { fill: colorOf(st, lead[0]), rx: 6 }), rect(lw, 0, 8, H, { fill: colorOf(st, lead[0]) }));
  st.mark(s, lead[0].key, 18, H / 2, 16, st.dark ? st.c.ink : st.c.paper);
  s.text(left, 32, 20, { font: f, size: 13, fill: st.dark ? st.c.ink : st.c.paper });
  s.text(right, lw + 12, 20, { font: f, size: 13, fill: st.on(colorOf(st, lead[0])) });
  s.hit(0, 0, W, H, tip(m, lead[0]));
  return s;
}

/** 分段徽章：条按工具分块、块里按模型分深浅；文字写第一的工具和它的前两个模型 */
function splitBadge(m: Model, st: Style): Scene | null {
  const tools = m.tools.filter((a) => a.lines > 0);
  const top = tools.find((a) => a.kind === 'agent');
  if (!top) return null;
  const W = 460, H = 96;
  const s = new Scene(W, H);
  s.bg = st.c.paper;
  s.add(rect(0.5, 0.5, W - 1, H - 1, { fill: st.c.paper, stroke: st.c.ink, strokeOpacity: 0.25, rx: Math.min(st.radius, 14) }));
  s.text(T(m, 'AI 参与编写', 'Written with AI'), 18, 28, { font: st.f.label, size: 13, fill: st.c.soft });
  s.text(`${top.name} ${Math.round(top.pct)}%`, W - 18, 28, { font: st.f.body, size: 13, fill: st.c.ink, anchor: 'end', maxW: W - 160 });
  let x = 18;
  const bw = W - 36;
  for (const a of tools) {
    const w = (bw * a.pct) / 100;
    let sx = x;
    for (const q of segsOf(m, st, a)) {
      const qw = (w * q.lines) / Math.max(1, a.lines);
      if (st.cells) st.bar(s, sx, 40, qw, 14, a.key === 'unattributed' ? st.hatch(s) : q.color, 0);
      else if (a.key === 'unattributed') s.add(rect(sx, 40, qw, 14, { fill: st.hatch(s) }));
      else s.add(rect(sx, 40, Math.max(0, qw - 0.75), 14, { fill: q.color }));
      s.hit(sx, 34, qw, 26, segTip(m, a, q));
      sx += qw;
    }
    // 工具之间留一道缝，比模型之间宽
    x += w;
    if (w > 3 && !st.cells) s.add(rect(x - 2, 40, 2, 14, { fill: st.c.paper }));
  }
  const segs = segsOf(m, st, top).filter((q) => !q.unknown).slice(0, 2);
  const sub = segs.length > 1 ? segs.map((q) => `${q.name} ${Math.round(q.pct)}%`).join(' · ') : modelLine(top);
  if (sub) s.text(sub, 18, 78, { font: st.f.body, size: 13, fill: st.c.soft, maxW: W - 36, overflow: 'clip' });
  return s;
}

// ───────────────────────── 署名行（页脚） ─────────────────────────

export const BYLINE_VARIANTS = ['line', 'models'];

export function byline(m: Model, st: Style, o: PieceOpts): Scene | null {
  const models = o.variant === 'models';
  const lead = (models ? m.tools : m.rows).filter((a) => a.kind === 'agent');
  if (!lead.length) return null;
  const W = 1200, H = 120;
  const s = new Scene(W, H);
  // 先排固定信息：右边的统计、句尾「写成」，再把剩下的宽度分给名字
  const sum = summary(m), sumW = measure(sum, st.f.label, 14) * 1.04;
  // 窄幅有自己的底（装饰让开中间的文字带）；没有就用大画幅的底
  if (st.strip) st.strip(s, { x: 0, y: 0, w: W, h: H }, m, { x: W - 48 - sumW, y: 58, w: sumW, h: 18 });
  else st.ground(s, { x: 0, y: 0, w: W, h: H }, m);
  const tail = T(m, '写成', '');
  const nameY = models ? 66 : 72;
  let x = 48;
  x += st.title(s, m.project, x, 74, 34, 'start', { maxW: 300 }) + 26;
  s.add(line(x, 44, x, 80, { stroke: st.c.ink, strokeOpacity: 0.3 }));
  x += 26;
  x += s.text(T(m, '用', 'with'), x, nameY, { font: st.f.body, size: 20, fill: st.c.soft }) + 16;
  const end = W - 48 - sumW - 36;
  const subOf = (a: Agent) => { const segs = segsOf(m, st, a).filter((q) => !q.unknown); return segs.length > 1 ? segs.slice(0, 3).map((q) => q.name).join(' · ') : modelLine(a); };
  const more = (k: number) => T(m, `等 ${k} 个`, `+${k} more`);
  const plan = bylinePlan(lead, models ? 3 : 4, st, x, end, tail, more, models ? subOf : null);
  for (const it of plan.items) {
    const a = it.a;
    st.mark(s, a.key, it.x + 12, nameY - 8, 22);
    s.text(it.name, it.x + 30, nameY, { font: st.f.body, size: plan.size, fill: st.c.ink });
    if (it.sub) s.text(it.sub, it.x + 30, 90, { font: st.f.body, size: 13, fill: st.c.soft, maxW: it.subW, overflow: 'clip' });
    s.hit(it.x - 4, models ? 38 : 44, it.w + 34, models ? 60 : 40, tip(m, a));
  }
  let tx = plan.end;
  if (plan.more) tx += s.text(more(plan.more), tx, nameY, { font: st.f.body, size: plan.size, fill: st.c.soft }) + 10;
  if (tail) s.text(tail, tx, nameY, { font: st.f.body, size: plan.size, fill: st.c.soft });
  s.text(sum, W - 48, 72, { font: st.f.label, size: 14, fill: st.c.faint, anchor: 'end' });
  return s;
}

/** 署名行里名字怎么排：字号从 20 往下试到 16；还放不下先去掉名字后面的说明（「（未记录模型）」「· 工具」），
 *  再从后往前收进「等 N 个」，最后才截断。名字之间 34；句尾跟在最后一个名字后面（带模型时模型小字可以伸到句尾下面） */
function bylinePlan(lead: Agent[], cap: number, st: Style, x0: number, end: number, tail: string, more: (k: number) => string, subOf: ((a: Agent) => string) | null) {
  const GAP = 34, LOGO = 30;
  const short = (n: string) => n.replace(/\s*(（[^）]+）|\([^)]+\)|·\s.+)$/, '') || n;
  const tryAt = (size: number, n: number, bare: boolean, clipTo = Infinity) => {
    const moreW = n < lead.length ? measure(more(lead.length - n), st.f.body, size) + 10 : 0;
    const tailW = tail ? measure(tail, st.f.body, size) + 14 : 0;
    let x = x0, stop = x0, reach = x0;
    const items = lead.slice(0, n).map((a, k) => {
      let name = bare ? short(a.name) : a.name, nw = measure(name, st.f.body, size) * 1.03;
      if (nw > clipTo) { name = clipName(name, st, size, clipTo); nw = measure(name, st.f.body, size); }
      const sub = subOf?.(a) ?? '';
      const last = k === n - 1;
      // 模型小字：最后一项可以伸到句尾下面，一直到统计前面；其余的不超过下一项
      const subMax = last ? Math.max(nw, end - x - LOGO) : Math.max(nw, 300);
      const subW = sub ? Math.min(subMax, measure(sub, st.f.body, 13)) : 0;
      const w = last ? nw : Math.max(nw, subW);
      const it = { a, name, sub, subW: Math.max(subW, nw), x, w };
      stop = x + LOGO + nw + 14;
      reach = Math.max(stop + moreW + tailW, x + LOGO + subW);
      x += LOGO + w + GAP;
      return it;
    });
    return { size, items, end: stop, more: lead.length - n, fits: reach <= end };
  };
  const N = Math.min(cap, lead.length);
  for (const bare of [false, true])
    for (const size of [20, 18, 16]) { const p = tryAt(size, N, bare); if (p.fits) return p; }
  for (let n = N - 1; n >= 1; n--)
    for (const size of [20, 18, 16]) { const p = tryAt(size, n, true); if (p.fits) return p; }
  // 只剩一个名字还放不下：截断它
  return tryAt(16, 1, true, Math.max(80, end - x0 - LOGO - 60));
}
const clipName = (s: string, st: Style, size: number, maxW: number) => { const c = [...s]; while (c.length > 1 && measure(c.join('') + '…', st.f.body, size) > maxW) c.pop(); return c.join('').trimEnd() + '…'; };

// ───────────────────────── 人的部分 ─────────────────────────

/** 人：AI 写的代码提交了多少、留下多少；人发了多少消息、review 多少次、人写了多少 */
export function hand(m: Model, st: Style, o: PieceOpts): Scene | null {
  const h = m.hand;
  if (!h.prompts && !h.reviews && !h.handLines && !h.written) return null;
  const t = o.t ?? 1;
  const W = 1080;
  const nSteps = [h.written, h.committed, h.surviving].filter((v) => v > 0).length;
  const { s, c } = canvas(m, st, W, 300 + nSteps * 130 + 230 + 150 + (m.sign || m.named ? 60 : 0), 40);
  const x0 = c.x + 60, x1 = c.x + c.w - 60, cx = W / 2;
  st.label(s, T(m, '开发者', 'DEVELOPER'), x0, c.y + 70, { size: 15, fill: st.c.accent2 });
  st.title(s, m.project, x0, c.y + 150, 64, 'start', { maxW: x1 - x0 });
  s.text(T(m, '新增代码留存变化，以及开发者的提示词与手写代码。', 'Code retention progression, prompts, and manual lines.'), x0, c.y + 200, { font: st.f.body, size: 20, fill: st.c.soft, maxW: x1 - x0 });
  // 取舍漏斗：写下 → 提交 → 留下
  let y = c.y + 270;
  const steps = [
    { k: T(m, '新增', 'Added'), v: h.written },
    { k: T(m, '提交', 'Committed'), v: h.committed },
    { k: T(m, '留存', 'Retained'), v: h.surviving },
  ].filter((q) => q.v > 0);
  const max = Math.max(...steps.map((q) => q.v), 1);
  steps.forEach((q, i) => {
    const w = (x1 - x0) * (q.v / max) * ease(t, i, steps.length);
    const bx = cx - w / 2;
    const col = mix(st.c.accent2.startsWith('#') ? st.c.accent2 : '#888888', st.dark ? '#000000' : '#FFFFFF', 0.15 + i * 0.25);
    st.bar(s, bx, y, w, 64, col, i);
    s.text(q.k, x0, y - 14, { font: st.f.label, size: 15, fill: st.c.soft });
    st.num(s, fmt(q.v), x1, y - 12, 22, 'end');
    s.hit(x0, y - 40, x1 - x0, 110, { k: 'human', t: q.k, d: [T(m, `${fmt(q.v)} 行`, `${fmt(q.v)} lines`), ...(i ? [T(m, `是上一项的 ${Math.round((q.v / steps[i - 1].v) * 100)}%`, `${Math.round((q.v / steps[i - 1].v) * 100)}% of the previous step`)] : [])] });
    y += 130;
  });
  // 指令的脉搏：每周会话里发出的指令（没有就用会话数）
  const weekly = pulse(m);
  if (weekly.vals.length > 2) {
    y += 20;
    st.label(s, weekly.label, x0, y, { size: 14 });
    const ph = 140, top = y + 20, maxv = Math.max(...weekly.vals, 1);
    const step = (x1 - x0) / (weekly.vals.length - 1);
    let d = '';
    weekly.vals.forEach((v, i) => { const px = x0 + i * step, py = top + ph - (v / maxv) * ph * ease(t, i, weekly.vals.length); d += `${i ? 'L' : 'M'}${r(px)} ${r(py)}`; });
    s.add(path(d + `L${r(x1)} ${r(top + ph)}L${r(x0)} ${r(top + ph)}Z`, { fill: st.c.accent2, opacity: 0.12 }), path(d, { fill: 'none', stroke: st.c.accent2, strokeWidth: 2.4, strokeLinejoin: 'round' }));
    st.rule(s, x0, top + ph, x1 - x0, 'thin');
    s.hit(x0, top, x1 - x0, ph, { k: 'human', t: weekly.label, d: [T(m, `${weekly.vals.length} 周，最多一周 ${fmt(maxv)}`, `${weekly.vals.length} weeks, peak ${fmt(maxv)}`)] });
    y = top + ph + 50;
  }
  // 事实格
  const facts = [
    [T(m, '提示词', 'Prompts'), h.prompts], [T(m, '代码审阅', 'Reviews'), h.reviews], [T(m, '手写行', 'Manual lines'), h.handLines], [T(m, 'Tab 补全行', 'Tab lines'), h.tabLines],
  ].filter(([, v]) => (v as number) > 0) as [string, number][];
  const fw = (x1 - x0) / Math.max(1, facts.length);
  facts.forEach(([k, v], i) => {
    const fx = x0 + i * fw;
    st.num(s, fmt(v), fx, y + 40, 40, 'start');
    s.text(k, fx, y + 72, { font: st.f.body, size: 16, fill: st.c.soft });
    s.hit(fx - 6, y, fw - 10, 86, handTip(m));
  });
  y += 120;
  const names = peopleNames(m, 6);
  if (names || m.sign) s.text(m.sign ? T(m, `—— ${m.sign}`, `— ${m.sign}`) : names, x1, Math.min(y + 40, c.y + c.h - 40), { font: st.f.script ?? st.f.display, size: 30, fill: st.c.ink, anchor: 'end' });
  return s;
}
function pulse(m: Model): { label: string; vals: number[] } {
  const byWeek = new Map<string, { p: number; s: number }>();
  for (const x of m.timeline) {
    if (x.review) continue;
    const d = new Date(x.start * 1000);
    const wk = `${d.getUTCFullYear()}-${Math.floor((d.getTime() - Date.UTC(d.getUTCFullYear(), 0, 1)) / 6048e5)}`;
    const e = byWeek.get(wk) ?? { p: 0, s: 0 };
    e.p += x.prompts; e.s += 1;
    byWeek.set(wk, e);
  }
  const vals = [...byWeek.entries()].sort((a, b) => a[0].localeCompare(b[0], 'en', { numeric: true })).map(([, v]) => v);
  const hasPrompts = vals.some((v) => v.p > 0);
  return { label: hasPrompts ? T(m, '每周发给 AI 的消息', 'Messages to AI per week') : T(m, '每周的会话数', 'Sessions per week'), vals: vals.map((v) => (hasPrompts ? v.p : v.s)) };
}

void alpha; void modelLine; void zh;
export type { Agent };
