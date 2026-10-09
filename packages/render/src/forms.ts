// 全部展示形式。规则：一条信息只出现一次；不写凑数的话；没有真数据、真素材的元素不出现；
// 主导者只在 opt.director 打开时出现。
import type { Theme } from './themes.ts';
import type { Model, Contributor } from './data.ts';
import { fmt, pct, compact, nameOf } from './data.ts';
import { BRANDS, colorOf } from './brand.ts';
import { h, box, col, txt, label, mark, bar, fill, backdrop, surface, svgImage, stripes, type Node, type Out } from './ui.ts';

const keysOf = (m: Model) => m.rows.map((r) => r.key);
const rankOf = (m: Model, key: string) => Math.max(0, m.rows.findIndex((r) => r.key === key));
/** 真正的 AI 工具（CI 机器人不算）*/
const isAi = (k: string) => !!BRANDS[k]?.logo && k !== 'github-actions';
const zh = (m: Model) => m.raw?.__lang !== 'en';

// ═════════════════════════ 一眼看懂 ═════════════════════════

/** Tier：每行 logo · 名字 · 百分比，下面一根细条。嵌入网页的主形式 */
export function tier(m: Model, t: Theme, s = 2, withTitle = true): Out {
  const W = 400 * s;
  const rowH = 52;
  // 分享图里项目名已经在左边写过，卡片就不再写（withTitle=false）
  const head = (t.center ? (withTitle ? 92 : 48) : (withTitle ? 64 : 44)) + (m.director ? 22 : 0);
  const H = (28 + head + m.rows.length * rowH + 46 + 28) * s;
  const row = (r: Contributor) => {
    const unk = r.key === 'unattributed';
    return col({ height: rowH * s, justifyContent: 'center', gap: 9 * s },
      box({ alignItems: 'center', gap: 10 * s },
        mark(r.key, t, 16 * s),
        txt({ fontSize: 14 * s, fontWeight: 500, color: unk ? t.soft : t.ink, fontFamily: t.sans }, r.name),
        txt({ marginLeft: 'auto', fontSize: 13 * s, color: unk ? t.faint : t.soft, fontFamily: t.num, fontWeight: t.num === t.sans ? 500 : 400 }, pct(r.pct))),
      bar(t, r, rankOf(m, r.key), t.barH * s, 3 * s));
  };
  const title = t.center
    ? col({ alignItems: 'center', height: head * s, justifyContent: 'center', gap: 8 * s },
        label(t, 'Credits', 11 * s),
        withTitle ? txt({ fontFamily: t.display, fontSize: 30 * s, fontWeight: t.displayWeight, color: t.ink, lineHeight: 1 }, m.project) : null,
        m.director ? txt({ fontSize: 12 * s, color: t.soft, marginTop: 4 * s }, m.director) : null)
    : col({ height: head * s, justifyContent: 'flex-start', gap: 6 * s },
        box({ alignItems: 'baseline', justifyContent: withTitle ? 'space-between' : 'flex-end' },
          withTitle ? txt({ fontFamily: t.display, fontSize: 18 * s, fontWeight: t.displayWeight, color: t.ink }, m.project) : null,
          label(t, 'Credits', 10 * s)),
        m.director ? txt({ fontSize: 12 * s, color: t.soft }, m.director) : null);
  const foot = box({ height: 46 * s, alignItems: 'flex-end', justifyContent: t.center ? 'center' : 'flex-start', borderTop: `1px solid ${t.line}`, paddingBottom: 2 * s },
    txt({ fontSize: 12 * s, color: t.soft, fontFamily: t.sans }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')));
  const tree = col({ width: W, height: H, padding: `${28 * s}px ${28 * s}px ${28 * s}px`, ...surface(t, { borderRadius: t.radius * s }), ...(t.glow ? backdrop(t, keysOf(m), { backgroundColor: undefined }) : {}), color: t.ink, fontFamily: t.sans },
    title, col({}, ...m.rows.map(row)), foot);
  return { tree, w: W, h: H };
}

/** 分享图 1200×630：左边一行大字的项目名和一个数字，右边是 tier */
export function share(m: Model, t: Theme): Out {
  const W = 1200, H = 630;
  if (t.center) {
    // 片尾字幕风：只放卡片，居中，项目名在卡片里
    const card = tier(m, t, 1.25, true);
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) }, card.tree) };
  }
  // 左边只写项目名和时间跨度，其余信息都在右边卡片里，不重复
  const card = tier(m, t, 1.25, false);
  const ym = (d: string) => d.slice(0, 7).replace('-', '.');
  const range = m.range.from && m.range.to ? `${ym(m.range.from)} — ${ym(m.range.to)}` : '';
  const left = col({ flex: 1, justifyContent: 'center', gap: 22 },
    txt({ fontFamily: t.display, fontSize: 84, fontWeight: t.displayWeight, color: t.ink, lineHeight: 1, letterSpacing: t.display === t.sans ? -2 : -1 }, m.project),
    range ? txt({ fontFamily: 'JetBrains Mono', fontSize: 19, color: t.soft, letterSpacing: 1 }, range) : null);
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', padding: '0 88px', gap: 72, ...backdrop(t, keysOf(m)) }, left, card.tree) };
}

