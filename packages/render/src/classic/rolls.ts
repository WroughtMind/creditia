// 片尾字幕的新版式。原来的十一种在 forms.ts 里，一个字不动；新的都放在这里，作品清单里排在它们后面。
// 每种都按内容多少放大、铺开：一个工具和六个工具都能撑满画面（见 ui.ts 的 fit / spread / eq）。
//   opening    片头：左对齐，职务小字在上、名字大字在下，片名压在底部
//   production 出品：「X 出品 / 联合出品 Y · Z」，大片名，底下列模型和审阅
//   slate      场记板：黑板白字，PROD / DIRECTOR / CAMERA / SCENE / TAKE
//   crew       部门：每个工具一个部门，底下列它用到的模型和各写了多少行
import type { Theme } from './themes.ts';
import type { Model } from './data.ts';
import { fmt } from './data.ts';
import { modelName } from '../data.ts';
import { BRANDS, BOTS } from './brand.ts';
import { box, col, txt, label, mark, backdrop, ornament, fit, spread, eq, MONO_FONT, type Node, type Out } from './ui.ts';

export const ROLL_NEW = ['opening', 'production', 'slate', 'crew'];

const W = 1080, H = 1350;
// 按模型分组时 key 是「工具/模型」，按工具判断
const isAi = (k: string) => !!BRANDS[k.split('/')[0]] && !BOTS.has(k);
const keysOf = (m: Model) => m.rows.map((r) => r.key);
const hexA = (hex: string, a: string) => (/^#[0-9a-fA-F]{6}$/.test(hex) ? hex + a : hex);
const serifOf = (t: Theme) => t.display !== t.sans;
const dot = (d: string) => d.replaceAll('-', '.');

interface Person { key: string; name: string; model: string }
/** 主笔 / 协作 / 审阅，和原版的角色划分一样 */
function cast(m: Model) {
  const authors = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key));
  const p = (r: { key: string; name: string; models: string[] }): Person => ({ key: r.key, name: r.name, model: r.models[0] ? modelName(r.models[0]) : '' });
  return {
    lead: authors[0] ? p(authors[0]) : null,
    rest: authors.slice(1).map(p),
    reviewers: m.reviewers.map((r) => ({ key: r.key, name: r.name, model: r.model ? modelName(r.model) : '' })),
    authors,
  };
}
/** 每个工具用到的模型和各写了多少行（按行数排） */
function modelLines(m: Model): Map<string, [string, number][]> {
  const byId = new Map<string, any>((m.raw.contributors ?? []).map((c: any) => [c.id, c]));
  const acc = new Map<string, Map<string, number>>();
  for (const b of m.raw.breakdown ?? []) {
    const c = byId.get(b.contributor);
    if (!c?.harness) continue;
    const mm = acc.get(c.harness) ?? new Map<string, number>();
    const k = c.model ? modelName(c.model) : '';
    mm.set(k, (mm.get(k) ?? 0) + (Number(b.linesAdded) || 0));
    acc.set(c.harness, mm);
  }
  return new Map([...acc].map(([k, mm]) => [k, [...mm].filter(([, n]) => n > 0).sort((a, b) => b[1] - a[1])]));
}
const range = (m: Model) => (m.range.from && m.range.to ? `${dot(m.range.from)} — ${dot(m.range.to)}` : '');
const totals = (m: Model) => [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `+${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ');

export function rollNew(m: Model, t: Theme, v: string): Out | null {
  const c = cast(m);
  const serif = serifOf(t);

  if (v === 'opening') {
    // 片头：现代电影开场的署名卡。左对齐，职务小字在上、名字大字在下；片名压在底部
    const people = [
      ...(c.lead ? [{ role: '主笔', p: c.lead, lead: true }] : []),
      ...c.rest.map((p, i) => ({ role: i === 0 ? '协作' : '', p, lead: false })),
      ...c.reviewers.map((p, i) => ({ role: i === 0 ? '审阅' : '', p, lead: false })),
    ];
    const entry = (x: { role: string; p: Person; lead: boolean }) => col({ gap: 4 },
      x.role ? label(t, x.role, 11, { color: t.accent, letterSpacing: 4, marginBottom: 2 }) : null,
      box({ alignItems: 'center', gap: 12 },
        mark(x.p.key, t, x.lead ? 30 : 22),
        txt({ fontFamily: t.display, fontSize: x.lead ? 50 : 36, fontWeight: t.displayWeight, color: t.ink, lineHeight: 1.05, letterSpacing: serif ? 0 : -1, whiteSpace: 'nowrap' }, x.p.name)),
      x.p.model ? txt({ fontFamily: MONO_FONT, fontSize: 11, color: t.faint, marginTop: 2 }, x.p.model) : null);
    const list = fit(2.4, { alignItems: 'flex-start', transformOrigin: 'left center' }, spread(2.4, col({ gap: 22, alignItems: 'flex-start' }, ...people.map(entry))));
    return { w: W, h: H, fit: { padX: 110, padY: 220 }, tree: col({ width: W, height: H, padding: '104px 110px 96px', justifyContent: 'space-between', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      box({ justifyContent: 'space-between', alignItems: 'baseline' },
        label(t, 'creditia presents', 13, { letterSpacing: 6 }),
        txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, range(m))),
      box({ flexGrow: 1, alignItems: 'center' }, list),
      col({ gap: 16 },
        box({ height: 1, backgroundColor: t.line }),
        box({ justifyContent: 'space-between', alignItems: 'flex-end', gap: 30 },
          txt({ fontFamily: t.display, fontSize: 80 * (serif ? 1.05 : 0.9), fontWeight: t.displayWeight, lineHeight: 0.95, letterSpacing: serif ? 0 : -2 }, m.project),
          label(t, totals(m), 12, { marginBottom: 6 })))) };
  }

  if (v === 'production') {
    // 出品：电影开场的出品卡。主笔「出品」，其余「联合出品」；大片名；底下列模型和审阅
    const models = [...new Set(c.authors.flatMap((r) => r.models.map(modelName)))].slice(0, 6);
    const small = (s: string, style: Record<string, any> = {}) => txt({ fontFamily: t.labelFont, fontSize: 15, color: t.soft, letterSpacing: 4, textTransform: 'uppercase', textAlign: 'center', ...style }, s);
    return { w: W, h: H, fit: { padX: 90, padY: 120 }, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', padding: '0 90px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      fit(1.7, { alignItems: 'center', maxWidth: 880 },
        c.lead ? box({ alignItems: 'center', gap: 12 }, mark(c.lead.key, t, 22), small(`${c.lead.name} 出品`, { color: t.ink })) : null,
        c.rest.length ? col({ alignItems: 'center', gap: 8, marginTop: 22 },
          small('联合出品', { fontSize: 12, color: t.faint }),
          txt({ fontFamily: t.display, fontSize: 26, color: t.ink, textAlign: 'center', lineHeight: 1.4, maxWidth: 760 }, c.rest.map((p) => p.name).join('  ·  '))) : null,
        ornament(t, 13, { margin: '40px 0 26px' }),
        txt({ fontFamily: t.display, fontSize: 112 * (serif ? 1 : 0.86), fontWeight: t.displayWeight, lineHeight: 1, textAlign: 'center', letterSpacing: serif ? 0 : -3, maxWidth: 880 }, m.project),
        ornament(t, 13, { margin: '28px 0 40px' }),
        models.length ? col({ alignItems: 'center', gap: 8 }, small('模型', { fontSize: 12, color: t.accent }), txt({ fontSize: 17, color: t.soft, textAlign: 'center', lineHeight: 1.6, maxWidth: 720 }, models.join('  ·  '))) : null,
        c.reviewers.length ? col({ alignItems: 'center', gap: 8, marginTop: 24 }, small('审阅', { fontSize: 12, color: t.accent }), txt({ fontSize: 17, color: t.soft, textAlign: 'center' }, c.reviewers.map((p) => p.name).join('  ·  '))) : null,
        txt({ fontFamily: MONO_FONT, fontSize: 12, color: t.faint, marginTop: 44, letterSpacing: 1 }, [range(m), totals(m)].filter(Boolean).join('   ·   ')))) };
  }

  if (v === 'slate') {
    // 场记板：黑板白字（深色主题反过来是白板黑字）。上面一根斜纹拍板，下面是格子
    const bg = t.ink, fg = t.bg;
    const rule = hexA(fg, '40');
    const tags: { tag: string }[] = m.raw.versions ?? [];
    const cell = (k: string, v: string, grow = 1, size = 40) => col({ flexGrow: grow, flexBasis: 0, padding: '16px 22px 18px', gap: 6, borderLeft: `2px solid ${rule}` },
      txt({ fontFamily: MONO_FONT, fontSize: 13, color: hexA(fg, 'A6'), letterSpacing: 3 }, k),
      txt({ fontFamily: t.display, fontSize: size, fontWeight: t.displayWeight, color: fg, lineHeight: 1.08 }, v || '—'));
    const row = (...cells: Node[]) => box({ borderTop: `2px solid ${rule}`, marginLeft: -2 }, ...cells);
    const stripe = `repeating-linear-gradient(135deg, ${fg} 0px, ${fg} 44px, ${bg} 44px, ${bg} 88px)`;
    const others = [...new Set([...c.rest.map((p) => p.name), ...c.reviewers.map((p) => p.name)])];
    return { w: W, h: H, fit: { padX: 70, padY: 110 }, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      fit(1.15, { alignItems: 'center', width: 900 },
        box({ width: 900, height: 78, backgroundImage: stripe, border: `5px solid ${bg}`, borderRadius: 8, transform: 'rotate(-6deg)', transformOrigin: 'left bottom', marginBottom: 14 }),
        box({ width: 900, height: 52, backgroundImage: stripe, border: `5px solid ${bg}`, borderRadius: 6 }),
        col({ width: 900, backgroundColor: bg, borderRadius: 10, marginTop: 6, overflow: 'hidden' },
          box({ padding: '22px 24px 20px' }, col({ gap: 6 },
            txt({ fontFamily: MONO_FONT, fontSize: 13, color: hexA(fg, 'A6'), letterSpacing: 3 }, 'PROD.'),
            txt({ fontFamily: t.display, fontSize: 76, fontWeight: t.displayWeight, color: fg, lineHeight: 1 }, m.project))),
          row(cell('ROLL', tags.length ? tags[tags.length - 1].tag : 'v0', 1, 36), cell('SCENE', fmt(m.commits), 1, 36), cell('TAKE', fmt(m.sessions), 1, 36)),
          row(cell('DIRECTOR', c.lead?.name ?? '', 1.3, 34), cell('CAMERA', c.lead?.model ?? '', 1, 30)),
          row(cell('DATE', m.range.to ? dot(m.range.to) : '', 1, 30), cell('LINES', `+${fmt(m.linesAdded)}`, 1.3, 30))),
        others.length ? box({ marginTop: 34, gap: 14, alignItems: 'baseline', maxWidth: 900, flexWrap: 'wrap', justifyContent: 'center' },
          label(t, 'with', 13, { letterSpacing: 5 }),
          txt({ fontFamily: t.display, fontSize: 26, color: t.ink, textAlign: 'center' }, others.join('  ·  '))) : null)) };
  }

  if (v === 'crew') {
    // 部门：片尾的特效、摄影部门那种名单。每个工具一个部门，底下是它用到的模型和各写了多少行
    const per = modelLines(m);
    const depts = c.authors.map((r) => ({ r, crew: (per.get(r.key) ?? []).slice(0, 4) }));
    const row = (name: string, lines: number) => box({ alignItems: 'baseline', height: 40 },
      eq('crew', box({ justifyContent: 'flex-end' }, txt({ fontFamily: t.display, fontSize: 24, fontWeight: t.displayWeight, color: t.ink, whiteSpace: 'nowrap' }, name || '—'))),
      box({ width: 48, justifyContent: 'center' }, txt({ fontSize: 13, color: t.faint }, '·')),
      eq('crew', box({}, txt({ fontFamily: MONO_FONT, fontSize: 15, color: t.soft, whiteSpace: 'nowrap' }, `${fmt(lines)} 行`))));
    const dept = (d: (typeof depts)[number]) => col({ alignItems: 'center', gap: 10 },
      box({ alignItems: 'center', gap: 12 }, mark(d.r.key, t, 20), label(t, d.r.name, 14, { color: t.accent, letterSpacing: 6 })),
      col({}, ...(d.crew.length ? d.crew : [[d.r.models[0] ? modelName(d.r.models[0]) : '', d.r.lines] as [string, number]]).map(([n, l]) => row(n, l))));
    return { w: W, h: H, fit: { padX: 80, padY: 110 }, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      fit(2.2, { alignItems: 'center' },
        label(t, 'credits', 12, { letterSpacing: 8 }),
        txt({ fontFamily: t.display, fontSize: 54, fontWeight: t.displayWeight, margin: '12px 0 10px', textAlign: 'center' }, m.project),
        ornament(t, 11),
        spread(2.6, col({ gap: 34, marginTop: 36, alignItems: 'center' },
          ...depts.map(dept),
          c.reviewers.length ? col({ alignItems: 'center', gap: 10 }, label(t, '审阅', 14, { color: t.accent, letterSpacing: 6 }), txt({ fontFamily: t.display, fontSize: 24, color: t.ink }, c.reviewers.map((p) => p.name).join('  ·  '))) : null)),
        label(t, totals(m), 11, { marginTop: 40 }))) };
  }
  return null;
}
