// Tier：谁写的、各占多少。版式：rows（条形榜）、vertical（竖排，古籍/水墨）、hud（赛博面板）、podium（领奖台）
import { Scene, rect, line, path, circle, el, g, r, ease, measure, capOf, mix, lum } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { pct, fmt, hanzi } from '../data.ts';
import type { Style } from '../style.ts';
import { chamfer } from '../style.ts';
import { measureV } from './common.ts';
import { T, zh, colorOf, fillOf, tip, modelLine, handLine, handTip, peopleNames, summary, range, canvas, type PieceOpts } from './common.ts';
import { inkSeal } from '../styles/east.ts';

export const TIER_VARIANTS = ['rows', 'center', 'podium', 'vertical', 'hud', 'stats', 'split', 'grouped'];

export function tier(m: Model, st: Style, o: PieceOpts): Scene | null {
  if (!m.rows.length) return null;
  const v = o.variant ?? st.prefer.tier ?? 'rows';
  if (v === 'vertical') return vertical(m, st, o);
  if (v === 'hud') return hud(m, st, o);
  if (v === 'podium') return podium(m, st, o);
  if (v === 'center') return center(m, st, o);
  if (v === 'stats') return stats(m, st, o);
  if (v === 'split') return split(m, st, o);
  if (v === 'grouped') return grouped(m, st, o);
  return rows(m, st, o);
}