/** 署名行：一句话 */
export function byline(m: Model, t: Theme, lang: 'zh' | 'en'): Out {
  const tools = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => r.name);
  const list = (a: string[], and: string, sep: string) => (a.length < 2 ? a.join('') : `${a.slice(0, -1).join(sep)}${and}${a[a.length - 1]}`);
  const who = m.director;
  const s = lang === 'zh'
    ? (who ? `${who} 与 ${list(tools, ' 和 ', '、')} 共同完成` : `与 ${list(tools, ' 和 ', '、')} 共同完成`)
    : (who ? `Made by ${who} with ${list(tools, ' & ', ', ')}` : `Made with ${list(tools, ' & ', ', ')}`);
  const W = 1200, H = 120;
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 22, ...backdrop(t, keysOf(m)), fontFamily: t.sans },
    box({ gap: 10, alignItems: 'center' }, ...m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => mark(r.key, t, 20, t.soft))),
    txt({ fontSize: 20, color: t.soft, fontFamily: t.center ? t.display : t.sans, letterSpacing: t.center ? 0.3 : 0 }, s)) };
}

/** 徽章：原生 SVG（README），不经 Satori。左侧 logo，右侧一行占比 */
export function badge(m: Model, t: Theme): string {
  const parts = m.rows.filter((r) => r.kind === 'agent' && r.pct >= 1).map((r) => `${r.name} ${Math.round(r.pct)}%`);
  const value = parts.join('  ·  ');
  const w = (s: string) => [...s].reduce((a, ch) => a + (/[一-鿿]/.test(ch) ? 11 : /[il.·1 ]/.test(ch) ? 3.6 : /[A-Z%]/.test(ch) ? 7.2 : 6.3), 0);
  const L = 26, W = Math.ceil(L + w(value) + 14), H = 20;
  const bg = t.dark ? t.surface : t.mono ? '#FFFFFF' : '#FFFFFF';
  const border = t.dark ? 'rgba(255,255,255,0.12)' : 'rgba(0,0,0,0.12)';
  const icon = (m.rows[0] && require_logo(m.rows[0].key)) || '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="built with ${value}">
<rect x=".5" y=".5" width="${W - 1}" height="${H - 1}" rx="${t.radius ? 5 : 0}" fill="${bg}" stroke="${border}"/>
${icon ? `<svg x="8" y="4" width="12" height="12" viewBox="0 0 24 24" fill="${t.ink}">${icon}</svg>` : ''}
<text x="${L}" y="14" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="11" fill="${t.ink}">${value}</text></svg>`;
}
import { logoSvg } from './brand.ts';
function require_logo(key: string) {
  const s = logoSvg(key);
  return s ? s.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '').replace(/<title>.*?<\/title>/, '') : '';
}

// ═════════════════════════ 有情绪、适合传播 ═════════════════════════

/** 片尾字幕：只写角色和名字 */
export function roll(m: Model, t: Theme): Out {
  const W = 1080, H = 1350;
  const authors = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key));
  const humans = m.rows.filter((r) => r.kind === 'human');
  const leadR = authors[0], rest = authors.slice(1);
  const role = (name: string, ...people: Node[]) => col({ alignItems: 'center', gap: 18 }, label(t, name, 15), ...people);
  const person = (key: string | null, name: string, model?: string) => box({ alignItems: 'center', gap: 16 },
    key ? mark(key, t, 32) : null,
    txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 40 : 46, fontWeight: t.displayWeight, color: t.ink, letterSpacing: t.display === t.sans ? -0.5 : 0 }, name),
    model ? txt({ fontFamily: 'JetBrains Mono', fontSize: 18, color: t.faint, marginLeft: 6, marginTop: 6 }, model) : null);
  const blocks = [
    m.director ? role('主导', person(null, m.director)) : null,
    leadR ? role('主笔', person(leadR.key, leadR.name, leadR.models[0])) : null,
    rest.length ? role('协作', ...rest.map((r) => person(r.key, r.name, r.models[0]))) : null,
    humans.length ? role('贡献', ...humans.map((r) => person(null, r.name))) : null,
    m.reviewers.length ? role('审阅', ...m.reviewers.map((r) => person(r.key, r.name, r.model))) : null,
  ].filter(Boolean);
  return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 72, ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 72 : 96, fontWeight: t.displayWeight, letterSpacing: t.display === t.sans ? -2 : 0, marginBottom: 8 }, m.project),
    ...blocks) };
}

/** 精力小票：只回答"花了多少精力" */
export function receipt(m: Model, t: Theme): Out {
  const W = 1080, H = 1350;
  const items: [string, string, string?][] = [];
  if (m.spanDays) items.push(['工期', `${m.spanDays} 天`, `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}`]);
  if (m.activeDays) items.push(['动工', `${m.activeDays} 天`]);
  if (m.sessions) items.push(['会话', `${fmt(m.sessions)}`]);
  if (m.tokens) items.push(['Token', compact(m.tokens)]);
  if (m.commits) items.push(['提交', fmt(m.commits)]);
  if (m.linesAdded) items.push(['写下', `${fmt(m.linesAdded)} 行`]);
  if (m.linesSurviving) items.push(['留下', `${fmt(m.linesSurviving)} 行`]);
  if (m.director) items.push(['主导', m.director]);
  const rule = box({ height: 0, borderTop: `1.5px dashed ${t.line}`, margin: '8px 0' });
  const paper = col({ width: 620, padding: '64px 56px 56px', ...surface(t, { borderRadius: Math.min(t.radius, 10) }), gap: 0 },
    col({ alignItems: 'center', gap: 14, marginBottom: 40 },
      label(t, 'Receipt', 13),
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 44 : 54, fontWeight: t.displayWeight, color: t.ink }, m.project)),
    rule,
    col({ padding: '20px 0' },
      ...items.map(([k, v, sub]) => col({ padding: '13px 0' },
        box({ justifyContent: 'space-between', alignItems: 'baseline' },
          txt({ fontSize: 20, color: t.soft }, k),
          txt({ fontSize: 26, fontFamily: t.num === t.sans ? 'JetBrains Mono' : t.num, color: t.ink }, v)),
        sub ? txt({ alignSelf: 'flex-end', fontSize: 15, color: t.faint, marginTop: 6, fontFamily: 'JetBrains Mono' }, sub) : null))),
    rule,
    box({ justifyContent: 'center', marginTop: 28, gap: 14 }, ...m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => mark(r.key, t, 22, t.soft))));
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans }, paper) };
}

/** 统计图通用框：左上项目名 + 一句说明，底部图例 */
function frame(m: Model, t: Theme, W: number, H: number, caption: string, body: Node, legend = true): Out {
  const leg = legend
    ? box({ gap: 26, alignItems: 'center', flexWrap: 'wrap' },
        ...m.rows.map((r, i) => box({ alignItems: 'center', gap: 9 },
          box({ width: 10, height: 10, borderRadius: t.barRadius ? 3 : 0, ...fill(r, t, i, 2) }),
          txt({ fontSize: 16, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name))))
    : null;
  return { w: W, h: H, tree: col({ width: W, height: H, padding: '64px 80px 56px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    box({ alignItems: 'baseline', justifyContent: 'space-between', marginBottom: 44 },
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight }, m.project),
      label(t, caption, 13)),
    col({ flex: 1 }, body),
    leg ? box({ marginTop: 36 }, leg) : null) };
}

/** 时间织带：每天一格 */
export function ribbon(m: Model, t: Theme): Out | null {
  const days: { date: string; by: Record<string, number> }[] = m.raw.days ?? [];
  if (!days.length) return null;
  const start = Date.parse(days[0].date), end = Date.parse(days[days.length - 1].date);
  const n = Math.round((end - start) / 86400000) + 1;
  const map = new Map(days.map((d) => [d.date, d.by]));
  const W = 1200, inner = W - 160, gap = n > 120 ? 2 : 4;
  const cw = (inner - gap * (n - 1)) / n;
  const cells: Node[] = [], marks: Node[] = [];
  for (let i = 0; i < n; i++) {
    const d = new Date(start + i * 86400000);
    const by = map.get(d.toISOString().slice(0, 10));
    let key = '';
    if (by) key = Object.entries(by).filter(([k]) => !k.startsWith('h:')).sort((a, b) => b[1] - a[1])[0]?.[0] ?? 'unattributed';
    cells.push(box({ width: cw, height: 180, borderRadius: t.barRadius ? Math.min(3, cw / 3) : 0, ...(by ? fill({ key }, t, rankOf(m, key), 2) : { backgroundColor: t.track }) }));
    if (d.getUTCDate() === 1 || i === 0) marks.push(txt({ position: 'absolute', left: i * (cw + gap), fontSize: 14, color: t.soft, letterSpacing: 1 }, `${d.getUTCMonth() + 1}月`));
  }
  return frame(m, t, W, 500, `${n} 天`, col({ gap: 14 }, box({ gap }, ...cells), box({ position: 'relative', height: 20 }, ...marks)));
}

/** 代码马赛克：现在仓库里每个文件一块 */
export function mosaic(m: Model, t: Theme): Out | null {
  const files: { lines: number; by: Record<string, number> }[] = (m.raw.files ?? []).filter((f: any) => f.lines > 0);
  if (!files.length) return null;
  const W = 1200, H = 860, bw = W - 160, bh = H - 64 - 56 - 80 - 70;
  const items = files.map((f) => ({ v: f.lines, key: Object.entries(f.by).filter(([k]) => !k.startsWith('h:')).sort((a, b) => b[1] - a[1])[0]?.[0] ?? 'unattributed' })).sort((a, b) => b.v - a.v);
  const rects = squarify(items, bw, bh);
  const g = t.barRadius === 0 ? 1 : 2;
  const tiles = rects.map((q) => box({ position: 'absolute', left: q.x, top: q.y, width: Math.max(q.w - g, 0.5), height: Math.max(q.h - g, 0.5), borderRadius: t.barRadius && q.w > 10 && q.h > 10 ? 3 : 0, ...fill({ key: q.key }, t, rankOf(m, q.key), 3) }));
  return frame(m, t, W, H, `${fmt(files.length)} 个文件 · 现存 ${fmt(m.linesSurviving)} 行`, box({ position: 'relative', width: bw, height: bh }, ...tiles));
}
function squarify(items: { v: number; key: string }[], W: number, H: number) {
  const out: { x: number; y: number; w: number; h: number; key: string }[] = [];
  const total = items.reduce((s, i) => s + i.v, 0);
  let rest = items.map((i) => ({ ...i, a: (i.v / total) * W * H }));
  let r = { x: 0, y: 0, w: W, h: H };
  const worst = (row: number[], side: number) => {
    const s = row.reduce((a, b) => a + b, 0);
    return Math.max((side * side * Math.max(...row)) / (s * s), (s * s) / (side * side * Math.min(...row)));
  };
  while (rest.length) {
    const side = Math.min(r.w, r.h);
    const row = [rest[0]];
    let i = 1;
    while (i < rest.length && worst([...row, rest[i]].map((x) => x.a), side) <= worst(row.map((x) => x.a), side)) row.push(rest[i++]);
    rest = rest.slice(i);
    const s = row.reduce((a, b) => a + b.a, 0);
    if (r.w >= r.h) {
      const cw = s / r.h; let y = r.y;
      for (const it of row) { const hh = it.a / cw; out.push({ x: r.x, y, w: cw, h: hh, key: it.key }); y += hh; }
      r = { x: r.x + cw, y: r.y, w: r.w - cw, h: r.h };
    } else {
      const ch = s / r.w; let x = r.x;
      for (const it of row) { const ww = it.a / ch; out.push({ x, y: r.y, w: ww, h: ch, key: it.key }); x += ww; }
      r = { x: r.x, y: r.y + ch, w: r.w, h: r.h - ch };
    }
  }
  return out;
}

/** 个人履历卡：跨项目 */
export function profile(person: string, projects: Model[], t: Theme): Out {
  const W = 1200, H = 200 + projects.length * 92 + 40;
  const lines = projects.reduce((s, x) => s + x.linesAdded, 0);
  const tools = new Map<string, number>();
  for (const p of projects) for (const a of p.agents) tools.set(a.key, (tools.get(a.key) ?? 0) + a.lines);
  const first = projects.map((x) => x.range.from).filter(Boolean).sort()[0] ?? '';
  const proj = (p: Model) => col({ gap: 12 },
    box({ justifyContent: 'space-between', alignItems: 'baseline' },
      txt({ fontFamily: t.display, fontSize: 24, fontWeight: t.displayWeight }, p.project),
      txt({ fontSize: 15, color: t.soft, fontFamily: 'JetBrains Mono' }, `${fmt(p.linesAdded)} 行`)),
    box({ height: t.barH * 2, borderRadius: t.barRadius, overflow: 'hidden', backgroundColor: t.track, gap: 2 },
      ...p.rows.map((r, i) => box({ width: `${r.pct}%`, height: '100%', ...fill(r, t, i, 3) }))));
  return { w: W, h: H, tree: box({ width: W, height: H, padding: '72px 80px', gap: 96, ...backdrop(t, [...tools.keys()]), color: t.ink, fontFamily: t.sans },
    col({ width: 340, gap: 0 },
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 44 : 54, fontWeight: t.displayWeight, letterSpacing: t.display === t.sans ? -1 : 0 }, person),
      first ? txt({ fontSize: 17, color: t.soft, marginTop: 10 }, `${first.slice(0, 4)} 年 ${Number(first.slice(5, 7))} 月起`) : null,
      col({ marginTop: 'auto', gap: 18 },
        box({ alignItems: 'baseline', gap: 10 }, txt({ fontFamily: t.num, fontSize: 40, color: t.ink }, compact(lines)), txt({ fontSize: 16, color: t.soft }, `行 · ${projects.length} 个作品`)),
        box({ gap: 14 }, ...[...tools.entries()].filter(([k]) => isAi(k)).sort((a, b) => b[1] - a[1]).map(([k]) => mark(k, t, 24, t.soft))))),
    col({ flex: 1, gap: 36, justifyContent: 'center' }, ...projects.map(proj))) };
}

/** 终端截图：fetch 和 blame 共用 */
export interface Seg { text: string; color?: string; dim?: boolean; bold?: boolean }
export function term(cmd: string, lines: Seg[][], t: Theme, m?: Model): Out {
  const W = 1200, lh = 32, H = 64 + 44 + 32 + (lines.length + 1) * lh + 40 + 64;
  const dark = t.dark || t.id === 'glass' ? true : t.dark;
  const bg = dark ? (t.dark ? t.surface : '#15171C') : '#FFFFFF', fg = dark ? (t.dark ? t.ink : '#E7E9EE') : '#161514';
  const soft = dark ? (t.dark ? t.soft : '#8A909C') : '#76726B';
  const tt: Theme = { ...t, dark };
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, m ? keysOf(m) : []) },
    col({ width: W - 160, backgroundColor: bg, borderRadius: Math.min(t.radius, 12), border: `1px solid ${dark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}`, boxShadow: t.shadow === 'none' ? undefined : t.shadow, fontFamily: 'JetBrains Mono, Noto Sans SC' },
      box({ height: 44, alignItems: 'center', padding: '0 18px', gap: 8 },
        ...['#FF5F57', '#FEBC2E', '#28C840'].map((c) => box({ width: 12, height: 12, borderRadius: 99, backgroundColor: dark ? 'rgba(255,255,255,0.14)' : 'rgba(0,0,0,0.12)' }))),
      col({ padding: '16px 36px 40px', fontSize: 19, color: fg },
        box({ height: lh, alignItems: 'center' }, txt({ color: soft, whiteSpace: 'pre' }, '❯ '), txt({}, cmd)),
        ...lines.map((ln) => box({ height: lh, alignItems: 'center' },
          ...ln.map((s) => txt({ whiteSpace: 'pre', color: s.color ? colorOf(s.color, tt) : s.dim ? soft : fg, fontWeight: s.bold ? 700 : 400 }, s.text || ' '))))))) };
}

// ═════════════════════════ 融进工作流 ═════════════════════════

/** App 内致谢页（iOS 设置页样式） */
export function appPage(m: Model, t: Theme): Out {
  const s = 2, W = 390 * s;
  const rows = m.rows;
  const H = (54 + 44 + 20 + (m.director ? 92 : 0) + 30 + rows.length * 50 + 40 + 40) * s;
  const ios = t.dark ? { bg: '#000000', cell: '#1C1C1E', sep: 'rgba(84,84,88,0.6)', blue: '#0A84FF' } : { bg: '#F2F2F7', cell: '#FFFFFF', sep: 'rgba(60,60,67,0.18)', blue: '#007AFF' };
  const head = (s2: string) => txt({ fontSize: 13 * s, color: t.soft, margin: `${22 * s}px ${16 * s}px ${7 * s}px`, letterSpacing: 0.4 }, s2);
  return { w: W, h: H, tree: col({ width: W, height: H, backgroundColor: ios.bg, color: t.ink, fontFamily: 'Inter, Noto Sans SC', padding: `${54 * s}px ${16 * s}px 0` },
    box({ height: 44 * s, alignItems: 'center', position: 'relative' },
      txt({ fontSize: 17 * s, color: ios.blue }, '‹ 关于'),
      txt({ position: 'absolute', left: 0, right: 0, justifyContent: 'center', fontSize: 17 * s, fontWeight: 600 }, '致谢')),
    m.director ? head('主导') : null,
    m.director ? box({ backgroundColor: ios.cell, borderRadius: 10 * s, padding: `${13 * s}px ${16 * s}px` }, txt({ fontSize: 17 * s }, m.director)) : null,
    head(m.project),
    col({ backgroundColor: ios.cell, borderRadius: 10 * s, paddingLeft: 16 * s },
      ...rows.map((r, i) => box({ alignItems: 'center', gap: 12 * s, height: 50 * s, paddingRight: 16 * s, borderBottom: i < rows.length - 1 ? `${0.5 * s}px solid ${ios.sep}` : 'none' },
        mark(r.key, { ...t, ink: t.dark ? '#FFFFFF' : '#000000' } as Theme, 22 * s),
        txt({ fontSize: 17 * s, color: r.key === 'unattributed' ? t.soft : t.dark ? '#FFFFFF' : '#000000' }, r.name),
        txt({ marginLeft: 'auto', fontSize: 17 * s, color: t.soft }, pct(r.pct))))),
    txt({ fontSize: 13 * s, color: t.soft, margin: `${8 * s}px ${16 * s}px` }, `${fmt(m.commits)} 次提交 · ${fmt(m.linesAdded)} 行`)) };
}

// ═════════════════════════ 统计图 ═════════════════════════

/** 华夫饼：100 格，一格 1% */
export function waffle(m: Model, t: Theme): Out {
  const W = 1200, H = 680;
  const cells = allot(m.rows.map((r) => r.pct), 100);
  const seq: (Contributor | null)[] = m.rows.flatMap((r, i) => Array(cells[i]).fill(r));
  const S = 38, G = 6;
  const grid = col({ gap: G },
    ...Array.from({ length: 10 }, (_, y) => box({ gap: G },
      ...Array.from({ length: 10 }, (_, x) => {
        const r = seq[y * 10 + x];
        return box({ width: S, height: S, borderRadius: t.barRadius ? 7 : 0, ...(r ? fill(r, t, rankOf(m, r.key), 4) : { backgroundColor: t.track }) });
      }))));
  const top = m.rows[0];
  const side = col({ flex: 1, justifyContent: 'center', gap: 28 },
    col({ gap: 6 },
      txt({ fontFamily: t.num, fontSize: 88, color: t.ink, lineHeight: 1, letterSpacing: -2 }, `${Math.round(top.pct)}`),
      txt({ fontSize: 18, color: t.soft }, `格是 ${top.name}`)),
    col({ gap: 14, marginTop: 16 },
      ...m.rows.slice(1).map((r, i) => box({ alignItems: 'center', gap: 12 },
        box({ width: 12, height: 12, borderRadius: t.barRadius ? 3 : 0, ...fill(r, t, i + 1, 2) }),
        txt({ fontSize: 18, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
        txt({ marginLeft: 'auto', fontSize: 17, color: t.soft, fontFamily: t.num === t.sans ? t.sans : t.num }, `${cells[i + 1]} 格`)))));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: '64px 80px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    box({ alignItems: 'baseline', justifyContent: 'space-between', marginBottom: 40 },
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight }, m.project),
      label(t, '一格 = 1%', 13)),
    box({ flex: 1, gap: 96, alignItems: 'center' }, grid, side)) };
}
/** 最大余数法；有占比的至少 1 格 */
function allot(p: number[], N: number) {
  const c = p.map((x) => Math.floor((x / 100) * N));
  const order = p.map((x, i) => [(x / 100) * N - c[i], i] as const).sort((a, b) => b[0] - a[0]);
  for (let k = 0; c.reduce((a, b) => a + b, 0) < N && k < order.length; k++) c[order[k][1]]++;
  for (let i = 0; i < c.length; i++) if (c[i] === 0 && p[i] > 0) { c[i] = 1; c[c.indexOf(Math.max(...c))]--; }
  return c;
}

/** 环形图 */
export function donut(m: Model, t: Theme): Out {
  const W = 1200, H = 630, R = 210, w = t.barH >= 8 ? 56 : 34;
  const r0 = R - w / 2, C = 2 * Math.PI * r0;
  let acc = 0;
  const pat = `<defs><pattern id="u" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="8" height="8" fill="${t.stripeB}"/><rect width="4" height="8" fill="${t.stripeA}"/></pattern></defs>`;
  const gap = t.barRadius ? 4 : 2;
  const arcs = m.rows.map((r, i) => {
    const len = (r.pct / 100) * C;
    const col = r.key === 'unattributed' ? 'url(#u)' : colorOf(r.key, t, i);
    const s = `<circle cx="${R}" cy="${R}" r="${r0}" fill="none" stroke="${col}" stroke-width="${w}" stroke-dasharray="${Math.max(len - gap, 0.8)} ${C}" stroke-dashoffset="${-acc}" transform="rotate(-90 ${R} ${R})"/>`;
    acc += len;
    return s;
  }).join('');
  const ring = box({ position: 'relative', width: 2 * R, height: 2 * R, alignItems: 'center', justifyContent: 'center' },
    svgImage(`<svg xmlns="http://www.w3.org/2000/svg" width="${2 * R}" height="${2 * R}">${pat}<circle cx="${R}" cy="${R}" r="${r0}" fill="none" stroke="${t.track}" stroke-width="${w}"/>${arcs}</svg>`, 2 * R, 2 * R, { position: 'absolute', left: 0, top: 0 }),
    col({ alignItems: 'center', gap: 6 },
      txt({ fontFamily: t.num, fontSize: 48, color: t.ink, letterSpacing: -1 }, compact(m.linesAdded)),
      label(t, '行', 13)));
  const legend = col({ flex: 1, gap: 18, justifyContent: 'center' },
    txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight, marginBottom: 20 }, m.project),
    ...m.rows.map((r) => box({ alignItems: 'center', gap: 14, paddingBottom: 14, borderBottom: `1px solid ${t.line}` },
      mark(r.key, t, 20),
      txt({ fontSize: 19, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
      txt({ marginLeft: 'auto', fontSize: 18, color: t.soft, fontFamily: t.num === t.sans ? t.sans : t.num }, pct(r.pct)))));
  return { w: W, h: H, tree: box({ width: W, height: H, padding: '0 96px', gap: 104, alignItems: 'center', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans }, ring, legend) };
}

/** 堆叠面积：每周新增代码 */
export function area(m: Model, t: Theme): Out | null {
  const weeks: { week: string; byContributor: Record<string, number> }[] = m.raw.timeline ?? [];
  if (weeks.length < 2) return null;
  const groupOf = groupFn(m);
  const order = [...m.rows.map((r) => r.key)].reverse();
  const series = weeks.map((w) => {
    const g: Record<string, number> = {};
    for (const [id, v] of Object.entries(w.byContributor)) { const k = groupOf(id); g[k] = (g[k] ?? 0) + v; }
    return g;
  });
  const max = Math.max(...series.map((g) => order.reduce((s, k) => s + (g[k] ?? 0), 0)));
  const CW = 1040, CH = 360;
  const x = (i: number) => (i / (weeks.length - 1)) * CW;
  const y = (v: number) => CH - (v / max) * CH * 0.96;
  const pat = `<defs><pattern id="u" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="6" height="6" fill="${t.stripeB}"/><rect width="3" height="6" fill="${t.stripeA}"/></pattern></defs>`;
  // 用三次贝塞尔平滑
  const smooth = (pts: [number, number][]) => pts.map((p, i) => {
    if (i === 0) return `M${p[0]},${p[1]}`;
    const q = pts[i - 1], cx = (q[0] + p[0]) / 2;
    return `C${cx},${q[1]} ${cx},${p[1]} ${p[0]},${p[1]}`;
  }).join(' ');
  let base = weeks.map(() => 0);
  const paths: string[] = [];
  for (const k of order) {
    const top = base.map((b, i) => b + (series[i][k] ?? 0));
    const up = top.map((v, i) => [x(i), y(v)] as [number, number]);
    const down = base.map((v, i) => [x(i), y(v)] as [number, number]).reverse();
    const d = `${smooth(up)} L${down[0][0]},${down[0][1]} ${smooth(down).slice(1)} Z`;
    paths.push(`<path d="${d}" fill="${k === 'unattributed' ? 'url(#u)' : colorOf(k, t, rankOf(m, k))}"/>`);
    base = top;
  }
  const grid = [0.25, 0.5, 0.75].map((f) => `<line x1="0" x2="${CW}" y1="${CH * f}" y2="${CH * f}" stroke="${t.line}" stroke-width="1"/>`).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${CW}" height="${CH}">${pat}${grid}${paths.join('')}</svg>`;
  const from = m.range.from ? Date.parse(m.range.from) : 0;
  const lbl = (i: number) => txt({ fontSize: 14, color: t.soft, letterSpacing: 1 }, weekLabel(weeks[i].week, i === 0 ? from : 0));
  return frame(m, t, 1200, 700, '每周新增代码', col({ gap: 14 }, svgImage(svg, CW, CH), box({ justifyContent: 'space-between', width: CW }, lbl(0), lbl(Math.floor(weeks.length / 2)), lbl(weeks.length - 1))));
}
function weekLabel(w: string, notBefore = 0) {
  const [y, n] = w.split('-W').map(Number);
  const jan4 = Date.UTC(y, 0, 4), dow = (new Date(jan4).getUTCDay() + 6) % 7;
  const d = new Date(Math.max(jan4 - dow * 86400000 + (n - 1) * 7 * 86400000, notBefore));
  return `${d.getUTCMonth() + 1}月${d.getUTCDate()}日`;
}
const groupFn = (m: Model) => (id: string) => {
  const c = (m.raw.contributors ?? []).find((x: any) => x.id === id);
  if (c?.harness) return m.rows.some((r) => r.key === c.harness) ? c.harness : 'other';
  if (id === 'unattributed') return 'unattributed';
  return m.rows.some((r) => r.key === id) ? id : 'other';
};

