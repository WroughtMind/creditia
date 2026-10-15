// 片尾字幕：谁担任了什么角色。角色来自数据（哪个工具写了核心、界面、测试、文档），不是按名次编头衔。
// 版式：classic（双栏）、scroll（银幕滚动，HTML/GIF 会动）、card（终卡）、vertical（竖排）、ed（动画片尾）、billing（海报署名块）、terminal
import { Scene, rect, line, path, el, g, r, ease, measure, mix } from '../svg.ts';
import type { Model, Agent } from '../data.ts';
import { fmt, compact, pct, nameOf, hanzi } from '../data.ts';
import type { Style } from '../style.ts';
import { T, zh, tip, handFacts, handTip, peopleNames, canvas, range, measureV, type PieceOpts } from './common.ts';
import { inkSeal } from '../styles/east.ts';

export const ROLL_VARIANTS = ['classic', 'scroll', 'card', 'vertical', 'ed', 'billing', 'terminal'];

interface Credit { name: string; sub?: string; key?: string }
interface Section { head: string; credits: Credit[]; key?: string }

const AREA: Record<string, [string, string]> = {
  core: ['核心', 'Core'], frontend: ['界面', 'Interface'], tests: ['测试', 'Tests'], docs: ['文档', 'Documentation'], build: ['构建与发布', 'Build & Release'], assets: ['素材', 'Assets'],
};

/** 字幕内容：同一份，各版式只是排法不同 */
export function creditsOf(m: Model): Section[] {
  const out: Section[] = [];
  const agents = m.agents.filter((a) => a.kind === 'agent');
  const byKey = new Map(m.agents.map((a) => [a.key, a]));
  // 主笔：占比 ≥ 15% 的，否则第一名
  const leads = agents.filter((a) => a.pct >= 15);
  const lead = leads.length ? leads : agents.slice(0, 1);
  if (lead.length) out.push({ head: T(m, '主要作者', 'Written by'), credits: lead.map((a) => ({ name: a.name, sub: a.models.slice(0, 2).map((x) => x.name).join(' · '), key: a.key })) });
  // 各板块：该板块里占 ≥ 8% 的贡献方
  const order = ['core', 'frontend', 'tests', 'docs', 'build', 'assets'];
  for (const ar of order) {
    const by = m.areas[ar];
    if (!by) continue;
    const tot = Object.values(by).reduce((s, x) => s + x, 0);
    if (!tot) continue;
    const who = Object.entries(by).filter(([k, v]) => k !== 'unattributed' && v / tot >= 0.08).sort((a, b) => b[1] - a[1]);
    if (!who.length) continue;
    out.push({ head: T(m, AREA[ar][0], AREA[ar][1]), credits: who.map(([k]) => (k === 'human' ? { name: peopleNames(m, 2) || T(m, '人', 'People'), key: 'human' } : { name: byKey.get(k)?.name ?? nameOf(k, m.lang), key: k })) });
  }
  // 相邻板块名单相同就合并成一行职务（核心 · 界面 · 测试）
  for (let i = out.length - 1; i > 0; i--) {
    const a = out[i - 1], b = out[i];
    const same = (x: Section) => x.credits.map((c) => c.name).join('|');
    const isArea = (x: Section) => order.some((ar) => x.head.split(' · ').includes(T(m, AREA[ar][0], AREA[ar][1])));
    if (isArea(a) && isArea(b) && same(a) === same(b)) { a.head = `${a.head} · ${b.head}`; out.splice(i, 1); }
  }
  // 板块名单和执笔完全一样：说明执笔者包办了这些板块，并进执笔一行的说明
  if (out.length > 1 && out[1].credits.map((c) => c.name).join('|') === out[0].credits.map((c) => c.name).join('|')) {
    out[0].head = `${out[0].head} · ${out[1].head}`; out.splice(1, 1);
  }
  // 其余参与者
  const minor = agents.filter((a) => !lead.includes(a));
  if (minor.length) out.push({ head: T(m, '参与', 'With'), credits: minor.map((a) => ({ name: a.name, sub: a.models[0]?.name, key: a.key })) });
  // 审阅
  const rev: Credit[] = m.reviewers.map((x) => ({ name: x.name, sub: x.reviews ? T(m, `自动 review ${x.reviews} 次`, `${x.reviews} automated reviews`) : x.model, key: x.key }));
  if (m.showHand && m.hand.reviews) rev.push({ name: peopleNames(m, 2) || T(m, '人', 'People'), sub: T(m, `review ${m.hand.reviews} 次`, `${m.hand.reviews} reviews`), key: 'human' });
  if (rev.length) out.push({ head: 'Review', credits: rev });
  // 人的部分
  const hf = handFacts(m);
  if (hf.length) out.push({ head: T(m, '人', 'People'), credits: hf.map((f) => ({ name: f.text, key: 'human' })), key: 'human' });
  // 模型
  const models = [...new Set(agents.flatMap((a) => a.models.map((x) => x.name)))];
  if (models.length) out.push({ head: T(m, '模型', 'Models'), credits: models.map((x) => ({ name: x })) });
  if (m.unknown && m.unknown.pct >= 0.05) out.push({ head: T(m, '未确认', 'Unattributed'), credits: [{ name: T(m, `${pct(m.unknown.pct)} 的代码确定不了是谁写的`, `${pct(m.unknown.pct)} of the code has no clear author`), key: 'unattributed' }] });
  return out;
}
const closing = (m: Model) => T(m, `${m.sessions ? `${fmt(m.sessions)} 个会话` : ''}${m.tokens ? ` · ${compact(m.tokens)} token` : ''} · ${fmt(m.commits)} 次提交`, `${m.sessions ? `${fmt(m.sessions)} sessions` : ''}${m.tokens ? ` · ${compact(m.tokens)} tokens` : ''} · ${fmt(m.commits)} commits`).replace(/^ · /, '');

