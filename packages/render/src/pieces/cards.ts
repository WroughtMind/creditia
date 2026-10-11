// 小票、分享图、徽章、署名行、人的部分。
import { Scene, rect, line, path, circle, el, g, r, ease, measure, rng, hash, mix, alpha } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt, compact, pct } from '../data.ts';
import type { Style } from '../style.ts';
import { T, zh, colorOf, tip, handFacts, handTip, peopleNames, canvas, range, summary, modelLine, type PieceOpts } from './common.ts';

// ───────────────────────── 精力小票 ─────────────────────────

export const RECEIPT_VARIANTS = ['slip', 'ticket'];

function effort(m: Model): { k: string; v: string; d?: string }[] {
  const h = m.hand;
  const items: { k: string; v: string; d?: string }[] = [];
  if (m.spanDays) items.push({ k: T(m, '工期', 'Span'), v: T(m, `${m.spanDays} 天`, `${m.spanDays} days`), d: range(m) });
  if (m.activeDays) items.push({ k: T(m, '动工', 'Active days'), v: T(m, `${m.activeDays} 天`, `${m.activeDays} days`) });
  if (m.sessions) items.push({ k: T(m, '会话', 'Sessions'), v: fmt(m.sessions) });
  if (m.tokens) items.push({ k: 'Token', v: compact(m.tokens) });
  if (h.prompts) items.push({ k: T(m, '指令', 'Prompts'), v: fmt(h.prompts) });
  items.push({ k: T(m, '提交', 'Commits'), v: fmt(m.commits) });
  if (h.written) items.push({ k: T(m, 'agent 写下', 'Generated'), v: T(m, `${fmt(h.written)} 行`, `${fmt(h.written)} lines`) });
  items.push({ k: T(m, '进入仓库', 'Committed'), v: T(m, `${fmt(m.linesAdded)} 行`, `${fmt(m.linesAdded)} lines`) });
  if (m.linesSurviving) items.push({ k: T(m, '至今仍在', 'Still here'), v: T(m, `${fmt(m.linesSurviving)} 行`, `${fmt(m.linesSurviving)} lines`) });
  return items;
}