/** 泳道：每个工具一条，会话画成横条 */
export function lanes(m: Model, t: Theme): Out | null {
  const ss: { contributor: string; start: number; end: number; review: boolean }[] = (m.raw.sessions ?? []).filter((s: any) => !s.review);
  if (!ss.length) return null;
  const g = groupFn(m);
  const keys = m.rows.filter((r) => r.kind === 'agent' && ss.some((s) => g(s.contributor) === r.key)).map((r) => r.key);
  const t0 = Math.min(...ss.map((s) => s.start)), t1 = Math.max(...ss.map((s) => s.end));
  const f = new Date((t0 + 8 * 3600) * 1000);
  const from = Date.UTC(f.getUTCFullYear(), f.getUTCMonth(), 1) / 1000 - 8 * 3600;
  const LW = 1040 - 170, LH = 64;
  const X = (ts: number) => ((ts - from) / (t1 - from)) * LW;
  const rows = keys.map((k) => box({ alignItems: 'center', height: LH, borderBottom: `1px solid ${t.line}` },
    box({ width: 170, alignItems: 'center', gap: 12 }, mark(k, t, 18), txt({ fontSize: 17 }, nameOf(k))),
    box({ position: 'relative', width: LW, height: 20 },
      ...ss.filter((s) => g(s.contributor) === k).map((s) =>
        box({ position: 'absolute', left: X(s.start), top: 0, width: Math.max(X(s.end) - X(s.start), 2), height: 20, borderRadius: t.barRadius ? 3 : 0, backgroundColor: colorOf(k, t, rankOf(m, k)), opacity: 0.9 })))));
  const ticks: Node[] = [];
  for (let d = from; d <= t1 - 4 * 86400; d += 86400) {
    const cn = new Date((d + 8 * 3600) * 1000);
    if (cn.getUTCDate() === 1) ticks.push(txt({ position: 'absolute', left: X(d), fontSize: 14, color: t.soft, letterSpacing: 1 }, `${cn.getUTCMonth() + 1}月`));
  }
  return frame(m, t, 1200, 64 + 80 + keys.length * LH + 50 + 56, `${fmt(ss.length)} 次会话`,
    col({}, box({ borderTop: `1px solid ${t.line}` }), ...rows, box({ marginLeft: 170, position: 'relative', height: 30, marginTop: 14 }, ...ticks)), false);
}