const tipFor = (m: Model, c: Credit, head: string) => {
  const a = c.key ? m.agents.find((x) => x.key === c.key) : undefined;
  if (a) return tip(m, a);
  if (c.key === 'human') return handTip(m);
  return { k: c.key, t: c.name, d: [head] };
};

export function roll(m: Model, st: Style, o: PieceOpts): Scene | null {
  const secs = creditsOf(m);
  if (!secs.length) return null;
  const v = o.variant ?? st.prefer.roll ?? 'classic';
  if (v === 'card') return card(m, st, secs, o);
  if (v === 'vertical') return vertical(m, st, secs, o);
  if (v === 'ed') return ed(m, st, secs, o);
  if (v === 'billing') return billing(m, st, secs, o);
  if (v === 'terminal') return terminalRoll(m, st, secs, o);
  const s = classic(m, st, secs, v === 'scroll');
  return s;
}

/** 双栏：职务右对齐、名字左对齐，中线对称 */
function classic(m: Model, st: Style, secs: Section[], scroll: boolean): Scene {
  const W = 1080, gut = 28;
  const lineH = 54, subH = 24, gapH = 46;
  let inner = 340;
  // 合并的职务（主要作者 · 核心 · 界面……）一行一个，右对齐，不再横排成一长条冲出画面
  const headLines = (sc: Section) => sc.head.split(' · ');
  for (const sc of secs) { inner += gapH + Math.max(sc.credits.reduce((a, c) => a + lineH + (c.sub ? subH : 0), 0), headLines(sc).length * 26); }
  inner += 260;
  const view = scroll ? Math.round((W * 9) / 16) : 0;
  const { s, c } = canvas(m, st, W, inner + (scroll ? view * 0.15 : 0), 40);
  const cx = W / 2;
  let y = c.y + 110;
  st.label(s, T(m, '片尾字幕', 'END CREDITS'), cx, y, { anchor: 'middle', size: 14 });
  st.title(s, m.project || 'Untitled', cx, y + 92, 78, 'middle', { maxW: c.w - 120 });
  st.rule(s, cx - 140, y + 140, 280, 'orn');
  y += 240;
  for (const sc of secs) {
    y += gapH;
    const y0 = y;
    headLines(sc).forEach((h, j) => st.label(s, h, cx - gut, y0 + 4 + j * 26, { anchor: 'end', size: 17, fill: st.c.soft, maxW: c.w / 2 - gut - 50 }));
    sc.credits.forEach((cr) => {
      s.text(cr.name, cx + gut, y + 6, { font: st.f.display, size: 30, fill: st.c.ink, maxW: c.w / 2 - 70 });
      let hh = lineH;
      if (cr.sub) { s.text(cr.sub, cx + gut, y + 34, { font: st.f.body, size: 16, fill: st.c.soft, maxW: c.w / 2 - 70, overflow: 'clip' }); hh += subH; }
      s.hit(c.x + 40, y - 34, c.w - 80, hh, tipFor(m, cr, sc.head));
      y += hh;
    });
    y = Math.max(y, y0 + headLines(sc).length * 26);
  }
  y += 120;
  st.rule(s, cx - 60, y - 40, 120, 'thin');
  s.text(closing(m), cx, y + 10, { font: st.f.body, size: 18, fill: st.c.soft, anchor: 'middle' });
  s.text(range(m), cx, y + 44, { font: st.f.label, size: 14, fill: st.c.faint, anchor: 'middle', track: 0.2 });
  if (m.sign) st.title(s, m.sign, cx, y + 120, 34, 'middle');
  if (scroll) (s as any).motion = { kind: 'scroll', viewW: W, viewH: view, secs: Math.max(14, Math.round(s.h / 90)) };
  return s;
}