function rows(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const W = 880, M = 34, P = 50;
  const rowH = 96;
  const hl = handLine(m), names = peopleNames(m);
  const handH = hl ? 112 + (names ? 30 : 0) : 0;
  const { s, c } = canvas(m, st, W, P + 132 + m.rows.length * rowH + 24 + handH + 70, M);
  const x0 = c.x + P, x1 = c.x + c.w - P;
  let y = c.y + P;
  st.label(s, T(m, '署名', 'CREDITS'), x0, y + 10, { size: 13 });
  st.label(s, range(m), x1, y + 10, { size: 12, anchor: 'end', fill: st.c.faint });
  st.title(s, m.project || 'Untitled', x0, y + 78, 56, 'start', { maxW: x1 - x0 });
  y += 132;
  const n = m.rows.length;
  m.rows.forEach((a, i) => {
    const k = ease(t, i, n);
    const color = colorOf(st, a);
    const mx = x0 + 20, nx = x0 + 58;
    st.mark(s, a.key, mx, y + 24, 34, a.key === 'unattributed' ? undefined : undefined);
    s.text(a.name, nx, y + 33, { font: st.f.display, size: 27, fill: st.c.ink, maxW: x1 - nx - 140 });
    const ml = modelLine(a);
    if (ml) s.text(ml, nx, y + 56, { font: st.f.body, size: 14, fill: st.c.soft, maxW: x1 - nx - 140, overflow: 'clip' });
    st.num(s, pct(a.pct * (t >= 1 ? 1 : k)), x1, y + 36, 32, 'end');
    const by = y + 70, bw = x1 - nx, bh = 9;
    st.track(s, nx, by, bw, bh);
    const w = Math.max(a.pct > 0 ? 3 : 0, (bw * a.pct) / 100) * k;
    if (a.key === 'unattributed') s.add(rect(nx, by, w, bh, { fill: st.hatch(s), rx: Math.min(st.radius, bh / 2) }));
    else st.bar(s, nx, by, w, bh, color, i);
    s.hit(x0 - 8, y - 6, x1 - x0 + 16, rowH - 4, tip(m, a));
    y += rowH;
  });
  if (hl) {
    y += 18;
    st.rule(s, x0, y, x1 - x0, 'thin');
    y += 40;
    st.label(s, T(m, '人', 'PEOPLE'), x0, y, { size: 13, fill: st.c.accent2 });
    s.text(hl, x0, y + 34, { font: st.f.body, size: 18, fill: st.c.ink, maxW: x1 - x0 });
    if (names) s.text(names, x0, y + 64, { font: st.f.body, size: 15, fill: st.c.soft, maxW: x1 - x0 });
    s.hit(x0 - 8, y - 22, x1 - x0 + 16, names ? 96 : 66, handTip(m));
    y += handH - 58;
  }
  y += 40;
  s.text(summary(m), x0, y, { font: st.f.body, size: 14, fill: st.c.soft });
  s.text('creditia', x1, y, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end', track: 0.12 });
  return s;
}

/** 竖排：右起，一列一位贡献方；名字竖写，占比用汉字 */
function vertical(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const list = m.rows;
  const colW = 108, M = 34;
  const W = Math.max(720, M * 2 + 150 + list.length * colW + 120);
  const { s, c } = canvas(m, st, W, 912, M);
  // 题签：最右一列，项目名竖写
  const tx = c.x + c.w - 70;
  const th = Math.min(560, measureV(m.project || '', st, 46) + 56);
  s.add(rect(tx - 34, c.y + 40, 68, th, { fill: st.c.paper, stroke: st.c.ink, strokeWidth: 1.2 }));
  s.vtext(m.project || '', tx, c.y + 66, { font: st.f.display, size: 46, fill: st.c.ink });
  inkSeal(s, tx, c.y + 40 + th + 46, 52, zh(m) ? '合作' : 'AI', { color: st.c.accent });
  // 界栏
  const top = c.y + 40, bottom = c.y + c.h - 120;
  const xs = (i: number) => tx - 120 - i * colW;
  for (let i = 0; i <= list.length; i++) s.add(line(xs(i) + colW / 2, top, xs(i) + colW / 2, bottom, { stroke: st.c.ink, strokeOpacity: 0.35, strokeWidth: 0.8 }));
  s.add(line(xs(list.length) + colW / 2, top, xs(0) + colW / 2, top, { stroke: st.c.ink, strokeOpacity: 0.5, strokeWidth: 1 }), line(xs(list.length) + colW / 2, bottom, xs(0) + colW / 2, bottom, { stroke: st.c.ink, strokeOpacity: 0.5, strokeWidth: 1 }));
  list.forEach((a, i) => {
    const k = ease(t, i, list.length);
    const x = xs(i);
    st.mark(s, a.key, x, top + 40, 36);
    const name = a.key === 'unattributed' ? T(m, '未确认', 'Unattributed') : a.name;
    const nh = s.vtext(name, x - 16, top + 80, { font: st.f.display, size: 30, fill: st.c.ink, gap: 0.05 });
    // 占比：汉字竖写（五十六点二）
    const p = Math.round(a.pct * 10) / 10;
    const ptxt = zh(m) ? `${hanzi(Math.floor(p))}${Math.round((p % 1) * 10) ? '點' + hanzi(Math.round((p % 1) * 10)) : ''}` : pct(p);
    s.vtext(ptxt, x + 24, top + 84, { font: st.f.body, size: 17, fill: st.c.soft, gap: 0.02 });
    // 竖条：从底往上
    const bh = Math.max(4, (bottom - top - 120 - nh) * (a.pct / 100) * 1.0) * k;
    const bx = x - 7;
    if (a.key === 'unattributed') s.add(rect(bx, bottom - 20 - bh, 14, bh, { fill: st.hatch(s) }));
    else st.bar(s, bx, bottom - 20 - bh, 14, bh, colorOf(st, a), i);
    s.hit(x - colW / 2, top, colW, bottom - top, tip(m, a));
  });
  // 落款：人的部分
  const hl = handLine(m);
  const foot = c.y + c.h - 82;
  if (hl) { s.text(hl, c.x + c.w - 60, foot, { font: st.f.body, size: 17, fill: st.c.ink, anchor: 'end', maxW: c.w - 140 }); s.hit(c.x + 40, foot - 26, c.w - 80, 40, handTip(m)); }
  s.text(summary(m) + (m.sign ? T(m, ` · ${m.sign} 题`, ` · ${m.sign}`) : ''), c.x + c.w - 60, foot + 34, { font: st.f.body, size: 14, fill: st.c.soft, anchor: 'end' });
  return s;
}

/** HUD：每位贡献方一块倒角面板，大号百分比 */
function hud(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const list = m.rows;
  const W = 960, M = 30, P = 40, gap = 16;
  const cols = 2, cw = (W - M * 2 - P * 2 - gap) / cols, ch = 150;
  const nr = Math.ceil(list.length / cols);
  const hl = handLine(m);
  const { s, c } = canvas(m, st, W, P + 150 + nr * (ch + gap) + (hl ? 90 : 0) + 60, M);
  const x0 = M + P, x1 = W - M - P;
  let y = c.y + P;
  st.label(s, T(m, '// 署名', '// CREDITS'), x0, y + 6, { size: 13, fill: st.c.accent2 });
  st.title(s, m.project || 'Untitled', x0, y + 82, 72, 'start', { maxW: x1 - x0 - 160 });
  st.label(s, range(m), x1, y + 6, { size: 12, anchor: 'end' });
  y += 130;
  list.forEach((a, i) => {
    const k = ease(t, i, list.length);
    const cx = x0 + (i % cols) * (cw + gap), cy = y + Math.floor(i / cols) * (ch + gap);
    const color = a.key === 'unattributed' ? st.c.faint : colorOf(st, a);
    s.add(path(chamfer(cx, cy, cw, ch, 16), { fill: st.c.paper, fillOpacity: 0.9, stroke: color, strokeOpacity: 0.7, strokeWidth: 1.2 }));
    s.add(rect(cx + 16, cy - 1.5, 38, 3, { fill: color }));
    s.text(String(i + 1).padStart(2, '0'), cx + cw - 18, cy + 30, { font: st.f.mono, size: 13, fill: st.c.faint, anchor: 'end' });
    st.mark(s, a.key, cx + 38, cy + 42, 36, color);
    s.text(a.name.toUpperCase(), cx + 70, cy + 40, { font: st.f.display, size: 25, fill: st.c.ink, maxW: cw - 120 });
    const ml = modelLine(a, 2);
    if (ml) s.text(ml, cx + 70, cy + 60, { font: st.f.mono, size: 12, fill: st.c.soft, maxW: cw - 100, overflow: 'clip' });
    st.num(s, pct(a.pct * k), cx + 20, cy + 118, 46, 'start', { fill: color });
    const bx = cx + 20, bw = cw - 40;
    st.track(s, bx, cy + 128, bw, 8);
    if (a.key === 'unattributed') s.add(rect(bx, cy + 128, (bw * a.pct) / 100 * k, 8, { fill: st.hatch(s) }));
    else st.bar(s, bx, cy + 128, Math.max(8, (bw * a.pct) / 100) * k, 8, color, i);
    s.hit(cx, cy, cw, ch, tip(m, a));
  });
  y += nr * (ch + gap) + 10;
  if (hl) {
    st.rule(s, x0, y + 10, x1 - x0, 'orn');
    st.label(s, T(m, '// 人', '// PEOPLE'), x0, y + 44, { size: 13, fill: st.c.accent });
    s.text(hl, x0, y + 74, { font: st.f.body, size: 18, fill: st.c.ink, maxW: x1 - x0 });
    s.hit(x0, y + 24, x1 - x0, 60, handTip(m));
    y += 90;
  }
  s.text(summary(m).toUpperCase(), x0, y + 34, { font: st.f.mono, size: 13, fill: st.c.soft, track: 0.1 });
  return s;
}

/** 领奖台：前三名站台，其余在下面一行 */
function podium(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const agents = m.rows.filter((a) => a.kind !== 'unknown' && a.kind !== 'other');
  const top3 = agents.slice(0, 3);
  const rest = m.rows.filter((a) => !top3.includes(a));
  const W = 1000, M = 34;
  const { s, c } = canvas(m, st, W, 832, M);
  st.label(s, T(m, '署名', 'CREDITS'), W / 2, c.y + 64, { anchor: 'middle' });
  st.title(s, m.project || 'Untitled', W / 2, c.y + 136, 60, 'middle', { maxW: c.w - 120 });
  const order = [1, 0, 2];
  const pw = 210, base = c.y + 600;
  const hs = [300, 220, 160];
  order.forEach((idx, slot) => {
    const a = top3[idx];
    if (!a) return;
    const k = ease(t, slot, 3);
    const cx = W / 2 + (slot - 1) * (pw + 24);
    const h = hs[idx] * k;
    const color = colorOf(st, a);
    // 台座：平顶方块，顶上一道亮边
    s.add(rect(cx - pw / 2, base - h, pw, h, { fill: color, opacity: 0.92 }));
    s.add(rect(cx - pw / 2, base - h, pw, 6, { fill: st.c.ink, opacity: 0.18 }));
    s.add(rect(cx - pw / 2, base - h, pw, h, { fill: 'none', stroke: st.c.ink, strokeOpacity: 0.25, strokeWidth: 1 }));
    s.text(['1', '2', '3'][idx], cx, base - h + 72, { font: st.f.num, size: 64, fill: st.on(color), anchor: 'middle', opacity: 0.9 });
    st.mark(s, a.key, cx, base - h - 120, 54);
    s.text(a.name, cx, base - h - 58, { font: st.f.display, size: 28, fill: st.c.ink, anchor: 'middle', maxW: pw + 10 });
    st.num(s, pct(a.pct), cx, base - h - 22, 22, 'middle', { fill: st.c.soft });
    s.hit(cx - pw / 2, base - h - 150, pw, h + 150, tip(m, a));
  });
  st.rule(s, c.x + 60, base, c.w - 120, 'thin');
  let x = c.x + 60;
  const y = base + 64;
  for (const a of rest) {
    st.mark(s, a.key, x + 12, y - 6, 22);
    const w = s.text(`${a.name} ${pct(a.pct)}`, x + 30, y, { font: st.f.body, size: 17, fill: st.c.soft });
    s.hit(x, y - 22, w + 40, 30, tip(m, a));
    x += w + 64;
    if (x > c.x + c.w - 160) break;
  }
  const hl = handLine(m);
  if (hl) { st.label(s, T(m, '人', 'PEOPLE'), W / 2, y + 78, { anchor: 'middle', fill: st.c.accent2 }); s.text(hl, W / 2, y + 112, { font: st.f.body, size: 18, fill: st.c.ink, anchor: 'middle', maxW: c.w - 100 }); s.hit(c.x + 50, y + 56, c.w - 100, 70, handTip(m)); }
  return s;
}

void circle; void el; void g; void r; void measure; void capOf; void mix; void fmt; void fillOf;
export type { Agent };


/** 居中对称：条从中线向两边长 */
function center(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const W = 880, M = 34, rowH = 132;
  const hl = handLine(m);
  const { s, c } = canvas(m, st, W, 230 + m.rows.length * rowH + (hl ? 120 : 0) + 90, M);
  const cx = W / 2;
  let y = c.y + 64;
  st.label(s, T(m, '署名', 'CREDITS'), cx, y, { anchor: 'middle', size: 13 });
  st.title(s, m.project || 'Untitled', cx, y + 74, 54, 'middle', { maxW: c.w - 100 });
  st.label(s, range(m), cx, y + 108, { anchor: 'middle', size: 12, fill: st.c.faint });
  st.rule(s, c.x + 80, y + 136, c.w - 160, 'orn');
  y += 190;
  const half = (c.w - 140) / 2;
  m.rows.forEach((a, i) => {
    const k = ease(t, i, m.rows.length);
    st.mark(s, a.key, cx, y + 4, 30);
    s.text(a.name, cx, y + 54, { font: st.f.display, size: 22, fill: st.c.ink, anchor: 'middle', track: 0.08, upper: st.labelUpper });
    const w = Math.max(4, (half * a.pct) / 100 * 1.6) * k;
    const by = y + 70;
    st.track(s, cx - half, by, half * 2, 8);
    if (a.key === 'unattributed') s.add(rect(cx - w / 2, by, w, 8, { fill: st.hatch(s) }));
    else st.bar(s, cx - Math.min(w, half * 2) / 2, by, Math.min(w, half * 2), 8, colorOf(st, a), i);
    st.num(s, pct(a.pct), cx, by + 34, 17, 'middle', { fill: st.c.soft });
    s.hit(c.x + 40, y - 18, c.w - 80, rowH - 8, tip(m, a));
    y += rowH;
  });
  if (hl) {
    st.rule(s, c.x + 160, y + 10, c.w - 320, 'thin');
    st.label(s, T(m, '人', 'PEOPLE'), cx, y + 52, { anchor: 'middle', fill: st.c.accent2 });
    s.text(hl, cx, y + 84, { font: st.f.body, size: 17, fill: st.c.ink, anchor: 'middle', maxW: c.w - 100 });
    s.hit(c.x + 60, y + 30, c.w - 120, 70, handTip(m));
    y += 120;
  }
  s.text(summary(m), cx, y + 40, { font: st.f.body, size: 13, fill: st.c.soft, anchor: 'middle', track: 0.06 });
  return s;
}

/** 六项能力：贡献、速度、广度、持久、精确、势头。按 agent 之间的相对值评 A–E */
export function abilities(m: Model, a: Agent) {
  const agents = m.agents.filter((x) => x.kind === 'agent');
  const areaCount = (key: string) => {
    let n = 0;
    for (const b of m.raw.breakdown ?? []) {
      if (m.keyOf(b.contributor) === key) n = Math.max(n, Object.values(b.byArea ?? {}).filter((v: any) => v > 0).length);
    }
    return n;
  };
  const recent = (key: string) => {
    const ws = m.weeks.slice(-Math.max(2, Math.round(m.weeks.length * 0.3)));
    const mine = ws.reduce((s2, w) => s2 + (w.by[key] ?? 0), 0), all = ws.reduce((s2, w) => s2 + Object.values(w.by).reduce((x, y) => x + y, 0), 0);
    return all ? mine / all : 0;
  };
  const raw = (x: Agent) => [x.pct, x.sessions ? x.lines / x.sessions : 0, areaCount(x.key), x.survival, x.verified, recent(x.key)];
  const all = agents.map(raw);
  const mine = raw(a);
  const norm = mine.map((v, i) => { const mx = Math.max(...all.map((r2) => r2[i]), 1e-9); return Math.max(0, Math.min(1, v / mx)); });
  const grade = (v: number) => (v >= 0.8 ? 'A' : v >= 0.6 ? 'B' : v >= 0.4 ? 'C' : v >= 0.2 ? 'D' : 'E');
  const names = zh(m) ? ['贡献', '速度', '广度', '持久', '精确', '势头'] : ['Power', 'Speed', 'Range', 'Durability', 'Precision', 'Momentum'];
  const why = zh(m) ? ['占全部代码的比例', '每个会话写下的行数', '涉及的板块数', '写下的代码至今仍在的比例', '有逐行证据的比例', '最近几周的份额'] : ['share of all code', 'lines per session', 'areas touched', 'share still in the code', 'line-level evidence', 'share in recent weeks'];
  return norm.map((v, i) => ({ name: names[i], v, g: grade(v), why: why[i] }));
}

/** 能力面板：前四位 agent 各一张六边形雷达，A–E 评级；评级依据放在悬停提示里 */
function stats(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const list = m.rows.filter((a) => a.kind === 'agent').slice(0, 4);
  const W = 1080, M = 30;
  const cw = (W - M * 2 - 60 - 24) / 2, ch = 430;
  const { s, c } = canvas(m, st, W, 190 + Math.ceil(list.length / 2) * (ch + 24) + 40, M);
  st.label(s, T(m, '能力', 'STATS'), c.x + 40, c.y + 56, { size: 15, fill: st.c.accent2 });
  st.title(s, m.project || 'Untitled', c.x + 40, c.y + 130, 64, 'start', { maxW: c.w - 80 });
  list.forEach((a, i) => {
    const x = c.x + 30 + (i % 2) * (cw + 24), y = c.y + 170 + Math.floor(i / 2) * (ch + 24);
    s.add(rect(x, y, cw, ch, { fill: st.c.paper, stroke: st.c.ink, strokeWidth: 3 }));
    st.mark(s, a.key, x + 40, y + 42, 40);
    s.text(a.name, x + 74, y + 54, { font: st.f.display, size: 28, fill: st.c.ink, maxW: cw - 180 });
    st.num(s, pct(a.pct), x + cw - 20, y + 56, 30, 'end', { fill: colorOf(st, a) });
    const ab = abilities(m, a);
    const R = 120, rx = x + cw / 2, ry = y + 250;
    const pt = (k: number, v: number) => [rx + Math.sin((k / 6) * Math.PI * 2) * R * v, ry - Math.cos((k / 6) * Math.PI * 2) * R * v];
    for (const lv of [1, 0.8, 0.6, 0.4, 0.2]) s.add(path('M' + [0, 1, 2, 3, 4, 5].map((k) => pt(k, lv).map(r).join(' ')).join('L') + 'Z', { fill: lv === 1 ? st.c.paper : 'none', stroke: st.c.ink, strokeOpacity: lv === 1 ? 1 : 0.22, strokeWidth: lv === 1 ? 2 : 1 }));
    for (let k = 0; k < 6; k++) { const [px, py] = pt(k, 1); s.add(line(rx, ry, px, py, { stroke: st.c.ink, strokeOpacity: 0.22 })); }
    const kk = ease(t, i, list.length);
    s.add(path('M' + ab.map((q, k) => pt(k, Math.max(0.06, q.v) * kk).map(r).join(' ')).join('L') + 'Z', { fill: colorOf(st, a), fillOpacity: 0.55, stroke: st.c.ink, strokeWidth: 2.4 }));
    ab.forEach((q, k) => {
      const [px, py] = pt(k, 1.32);
      const anchor = Math.abs(px - rx) < 4 ? 'middle' : px > rx ? 'start' : 'end';
      const lx = anchor === 'middle' ? px : px + (anchor === 'start' ? -18 : 18);
      s.text(q.g, lx, py - 2, { font: st.f.num, size: 30, fill: colorOf(st, a) === st.c.paper ? st.c.ink : st.c.ink, anchor, stroke: st.c.paper, strokeW: 3 });
      s.text(q.name, lx, py + 18, { font: st.f.label, size: 13, fill: st.c.soft, anchor });
      s.hit(lx - 46, py - 30, 92, 54, { k: a.key, t: `${a.name} · ${q.name} ${q.g}`, d: [q.why] });
    });
    s.hit(x, y, cw, 80, tip(m, a));
  });
  return s;
}

// ───────────────────────── 工具里分模型 ─────────────────────────

interface Seg { key: string; name: string; lines: number; pct: number; color: string; unknown?: boolean }
/** 一个工具按模型切成几段：按模型分组时颜色和别的作品里那一行一致，否则在工具色上分深浅；没记模型的部分单独一段 */
export function segsOf(m: Model, st: Style, a: Agent): Seg[] {
  const base = colorOf(st, a);
  const per = a.lines ? a.pct / a.lines : 0;
  // 深色往浅里调、浅色往深里调，黑白风格里也分得开
  const toward = lum(base) < 0.5 ? st.c.paper : st.c.ink;
  const shade = (j: number) => (j === 0 ? base : mix(base, toward, Math.min(0.72, 0.34 * j)));
  const out: Seg[] = a.models.map((q, j) => {
    const row = m.group === 'model' ? m.rows.find((x) => x.key === `${a.key}/${q.id}`) : undefined;
    return { key: `${a.key}/${q.id}`, name: q.name, lines: q.lines, pct: q.lines * per, color: row ? colorOf(st, row) : shade(j) };
  });
  const rest = a.lines - out.reduce((s2, x) => s2 + x.lines, 0);
  if (out.length && rest > 0) out.push({ key: a.key, name: T(m, '未记录模型', 'Model not recorded'), lines: rest, pct: rest * per, color: mix(base, st.c.paper, 0.65), unknown: true });
  return out.length ? out : [{ key: a.key, name: a.name, lines: a.lines, pct: a.pct, color: base }];
}
export const segTip = (m: Model, a: Agent, x: Seg) => ({ k: x.key, t: x.unknown ? `${a.name} · ${x.name}` : x.name, d: [T(m, `${fmt(x.lines)} 行 · ${pct(x.pct)}`, `${fmt(x.lines)} lines · ${pct(x.pct)}`), T(m, `用 ${a.name}`, `via ${a.name}`)] });

/** 分段：一个工具一行，条按模型切成几段，段间留缝；条下面写各段的模型和占比 */
function split(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const list = m.tools;
  const W = 920, M = 34, P = 50;
  const rowH = (a: Agent) => (segsOf(m, st, a).length > 1 ? 128 : 96);
  const { s, c } = canvas(m, st, W, P + 132 + list.reduce((h, a) => h + rowH(a), 0) + 24 + 70, M);
  const x0 = c.x + P, x1 = c.x + c.w - P;
  let y = c.y + P;
  st.label(s, T(m, '署名', 'CREDITS'), x0, y + 10, { size: 13 });
  st.label(s, range(m), x1, y + 10, { size: 12, anchor: 'end', fill: st.c.faint });
  st.title(s, m.project || 'Untitled', x0, y + 78, 56, 'start', { maxW: x1 - x0 });
  y += 132;
  list.forEach((a, i) => {
    const k = ease(t, i, list.length);
    const nx = x0 + 58, bw = x1 - nx, bh = 12, by = y + 68;
    st.mark(s, a.key, x0 + 20, y + 24, 34);
    s.text(a.name, nx, y + 33, { font: st.f.display, size: 27, fill: st.c.ink, maxW: x1 - nx - 140 });
    st.num(s, pct(a.pct * (t >= 1 ? 1 : k)), x1, y + 36, 32, 'end');
    st.track(s, nx, by, bw, bh);
    const segs = segsOf(m, st, a);
    const full = Math.max(a.pct > 0 ? 3 : 0, (bw * a.pct) / 100) * k;
    const gap = segs.length > 1 ? 3 : 0;
    let x = nx;
    segs.forEach((q, j) => {
      const w = Math.max(0, (full * q.lines) / Math.max(1, a.lines) - (j < segs.length - 1 ? gap : 0));
      if (a.key === 'unattributed') s.add(rect(x, by, w, bh, { fill: st.hatch(s), rx: Math.min(st.radius, bh / 2) }));
      else st.bar(s, x, by, w, bh, q.color, i);
      if (segs.length > 1) s.hit(x, by - 6, Math.max(w, 6), bh + 12, segTip(m, a, q));
      x += w + gap;
    });
    if (segs.length > 1) {
      // 条下面：各段的模型名和占比，色块对应
      let lx = nx;
      for (const q of segs) {
        if (lx > x1 - 80) break;
        s.add(rect(lx, by + 30, 10, 10, { fill: q.color, rx: Math.min(2, st.radius) }));
        const w = s.text(`${q.name} ${pct(q.pct)}`, lx + 16, by + 40, { font: st.f.body, size: 14, fill: st.c.soft, maxW: x1 - lx - 16, overflow: 'clip' });
        s.hit(lx - 4, by + 24, w + 24, 22, segTip(m, a, q));
        lx += w + 40;
      }
    }
    s.hit(x0 - 8, y - 6, x1 - x0 + 16, 60, tip(m, a));
    y += rowH(a);
  });
  y += 40;
  s.text(summary(m), x0, y, { font: st.f.body, size: 14, fill: st.c.soft });
  s.text('creditia', x1, y, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end', track: 0.12 });
  return s;
}

/** 分组：工具是小标题，下面每个模型一行、各有自己的条；只有一个模型的工具写成一行 */
function grouped(m: Model, st: Style, o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const list = m.tools;
  const W = 920, M = 34, P = 50, headH = 64, subH = 58;
  const blockH = (a: Agent) => { const n = segsOf(m, st, a).length; return headH + (n > 1 ? n * subH : 26) + 22; };
  const { s, c } = canvas(m, st, W, P + 132 + list.reduce((h, a) => h + blockH(a), 0) + 70, M);
  const x0 = c.x + P, x1 = c.x + c.w - P;
  let y = c.y + P;
  st.label(s, T(m, '署名', 'CREDITS'), x0, y + 10, { size: 13 });
  st.label(s, range(m), x1, y + 10, { size: 12, anchor: 'end', fill: st.c.faint });
  st.title(s, m.project || 'Untitled', x0, y + 78, 56, 'start', { maxW: x1 - x0 });
  y += 132;
  let row = 0;
  const total = list.reduce((n2, a) => n2 + Math.max(1, segsOf(m, st, a).length), 0);
  list.forEach((a, i) => {
    const segs = segsOf(m, st, a);
    st.mark(s, a.key, x0 + 18, y + 22, 30);
    s.text(a.name, x0 + 50, y + 32, { font: st.f.display, size: 25, fill: st.c.ink, maxW: x1 - x0 - 200 });
    st.num(s, pct(a.pct), x1, y + 32, 26, 'end');
    s.hit(x0 - 8, y - 4, x1 - x0 + 16, headH - 8, tip(m, a));
    st.rule(s, x0, y + 50, x1 - x0, 'thin');
    y += headH;
    const nx = x0 + 50, bw = x1 - nx - 110;
    if (segs.length <= 1) {
      // 只有一个模型：一条细条，模型名写在条后
      const k = ease(t, row++, total);
      st.track(s, nx, y - 4, bw, 8);
      if (a.key === 'unattributed') s.add(rect(nx, y - 4, (bw * a.pct) / 100 * k, 8, { fill: st.hatch(s) }));
      else st.bar(s, nx, y - 4, Math.max(3, (bw * a.pct) / 100) * k, 8, segs[0]?.color ?? colorOf(st, a), i);
      const ml = modelLine(a);
      if (ml) s.text(ml, x1, y + 4, { font: st.f.body, size: 14, fill: st.c.soft, anchor: 'end', maxW: 100, overflow: 'clip' });
      y += 26 + 22;
      return;
    }
    for (const q of segs) {
      const k = ease(t, row++, total);
      s.add(rect(nx, y + 2, 10, 10, { fill: q.color, rx: Math.min(2, st.radius) }));
      s.text(q.name, nx + 18, y + 12, { font: st.f.body, size: 18, fill: q.unknown ? st.c.soft : st.c.ink, maxW: bw - 18 });
      st.num(s, pct(q.pct * (t >= 1 ? 1 : k)), x1, y + 14, 20, 'end', { fill: st.c.soft });
      st.track(s, nx, y + 26, bw, 7);
      st.bar(s, nx, y + 26, Math.max(3, (bw * q.pct) / 100) * k, 7, q.color, i);
      s.hit(nx - 6, y - 8, x1 - nx + 12, subH - 6, segTip(m, a, q));
      y += subH;
    }
    y += 22;
  });
  y += 30;
  s.text(summary(m), x0, y, { font: st.f.body, size: 14, fill: st.c.soft });
  s.text('creditia', x1, y, { font: st.f.label, size: 13, fill: st.c.faint, anchor: 'end', track: 0.12 });
  return s;
}