/** 作息打卡：星期 × 小时 */
export function clock(m: Model, t: Theme): Out | null {
  const c: number[][] = m.raw.clock;
  if (!c) return null;
  const max = Math.max(...c.flat());
  const S = 34, G = 6;
  const wd = ['一', '二', '三', '四', '五', '六', '日'];
  const top = m.rows.find((r) => r.kind === 'agent');
  const base = t.mono || !top ? (t.dark ? '240,238,233' : '22,21,20') : hex(colorOf(top.key, t));
  const grid = col({ gap: G },
    ...c.map((row, i) => box({ alignItems: 'center', gap: G },
      txt({ width: 40, fontSize: 14, color: t.soft }, `周${wd[i]}`),
      ...row.map((v) => box({ width: S, height: S, borderRadius: t.barRadius ? 7 : 0, backgroundColor: v ? `rgba(${base},${(0.12 + 0.88 * Math.pow(v / max, 0.9)).toFixed(3)})` : t.track })))),
    box({ marginLeft: 40 + G, gap: G, marginTop: 6 },
      ...Array.from({ length: 24 }, (_, hr) => txt({ width: S, justifyContent: 'center', fontSize: 13, color: t.soft }, hr % 6 === 0 ? `${hr}时` : ''))));
  return frame(m, t, 1200, 600, `${fmt(m.commits)} 次提交`, grid, false);
}
const hex = (c: string) => {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(c);
  return m ? `${parseInt(m[1], 16)},${parseInt(m[2], 16)},${parseInt(m[3], 16)}` : '120,120,120';
};