/** 终卡：大标题 + 主笔大字，其余一行带过 */
function card(m: Model, st: Style, secs: Section[], o: PieceOpts): Scene {
  const W = 1080;
  const lead = secs[0];
  const { s, c } = canvas(m, st, W, 380 + lead.credits.length * 290 + 300, 40);
  const t = o.t ?? 1;
  const cx = W / 2;
  let y = c.y + 170;
  st.label(s, T(m, '主要作者', 'WRITTEN BY'), cx, y, { anchor: 'middle', size: 15 });
  y += 70;
  lead.credits.forEach((cr, i) => {
    const k = ease(t, i, lead.credits.length);
    const a = m.agents.find((x) => x.key === cr.key);
    st.mark(s, cr.key ?? '', cx, y + 36, 64);
    st.title(s, cr.name, cx, y + 150, 64, 'middle', { maxW: c.w - 140 });
    if (cr.sub) s.text(cr.sub, cx, y + 190, { font: st.f.body, size: 20, fill: st.c.soft, anchor: 'middle' });
    if (a) st.num(s, pct(a.pct * k), cx, y + 236, 26, 'middle', { fill: st.c.accent });
    s.hit(c.x + 60, y - 10, c.w - 120, 260, tipFor(m, cr, lead.head));
    y += 290;
  });
  st.rule(s, cx - 200, y, 400, 'orn');
  y += 60;
  const others = secs.find((x) => x.head === T(m, '参与', 'With'));
  if (others) {
    st.label(s, others.head, cx, y, { anchor: 'middle' });
    const txt = others.credits.map((x) => x.name).join('  ·  ');
    s.text(txt, cx, y + 40, { font: st.f.display, size: 24, fill: st.c.ink, anchor: 'middle', maxW: c.w - 120 });
    s.hit(c.x + 60, y - 20, c.w - 120, 70, { t: others.head, d: others.credits.map((x) => x.name + (x.sub ? ` · ${x.sub}` : '')) });
    y += 100;
  }
  const hf = handFacts(m);
  if (hf.length) {
    st.label(s, T(m, '人', 'PEOPLE'), cx, y, { anchor: 'middle', fill: st.c.accent2 });
    s.text(hf.map((f) => f.text).join('  ·  '), cx, y + 40, { font: st.f.body, size: 20, fill: st.c.ink, anchor: 'middle', maxW: c.w - 120 });
    s.hit(c.x + 60, y - 20, c.w - 120, 70, handTip(m));
    y += 100;
  }
  s.text(m.project, cx, c.y + c.h - 90, { font: st.f.display, size: 30, fill: st.c.ink, anchor: 'middle' });
  s.text(`${range(m)}  ·  ${closing(m)}`, cx, c.y + c.h - 52, { font: st.f.label, size: 14, fill: st.c.soft, anchor: 'middle', maxW: c.w - 100 });
  return s;
}