export function receipt(m: Model, st: Style, o: PieceOpts): Scene | null {
  if ((o.variant ?? st.prefer.receipt) === 'ticket') return ticket(m, st, o);
  const items = effort(m);
  const W = 720, P = 70, lh = 46;
  const inner = 250 + items.length * lh + 60 + m.rows.length * 40 + 250;
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
  st.label(s, T(m, '精 力 小 票', 'RECEIPT'), cx, y, { anchor: 'middle', size: 14 });
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
  for (const a of m.rows) {
    st.mark(s, a.key, x0 + 12, y - 7, 20);
    s.text(a.name, x0 + 34, y, { font: st.f.body, size: 17, fill: st.c.ink, maxW: 260 });
    s.text(`× ${fmt(a.lines)}`, x1 - 110, y, { font: mono, size: 15, fill: st.c.soft, anchor: 'end' });
    st.num(s, pct(a.pct), x1, y, 17, 'end');
    s.hit(x0 - 8, y - 24, x1 - x0 + 16, 36, tip(m, a));
    y += 40;
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
  if (hf.length) { s.text(T(m, `人：${hf.slice(0, 2).map((f) => `${f.v} ${f.k}`).join('，')}`, `Human: ${hf.slice(0, 2).map((f) => `${f.v} ${f.k}`).join(', ')}`), cx, y + 112, { font: st.f.body, size: 14, fill: st.c.soft, anchor: 'middle', maxW: pw - 80 }); s.hit(x0, y + 92, x1 - x0, 28, handTip(m)); }
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
  st.label(s, T(m, '一张票 · 一部作品', 'ADMIT ONE'), x + 50, y + 62, { size: 14 });
  st.title(s, m.project || 'Untitled', x + 50, y + 150, 76, 'start', { maxW: w - stub - 100 });
  const lead = m.rows.filter((a) => a.kind === 'agent').slice(0, 3);
  s.text(T(m, `主演　${lead.map((a) => a.name).join('　')}`, `STARRING  ${lead.map((a) => a.name).join('  ')}`), x + 50, y + 210, { font: st.f.body, size: 22, fill: st.c.ink, maxW: w - stub - 100 });
  lead.forEach((a, i) => s.hit(x + 40, y + 182, w - stub - 80, 40, tip(m, a)).hits.length && i);
  const facts = [[T(m, '片长', 'RUNTIME'), T(m, `${m.spanDays} 天`, `${m.spanDays} days`)], [T(m, '场次', 'SESSIONS'), fmt(m.sessions)], [T(m, '提交', 'COMMITS'), fmt(m.commits)], [T(m, '行', 'LINES'), fmt(m.linesAdded)]];
  facts.forEach(([k, v], i) => {
    const fx = x + 50 + i * ((w - stub - 100) / 4);
    st.label(s, k, fx, y + h - 78, { size: 12 });
    st.num(s, v, fx, y + h - 40, 28, 'start');
  });
  // 票根
  const sx = x + w - stub / 2;
  st.label(s, T(m, '票根', 'STUB'), sx, y + 62, { anchor: 'middle', size: 12 });
  st.num(s, range(m).split(' — ')[1] ?? '', sx, y + 110, 22, 'middle');
  const rnd = rng(hash(m.project));
  const bars: string[] = [];
  for (let k = 0; k < 44; k++) { const bw = 1 + Math.floor(rnd() * 3); bars.push(rect(sx - 110 + k * 5, y + 150, bw, 90, { fill: st.c.ink })); }
  s.add(g({ opacity: 0.8 }, ...bars));
  const hf = handFacts(m);
  if (hf[0]) { s.text(`${hf[0].v} ${hf[0].k}`, sx, y + h - 46, { font: st.f.body, size: 16, fill: st.c.soft, anchor: 'middle' }); s.hit(sx - 120, y + h - 70, 240, 36, handTip(m)); }
  return s;
}

// ───────────────────────── 分享图（1200×630） ─────────────────────────

export function share(m: Model, st: Style, o: PieceOpts): Scene | null {
  const t = o.t ?? 1;
  const W = 1200;
  const { s, c } = canvas(m, st, W, 550, 40);
  const x0 = c.x + 56, x1 = c.x + c.w - 56;
  st.label(s, T(m, '人与 AI 一起写成', 'MADE WITH AI'), x0, c.y + 70, { size: 14 });
  st.title(s, m.project || 'Untitled', x0, c.y + 170, 96, 'start', { maxW: x1 - x0 });
  const ag = m.agents.filter((a) => a.kind === 'agent');
  const models = [...new Set(ag.flatMap((a) => a.models.map((q) => q.name)))];
  s.text(T(m, `${ag.length} 个 AI 工具、${models.length} 个模型，${m.spanDays ? `${m.spanDays} 天` : ''}写成 ${fmt(m.linesAdded)} 行`, `${ag.length} AI tools, ${models.length} models, ${fmt(m.linesAdded)} lines${m.spanDays ? ` in ${m.spanDays} days` : ''}`), x0, c.y + 228, { font: st.f.body, size: 24, fill: st.c.soft, maxW: x1 - x0 });
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
  // 前三名
  const top = m.rows.filter((a) => a.kind === 'agent').slice(0, 3);
  top.forEach((a, i) => {
    const tx = x0 + i * (bw / 3);
    st.mark(s, a.key, tx + 14, c.y + c.h - 120, 28);
    s.text(a.name, tx + 40, c.y + c.h - 110, { font: st.f.display, size: 24, fill: st.c.ink, maxW: bw / 3 - 140 });
    st.num(s, pct(a.pct), tx + bw / 3 - 30, c.y + c.h - 110, 24, 'end', { fill: st.c.soft });
    s.hit(tx, c.y + c.h - 150, bw / 3 - 10, 54, tip(m, a));
  });
  s.text(summary(m), x0, c.y + c.h - 50, { font: st.f.body, size: 17, fill: st.c.soft });
  const hf = handFacts(m)[0];
  if (hf) s.text(`${hf.v} ${hf.k}`, x1, c.y + c.h - 50, { font: st.f.body, size: 17, fill: st.c.soft, anchor: 'end' });
  return s;
}

// ───────────────────────── 徽章 ─────────────────────────

export const BADGE_VARIANTS = ['flat', 'stack', 'emblem'];

export function badge(m: Model, st: Style, o: PieceOpts): Scene | null {
  const v = o.variant ?? 'flat';
  const lead = m.rows.filter((a) => a.kind === 'agent').slice(0, 3);
  if (!lead.length) return null;
  if (v === 'emblem') {
    const S = 260;
    const s = new Scene(S, S);
    s.bg = st.c.bg;
    s.add(circle(S / 2, S / 2, S / 2 - 4, { fill: st.c.paper, stroke: st.c.ink, strokeWidth: 3 }));
    s.add(circle(S / 2, S / 2, S / 2 - 16, { fill: 'none', stroke: st.c.ink, strokeWidth: 1, strokeOpacity: 0.5 }));
    // 外圈一圈字
    const txt = T(m, `· 人 与 AI 合 作 · ${m.project} `, `· MADE WITH AI · ${m.project.toUpperCase()} `);
    const R0 = S / 2 - 46;
    const id = s.def('ring', (id2) => el('path', { id: id2, d: `M${S / 2} ${S / 2}m-${R0} 0a${R0} ${R0} 0 1 1 ${2 * R0} 0a${R0} ${R0} 0 1 1 ${-2 * R0} 0` }));
    s.add(el('text', { fontFamily: st.f.label.family, fontWeight: st.f.label.weight ?? 500, fontSize: 15, fill: st.c.ink, letterSpacing: 3 }, el('textPath', { href: `#${id}`, startOffset: '0' }, txt)));
    st.mark(s, lead[0].key, S / 2, S / 2 - 10, 48);
    st.num(s, pct(lead[0].pct), S / 2, S / 2 + 40, 18, 'middle');
    s.hit(40, 40, S - 80, S - 80, tip(m, lead[0]));
    return s;
  }
  if (v === 'stack') {
    const W = 420, H = 76;
    const s = new Scene(W, H);
    s.bg = st.c.paper;
    s.add(rect(0.5, 0.5, W - 1, H - 1, { fill: st.c.paper, stroke: st.c.ink, strokeOpacity: 0.25, rx: Math.min(st.radius, 14) }));
    s.text(T(m, '与 AI 一起写成', 'Built with AI'), 18, 28, { font: st.f.label, size: 13, fill: st.c.soft });
    let x = 18;
    m.rows.forEach((a, i) => {
      const w = ((W - 36) * a.pct) / 100;
      if (a.key === 'unattributed') s.add(rect(x, 42, w, 14, { fill: st.hatch(s) }));
      else s.add(rect(x, 42, Math.max(0, w - 1.5), 14, { fill: colorOf(st, a) }));
      s.hit(x, 36, w, 26, tip(m, a));
      x += w;
      void i;
    });
    s.text(lead.map((a) => `${a.name} ${Math.round(a.pct)}%`).join('  '), W - 18, 28, { font: st.f.body, size: 13, fill: st.c.ink, anchor: 'end', maxW: W - 160 });
    return s;
  }
  // flat：shields 式
  const left = T(m, '与 AI 写成', 'built with');
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

// ───────────────────────── 署名行（页脚） ─────────────────────────

export function byline(m: Model, st: Style, _o: PieceOpts): Scene | null {
  const lead = m.rows.filter((a) => a.kind === 'agent');
  if (!lead.length) return null;
  const W = 1200, H = 120;
  const s = new Scene(W, H);
  st.ground(s, { x: 0, y: 0, w: W, h: H }, m);
  let x = 48;
  x += st.title(s, m.project, x, 74, 34) + 26;
  s.add(line(x, 44, x, 80, { stroke: st.c.ink, strokeOpacity: 0.3 }));
  x += 26;
  x += s.text(T(m, '与', 'with'), x, 72, { font: st.f.body, size: 20, fill: st.c.soft }) + 16;
  for (const a of lead.slice(0, 4)) {
    st.mark(s, a.key, x + 12, 64, 22);
    const w = s.text(a.name, x + 30, 72, { font: st.f.body, size: 20, fill: st.c.ink });
    s.hit(x - 4, 44, w + 40, 40, tip(m, a));
    x += w + 54;
    if (x > W - 300) break;
  }
  s.text(T(m, '一起写成', 'made together'), x - 10, 72, { font: st.f.body, size: 20, fill: st.c.soft });
  s.text(summary(m), W - 48, 72, { font: st.f.label, size: 14, fill: st.c.faint, anchor: 'end' });
  return s;
}

// ───────────────────────── 人的部分 ─────────────────────────

/** 人的部分：不比行数，比的是方向和取舍——发出的指令、审阅、亲手写的、从生成里留下的 */
export function hand(m: Model, st: Style, o: PieceOpts): Scene | null {
  const h = m.hand;
  if (!h.prompts && !h.reviews && !h.handLines && !h.written) return null;
  const t = o.t ?? 1;
  const W = 1080;
  const nSteps = [h.written, h.committed, h.surviving].filter((v) => v > 0).length;
  const { s, c } = canvas(m, st, W, 300 + nSteps * 130 + 230 + 150 + (m.sign || m.named ? 60 : 0), 40);
  const x0 = c.x + 60, x1 = c.x + c.w - 60, cx = W / 2;
  st.label(s, T(m, '人 的 部 分', 'THE HUMAN HAND'), x0, c.y + 70, { size: 15, fill: st.c.accent2 });
  st.title(s, m.project, x0, c.y + 150, 64, 'start', { maxW: x1 - x0 });
  s.text(T(m, '代码大多是 agent 写的；写什么、留下什么，是人决定的。', 'Agents wrote most of the code. People decided what to write and what to keep.'), x0, c.y + 200, { font: st.f.body, size: 20, fill: st.c.soft, maxW: x1 - x0 });
  // 取舍漏斗：写下 → 提交 → 留下
  let y = c.y + 270;
  const steps = [
    { k: T(m, 'agent 在会话里写下', 'Generated in sessions'), v: h.written },
    { k: T(m, '人决定提交', 'Committed by people'), v: h.committed },
    { k: T(m, '至今仍在', 'Still in the code'), v: h.surviving },
  ].filter((q) => q.v > 0);
  const max = Math.max(...steps.map((q) => q.v), 1);
  steps.forEach((q, i) => {
    const w = (x1 - x0) * (q.v / max) * ease(t, i, steps.length);
    const bx = cx - w / 2;
    const col = mix(st.c.accent2.startsWith('#') ? st.c.accent2 : '#888888', st.dark ? '#000000' : '#FFFFFF', 0.15 + i * 0.25);
    st.bar(s, bx, y, w, 64, col, i);
    s.text(q.k, x0, y - 14, { font: st.f.label, size: 15, fill: st.c.soft });
    st.num(s, fmt(q.v), x1, y - 12, 22, 'end');
    s.hit(x0, y - 40, x1 - x0, 110, { k: 'human', t: q.k, d: [T(m, `${fmt(q.v)} 行`, `${fmt(q.v)} lines`), ...(i ? [T(m, `是上一步的 ${Math.round((q.v / steps[i - 1].v) * 100)}%`, `${Math.round((q.v / steps[i - 1].v) * 100)}% of the previous step`)] : [])] });
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
    [T(m, '指令', 'Prompts'), h.prompts], [T(m, '审阅', 'Reviews'), h.reviews], [T(m, '亲手写的行', 'Lines by hand'), h.handLines], [T(m, 'Tab 补全辅助', 'Tab-assisted'), h.tabLines],
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
  return { label: hasPrompts ? T(m, '每周发出的指令', 'Prompts per week') : T(m, '每周开启的会话', 'Sessions per week'), vals: vals.map((v) => (hasPrompts ? v.p : v.s)) };
}

void alpha; void modelLine; void zh;
export type { Agent };