/** 版本柱状图 */
export function versions(m: Model, t: Theme): Out | null {
  const tags: { tag: string; date: string }[] = m.raw.versions ?? [];
  if (!tags.length) return null;
  const g = groupFn(m);
  const by: Record<string, Record<string, number>> = {};
  for (const b of m.raw.breakdown ?? []) {
    const k = g(b.contributor);
    if ((m.raw.directors ?? []).some((d: any) => d.id === b.contributor)) continue;
    for (const [v, n] of Object.entries<number>(b.byVersion ?? {})) { by[v] ??= {}; by[v][k] = (by[v][k] ?? 0) + n; }
  }
  const order = [...tags.map((x) => x.tag), 'unreleased'].filter((v) => by[v]);
  const stack = m.rows.map((r) => r.key);
  const totals = order.map((v) => Object.values(by[v]).reduce((a, b) => a + b, 0));
  const max = Math.max(...totals), BH = 340;
  const bars = order.map((v, i) => col({ alignItems: 'center', width: 200 },
    box({ height: BH, alignItems: 'flex-end' },
      col({ width: 96, height: (totals[i] / max) * BH, borderRadius: t.barRadius ? 8 : 0, overflow: 'hidden', flexDirection: 'column-reverse', gap: 2 },
        ...stack.map((k, j) => (by[v][k] ? box({ height: `${(by[v][k] / totals[i]) * 100}%`, minHeight: 2, ...fill({ key: k }, t, j, 3) }) : null)))),
    txt({ marginTop: 22, fontFamily: t.num === t.sans ? 'JetBrains Mono' : t.num, fontSize: 22, color: t.ink }, v === 'unreleased' ? '未发布' : v),
    txt({ marginTop: 6, fontSize: 14, color: t.soft }, v === 'unreleased' ? `${compact(totals[i])} 行` : `${tags.find((x) => x.tag === v)?.date.replaceAll('-', '.')} · ${compact(totals[i])} 行`)));
  return frame(m, t, 1200, 720, '各版本新增代码', box({ justifyContent: 'center', gap: 64, flex: 1, alignItems: 'flex-end' }, ...bars));
}