/** 竖排：右起一栏一个职务，名字竖写；适合水墨、古籍、浮世绘 */
function vertical(m: Model, st: Style, secs: Section[], o: PieceOpts): Scene {
  const t = o.t ?? 1;
  const NS = 44, SS = 18, HS = 22;
  // 先排版：每个元素一列，从右往左
  type Col = { kind: 'head' | 'name'; text: string; sub?: string; cr?: Credit; head: string; w: number; h: number };
  const cols: Col[] = [];
  for (const sc of secs.filter((x) => x.head !== T(m, '模型', 'Models'))) {
    const head = zh(m) ? sc.head.replace(/ · /g, '・') : sc.head;
    cols.push({ kind: 'head', text: head, head: sc.head, w: 58, h: measureV(head, st, HS, st.f.label) });
    for (const cr of sc.credits) cols.push({ kind: 'name', text: cr.name, sub: cr.sub, cr, head: sc.head, w: cr.sub ? 104 : 70, h: Math.max(measureV(cr.name, st, NS), cr.sub ? measureV(cr.sub, st, SS, st.f.body) + 12 : 0) });
  }
  const titleW = 200, sealW = 150;
  const W = Math.max(1000, 80 + titleW + cols.reduce((n, c) => n + c.w, 0) + sealW + 60);
  const inner = Math.max(720, Math.max(...cols.map((c) => (c.kind === 'head' ? c.h + 60 : c.h + 200))) + 160);
  const { s, c } = canvas(m, st, W, inner, 40);
  const tx = c.x + c.w - 80;
  s.vtext(m.project || '', tx, c.y + 70, { font: st.f.display, size: 64, fill: st.c.ink });
  s.vtext(zh(m) ? '署名' : 'Credits', tx - 74, c.y + 80, { font: st.f.label, size: 22, fill: st.c.soft });
  let x = tx - titleW + 30;
  cols.forEach((col, i) => {
    const k = ease(t, i, cols.length);
    const cx = x - col.w / 2;
    if (k > 0) {
      if (col.kind === 'head') s.vtext(col.text, cx, c.y + 90, { font: st.f.label, size: HS, fill: st.c.accent2, gap: 0.2 });
      else {
        s.vtext(col.text, cx + (col.sub ? 16 : 0), c.y + 200, { font: st.f.display, size: NS, fill: st.c.ink });
        if (col.sub) s.vtext(col.sub, cx - 30, c.y + 212, { font: st.f.body, size: SS, fill: st.c.soft });
        s.hit(x - col.w, c.y + 180, col.w, col.h + 40, tipFor(m, col.cr!, col.head));
      }
    }
    x -= col.w;
  });
  const foot = c.y + c.h - 40;
  s.text(`${range(m)}  ${closing(m)}`, c.x + 60, foot, { font: st.f.body, size: 16, fill: st.c.soft });
  if (m.sign) { s.vtext(m.sign, c.x + 90, c.y + 90, { font: st.f.display, size: 34, fill: st.c.ink }); inkSeal(s, c.x + 90, c.y + 120 + measureV(m.sign, st, 34), 54, [...m.sign].slice(0, 2).join('') || '印', { color: st.c.accent }); }
  else inkSeal(s, c.x + 90, c.y + 110, 64, zh(m) ? '署名' : 'CR', { color: st.c.accent });
  return s;
}

/** 动画片尾：横版，左边留给风景，右侧一栏字幕（职务小字在上、名字在下） */
function ed(m: Model, st: Style, secs: Section[], o: PieceOpts): Scene {
  const W = 1920, H = 1080;
  const s = new Scene(W, H);
  st.ground(s, { x: 0, y: 0, w: W, h: H }, m);
  const t = o.t ?? 1;
  const x0 = W * 0.66, w = W * 0.28;
  // 右侧一层渐隐的纸色，保证字可读
  s.add(rect(x0 - 160, 0, W - x0 + 160, H, { fill: linearVeil(s, st) }));
  const flat = secs.filter((x) => x.head !== T(m, '模型', 'Models'));
  const rows = flat.reduce((n, sc) => n + 1 + sc.credits.length, 0);
  const k0 = Math.min(1, (H - 330) / (rows * 46 + flat.length * 16));
  let y = 170;
  st.title(s, m.project || '', x0, y, 60, 'start', { maxW: w });
  y += 64;
  flat.forEach((sc, si) => {
    const k = ease(t, si, flat.length);
    if (k <= 0) return;
    s.text(sc.head, x0, y + 26 * k0, { font: st.f.label, size: 20 * k0, fill: st.c.soft, opacity: k });
    y += 38 * k0;
    for (const cr of sc.credits) {
      s.text(cr.name, x0 + 28, y + 30 * k0, { font: st.f.display, size: 32 * k0, fill: st.c.ink, maxW: w - 28, opacity: k });
      s.hit(x0 - 10, y, w + 20, 44 * k0, tipFor(m, cr, sc.head));
      y += 46 * k0;
    }
    y += 16 * k0;
  });
  s.text(closing(m), x0, H - 70, { font: st.f.body, size: 18, fill: st.c.soft });
  return s;
}
function linearVeil(s: Scene, st: Style) {
  return s.url('ed-veil', (id) => el('linearGradient', { id, x1: 0, y1: 0, x2: 1, y2: 0 }, el('stop', { offset: 0, stopColor: st.c.paper, stopOpacity: 0 }), el('stop', { offset: 0.35, stopColor: st.c.paper, stopOpacity: 0.78 }), el('stop', { offset: 1, stopColor: st.c.paper, stopOpacity: 0.9 })));
}