/** 沉淀：写下的代码至今还剩多少 */
export function settle(m: Model, t: Theme): Out | null {
  if (!m.linesAdded || !m.linesSurviving) return null;
  const keep = (m.linesSurviving / m.linesAdded) * 100;
  const W = 1200, H = 560;
  const step = (k: string, v: string, p: number, solid: boolean) => col({ gap: 16 },
    box({ justifyContent: 'space-between', alignItems: 'baseline' },
      label(t, k, 14),
      txt({ fontFamily: t.num, fontSize: 30, color: t.ink }, v)),
    box({ height: Math.max(t.barH * 3, 14), borderRadius: t.barRadius, backgroundColor: t.track },
      box({ width: `${p}%`, height: '100%', borderRadius: t.barRadius, backgroundColor: solid ? t.ink : t.faint })));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: '72px 96px', justifyContent: 'space-between', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    box({ alignItems: 'baseline', justifyContent: 'space-between' },
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight }, m.project),
      box({ alignItems: 'baseline', gap: 8 }, txt({ fontFamily: t.num, fontSize: 64, letterSpacing: -1 }, `${Math.round(keep)}%`), txt({ fontSize: 17, color: t.soft }, '留到了今天'))),
    col({ gap: 40 },
      step('写下', `${fmt(m.linesAdded)} 行`, 100, false),
      step('至今仍在', `${fmt(m.linesSurviving)} 行`, keep, true))) };
}

export { stripes };