/** 海报署名块：大标题，底部一块窄长字的署名 */
function billing(m: Model, st: Style, secs: Section[], _o: PieceOpts): Scene {
  const W = 1080;
  const { s, c } = canvas(m, st, W, 1440, 40);
  const cx = W / 2;
  const lead = secs[0];
  st.label(s, lead.credits.map((x) => x.name).join('  ·  ').toUpperCase(), cx, c.y + 120, { anchor: 'middle', size: 22, fill: st.c.ink });
  s.hit(c.x + 40, c.y + 90, c.w - 80, 44, { t: lead.head, d: lead.credits.map((x) => x.name) });
  st.title(s, m.project || 'Untitled', cx, c.y + 560, 220, 'middle', { maxW: c.w - 80 });
  // 署名块
  const cond = { family: 'Oswald, Anton, Noto Sans SC', weight: 400 };
  const lines: [string, string][] = secs.filter((x) => x.head !== T(m, '模型', 'Models')).map((sc) => [sc.head.toUpperCase(), sc.credits.map((x) => x.name.toUpperCase()).join('  ·  ')]);
  const bl = c.y + c.h - 60 - Math.min(6, lines.length) * 62 - 30;
  st.rule(s, c.x + 80, bl - 40, c.w - 160, 'thin');
  let y = bl;
  for (const [h, names] of lines.slice(0, 6)) {
    s.text(h, cx, y, { font: cond, size: 13, fill: st.c.soft, track: 0.24, anchor: 'middle', maxW: c.w - 140 });
    s.text(names, cx, y + 30, { font: cond, size: 26, fill: st.c.ink, track: 0.04, anchor: 'middle', maxW: c.w - 140 });
    s.hit(c.x + 60, y - 18, c.w - 120, 56, { t: h, d: [names] });
    y += 62;
  }
  s.text(`${closing(m).toUpperCase()}  ·  ${range(m)}`, cx, c.y + c.h - 50, { font: cond, size: 14, fill: st.c.soft, anchor: 'middle', track: 0.2 });
  return s;
}

/** 终端：像一段命令输出 */
function terminalRoll(m: Model, st: Style, secs: Section[], o: PieceOpts): Scene {
  const W = 1080;
  let rows = 6;
  for (const sc of secs) rows += 2 + sc.credits.length;
  const lh = 34;
  const { s, c } = canvas(m, st, W, rows * lh + 160, 36);
  const t = o.t ?? 1;
  const mono = st.f.mono;
  let y = c.y + 70;
  const x = c.x + 44;
  st.title(s, `creditia roll ${m.project}`, x, y, 34);
  y += 60;
  const total = secs.reduce((n, sc) => n + sc.credits.length + 1, 0);
  let i = 0;
  for (const sc of secs) {
    if (ease(t, i++, total) <= 0) break;
    s.text(`# ${sc.head}`, x, y, { font: mono, size: 18, fill: st.c.accent2 });
    y += lh;
    for (const cr of sc.credits) {
      if (ease(t, i++, total) <= 0) break;
      const a = m.agents.find((q) => q.key === cr.key);
      const col = a ? st.color(a.key, a.rank) : st.c.ink;
      s.text('  ' + cr.name.padEnd(22, ' '), x, y, { font: mono, size: 18, fill: col });
      if (cr.sub) s.text(cr.sub, x + measure('  ' + ''.padEnd(24, ' '), mono, 18), y, { font: mono, size: 16, fill: st.c.soft, maxW: c.w - 400, overflow: 'clip' });
      s.hit(x - 10, y - 24, c.w - 80, lh, tipFor(m, cr, sc.head));
      y += lh;
    }
    y += lh * 0.6;
  }
  s.text(`-- ${closing(m)} --`, x, y + 20, { font: mono, size: 16, fill: st.c.faint });
  return s;
}

void line; void path; void el; void g; void r; void mix; void hanzi; void zh;
export type { Agent };
