// 全部展示形式。规则：一条信息只出现一次；不写凑数的话；没有真数据、真素材的元素不出现；
// 主导者只在 opt.director 打开时出现。
//
// 风格之间靠 family 拉开距离：同一种形式在六个家族里是不同的排版结构，而不是换个颜色。
// 片尾字幕和小票另有可选的版式变体（variant）。
import type { Theme } from './themes.ts';
import type { Model, Contributor } from './data.ts';
import { fmt, pct, compact, nameOf } from './data.ts';
import { BRANDS, colorOf, famColor, logoSvg } from './brand.ts';
import { h, box, col, txt, label, mark, bar, fill, backdrop, surface, svgImage, stripes, blocks, plate, plateMetrics, ornament, MONO_FONT, type Node, type Out } from './ui.ts';

const keysOf = (m: Model) => m.rows.map((r) => r.key);
/** 主演名单：有 logo 的 AI 工具按名次排 */
const castOf = (m: Model, n: number) => m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).slice(0, n).map((r) => r.name);
const rankOf = (m: Model, key: string) => Math.max(0, m.rows.findIndex((r) => r.key === key));
/** 真正的 AI 工具（CI 机器人不算）*/
const isAi = (k: string) => !!BRANDS[k]?.logo && k !== 'github-actions';
/** 细线：杂志和单色风格的 line 是纯黑，只适合描边，分隔线要调淡 */
const hair = (t: Theme) => (t.family === 'editorial' ? 'rgba(18,17,16,0.28)' : t.family === 'mono' ? 'rgba(0,0,0,0.18)' : t.line);
const hexA = (hex: string, a: string) => (/^#[0-9a-fA-F]{6}$/.test(hex) ? hex + a : hex);
/** satori 不支持 dotted 边框；点线引导用 repeating-linear-gradient 画 */
const dotline = (t: Theme, color?: string) => ({ height: 2, alignSelf: 'center' as const, backgroundImage: `repeating-linear-gradient(90deg, ${color ?? t.faint} 0px, ${color ?? t.faint} 2px, transparent 2px, transparent 7px)` });

/** 人：不和 agent 同级。单独一行小字写名字和占比，不放条形 */
function humanLine(m: Model, t: Theme, s = 1): Node | null {
  if (!m.humans.length) return null;
  const text = m.humans.slice(0, 4).map((x) => `${x.name} ${pct(x.pct)}`).join('　') + (m.humans.length > 4 ? ` 等 ${m.humans.length} 人` : '');
  switch (t.family) {
    case 'film':
      return box({ justifyContent: 'center', marginTop: 16 * s }, txt({ fontFamily: t.display, fontSize: 15 * s, color: t.soft }, `— 人 · ${text} —`));
    case 'terminal':
      return txt({ fontFamily: MONO_FONT, fontSize: 11 * s, color: t.faint, marginTop: 10 * s }, `# 人写的部分：${text}`);
    case 'editorial':
      return box({ marginTop: 14 * s, alignItems: 'flex-start', gap: 10 * s },
        label(t, '人', 10 * s, { marginTop: 2 * s }), txt({ fontSize: 13 * s, color: t.soft }, text));
    case 'mono':
      return box({ marginTop: 12 * s, alignItems: 'center', gap: 8 * s },
        box({ width: 8 * s, height: 8 * s, backgroundColor: t.ink }),
        txt({ fontSize: 10.5 * s, letterSpacing: 1 * s, textTransform: 'uppercase', color: t.soft }, `人 · ${text}`));
    case 'glass':
      return txt({ fontSize: 12 * s, color: '#5E6472', marginTop: 12 * s }, `人 · ${text}`);
    default:
      return box({ marginTop: 12 * s, alignItems: 'flex-start', gap: 10 * s },
        label(t, '人', 10 * s, { marginTop: 2 * s }), txt({ fontSize: 12.5 * s, color: t.soft }, text));
  }
}

// ═════════════════════════ 一眼看懂 ═════════════════════════

/** Tier：谁做的、各占多少。按家族用不同排版 */
export function tier(m: Model, t: Theme, s = 2, withTitle = true): Out {
  switch (t.family) {
    case 'film': return tierFilm(m, t, s, withTitle);
    case 'terminal': return tierTerminal(m, t, s, withTitle);
    case 'editorial': return tierEditorial(m, t, s, withTitle);
    case 'mono': return tierMono(m, t, s, withTitle);
    case 'glass': return tierGlass(m, t, s, withTitle);
    case 'cinema': case 'playbill': case 'gala': case 'broadsheet':
    case 'crt': case 'arcade': case 'neon': case 'poster':
      return tierStyled(m, t, s, withTitle);
    default: return tierMinimal(m, t, s, withTitle);
  }
}

/** Minimal：留白、发丝线、细胶囊条 */
function tierMinimal(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 52;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const head = (withTitle ? 64 : 44) + (m.director ? 22 : 0);
  const H = (28 + head + rows.length * rowH + 46 + 28 + (m.humans.length ? 30 : 0)) * s;
  const row = (r: Contributor) => col({ height: rowH * s, justifyContent: 'center', gap: 9 * s },
    box({ alignItems: 'center', gap: 10 * s },
      mark(r.key, t, 16 * s),
      txt({ fontSize: 14 * s, fontWeight: 500, color: r.key === 'unattributed' ? t.soft : t.ink, fontFamily: t.sans }, r.name),
      txt({ marginLeft: 'auto', fontSize: 13 * s, color: r.key === 'unattributed' ? t.faint : t.soft, fontFamily: t.num, fontWeight: t.num === t.sans ? 500 : 400 }, pct(r.pct))),
    bar(t, r, rankOf(m, r.key), t.barH * s, 3 * s));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: 28 * s, ...surface(t, { borderRadius: t.radius * s }), color: t.ink, fontFamily: t.sans },
    col({ height: head * s, justifyContent: 'flex-start', gap: 6 * s },
      box({ alignItems: 'baseline', justifyContent: withTitle ? 'space-between' : 'flex-end' },
        withTitle ? txt({ fontFamily: t.display, fontSize: 18 * s, fontWeight: t.displayWeight, color: t.ink }, m.project) : null,
        label(t, 'Credits', 10 * s)),
      m.director ? txt({ fontSize: 12 * s, color: t.soft }, m.director) : null),
    col({}, ...rows.map(row)),
    humanLine(m, t, s),
    box({ height: 46 * s, alignItems: 'flex-end', borderTop: `1px solid ${t.line}`, paddingBottom: 2 * s },
      txt({ fontSize: 12 * s, color: t.soft, fontFamily: t.sans }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')))) };
}

/** Film Credits：居中、衬线、大字距，没有条形 */
function tierFilm(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 46;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const head = (withTitle ? 118 : 40) + (m.director ? 24 : 0);
  const H = (32 + head + rows.length * rowH + 44 + 32 + (m.humans.length ? 30 : 0)) * s;
  const row = (r: Contributor) => box({ height: rowH * s, alignItems: 'baseline', justifyContent: 'center', gap: 12 * s },
    mark(r.key, t, 15 * s),
    txt({ fontFamily: t.display, fontSize: 26 * s, fontWeight: 400, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
    txt({ fontFamily: t.num, fontSize: 13 * s, color: t.faint }, pct(r.pct)));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: 32 * s, alignItems: 'center', ...surface(t, { borderRadius: t.radius * s }), color: t.ink, fontFamily: t.sans, justifyContent: 'flex-start' },
    col({ height: head * s, alignItems: 'center', justifyContent: 'center', gap: 10 * s },
      label(t, 'Credits', 10 * s, { color: t.accent, letterSpacing: 4 * s }),
      withTitle ? txt({ fontFamily: t.display, fontSize: 36 * s, color: t.ink, lineHeight: 1.05, textAlign: 'center' }, m.project) : null,
      m.director ? txt({ fontFamily: t.display, fontSize: 15 * s, color: t.soft }, `由 ${m.director} 主导`) : null,
      txt({ fontSize: 11 * s, color: t.accent, letterSpacing: 5 * s, marginTop: 2 * s }, '· · ·')),
    col({ width: '100%' }, ...rows.map(row)),
    humanLine(m, t, s),
    box({ height: 44 * s, alignItems: 'flex-end', justifyContent: 'center', width: '100%' },
      txt({ fontSize: 10 * s, color: t.soft, letterSpacing: 2.2 * s, textTransform: 'uppercase' }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join(' · ')))) };
}

/** Terminal：一段终端输出，方块条 */
function tierTerminal(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 40;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const H = (34 + 16 + (withTitle ? 26 : 0) + (m.director ? 20 : 0) + rows.length * rowH + 16 + 22 + 16 + (m.humans.length ? 30 : 0)) * s;
  const row = (r: Contributor, last: boolean) => box({ height: rowH * s, alignItems: 'center', gap: 10 * s },
    txt({ fontFamily: MONO_FONT, fontSize: 13 * s, color: t.faint }, last ? '└─' : '├─'),
    mark(r.key, t, 14 * s),
    txt({ fontSize: 14 * s, color: r.key === 'unattributed' ? t.soft : t.ink, fontFamily: MONO_FONT }, r.name),
    box({ marginLeft: 'auto' }, blocks(r, t, rankOf(m, r.key), 9, 12 * s)),
    txt({ width: 52 * s, textAlign: 'right', fontFamily: MONO_FONT, fontSize: 12 * s, color: t.soft }, pct(r.pct)));
  return { w: W, h: H, tree: col({ width: W, height: H, backgroundColor: t.surface, border: `1px solid ${t.edge}`, borderRadius: t.radius * s, overflow: 'hidden', color: t.ink, fontFamily: MONO_FONT },
    box({ height: 34 * s, alignItems: 'center', padding: `0 ${14 * s}px`, gap: 8 * s, borderBottom: `1px solid ${t.edge}` },
      txt({ fontFamily: MONO_FONT, fontSize: 13 * s, color: t.accent }, '❯'),
      txt({ fontFamily: MONO_FONT, fontSize: 13 * s }, 'creditia cred'),
      txt({ marginLeft: 'auto', fontFamily: MONO_FONT, fontSize: 12 * s, color: t.faint }, withTitle ? m.project : '')),
    col({ padding: `${14 * s}px ${18 * s}px ${16 * s}px` },
      m.director ? txt({ fontFamily: MONO_FONT, fontSize: 12 * s, color: t.faint, marginBottom: 8 * s }, `# 由 ${m.director} 主导`) : null,
      ...rows.map((r, i) => row(r, i === rows.length - 1)),
      humanLine(m, t, s),
      txt({ fontFamily: MONO_FONT, fontSize: 11 * s, color: t.faint, marginTop: 12 * s }, [
        m.commits && `+${fmt(m.commits)} commits`, m.linesAdded && `+${fmt(m.linesAdded)} lines`,
      ].filter(Boolean).join('   ')))) };
}

/** Editorial：杂志目录页，点线引导 + 衬线大数字 */
function tierEditorial(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 54;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const head = (withTitle ? 92 : 40) + (m.director ? 26 : 0);
  const H = (28 + head + rows.length * rowH + 8 + 42 + 24 + (m.humans.length ? 30 : 0)) * s;
  const leader = box({ flex: 1, margin: `0 ${10 * s}px`, ...dotline(t) });
  const row = (r: Contributor) => box({ height: rowH * s, alignItems: 'baseline', borderBottom: `1px solid ${hair(t)}` },
    txt({ fontSize: 16 * s, fontWeight: 500, color: r.key === 'unattributed' ? t.soft : t.ink, fontFamily: t.sans }, r.name),
    leader,
    txt({ fontFamily: t.num, fontSize: 24 * s, fontWeight: t.displayWeight, color: r.key === 'unattributed' ? t.soft : t.ink }, pct(r.pct)));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: `${24 * s}px ${26 * s}px`, ...surface(t, { borderRadius: 0 }), color: t.ink, fontFamily: t.sans },
    col({ height: head * s, gap: 8 * s },
      box({ alignItems: 'baseline', justifyContent: 'space-between' },
        withTitle ? txt({ fontFamily: t.display, fontSize: 40 * s, fontWeight: t.displayWeight, lineHeight: 1 }, m.project) : null,
        label(t, 'Credits', 10 * s, { color: t.soft })),
      m.director ? txt({ fontFamily: t.display, fontSize: 15 * s, color: t.soft }, `由 ${m.director} 主导`) : null,
      box({ height: 3 * s, backgroundColor: t.ink, marginTop: 4 * s })),
    col({}, ...rows.map(row)),
    humanLine(m, t, s),
    box({ marginTop: 10 * s, borderTop: `2px solid ${t.ink}`, paddingTop: 10 * s, alignItems: 'center', justifyContent: 'space-between' },
      txt({ fontSize: 11 * s, color: t.soft, letterSpacing: 1.4 * s, textTransform: 'uppercase' }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')),
      box({ width: 8 * s, height: 8 * s, backgroundColor: t.accent }))) };
}

/** Mono：黑白海报，百分比是黑底白字的方块 */
function tierMono(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 52;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const H = ((withTitle ? 46 : 0) + rows.length * rowH + (m.director ? 26 : 0) + 46 + (m.humans.length ? 34 : 0)) * s;
  const row = (r: Contributor) => box({ height: rowH * s, alignItems: 'center', gap: 12 * s, borderBottom: `1px solid ${hair(t)}` },
    box({ width: 44 * s, height: 26 * s, backgroundColor: r.key === 'unattributed' ? t.faint : t.ink, alignItems: 'center', justifyContent: 'center' },
      txt({ fontFamily: t.num, fontSize: 12 * s, fontWeight: 700, color: '#FFFFFF' }, pct(r.pct))),
    txt({ fontSize: 14 * s, fontWeight: 700, letterSpacing: 0.6 * s, textTransform: 'uppercase', color: r.key === 'unattributed' ? t.soft : t.ink, fontFamily: t.sans }, r.name),
    box({ marginLeft: 'auto' }, mark(r.key, { ...t, dark: false } as Theme, 13 * s, t.ink)));
  return { w: W, h: H, tree: col({ width: W, height: H, ...surface(t, { borderRadius: 0, padding: 0 }), overflow: 'hidden', color: t.ink, fontFamily: t.sans },
    withTitle ? box({ height: 46 * s, backgroundColor: t.ink, alignItems: 'center', padding: `0 ${16 * s}px` },
      txt({ color: '#FFFFFF', fontSize: 15 * s, fontWeight: 700, letterSpacing: 1 * s, textTransform: 'uppercase', fontFamily: t.display }, m.project),
      txt({ marginLeft: 'auto', color: 'rgba(255,255,255,0.72)', fontSize: 10 * s, letterSpacing: 2 * s, textTransform: 'uppercase' }, 'Credits')) : null,
    m.director ? txt({ padding: `${6 * s}px ${16 * s}px 0`, fontSize: 12 * s, color: t.soft }, m.director) : null,
    col({ padding: `0 ${16 * s}px`, flexGrow: 1 }, ...rows.map(row), humanLine(m, t, s)),
    box({ height: 46 * s, backgroundColor: t.ink, alignItems: 'center', padding: `0 ${16 * s}px` },
      txt({ color: '#FFFFFF', fontSize: 11 * s, letterSpacing: 1.2 * s, textTransform: 'uppercase' }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')))) };
}

/** Glass：柔光背景、悬浮玻璃卡、每行一颗药丸 */
function tierGlass(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const W = 400 * s, rowH = 64;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const head = (withTitle ? 76 : 40) + (m.director ? 22 : 0);
  const H = (24 + head + rows.length * rowH - 8 + 44 + 24 + (m.humans.length ? 30 : 0)) * s;
  const pill = (r: Contributor) => col({ height: rowH * s - 8 * s, marginBottom: 8 * s, padding: `${10 * s}px ${14 * s}px`, borderRadius: 18 * s, backgroundColor: 'rgba(255,255,255,0.66)', border: '1px solid rgba(255,255,255,0.9)', boxShadow: '0 1px 2px rgba(20,22,28,0.05), 0 6px 16px rgba(20,22,28,0.05)', justifyContent: 'center', gap: 7 * s },
    box({ alignItems: 'center', gap: 10 * s },
      box({ width: 26 * s, height: 26 * s, borderRadius: 99, alignItems: 'center', justifyContent: 'center', backgroundColor: r.key === 'unattributed' ? 'rgba(255,255,255,0.6)' : hexA(famColor(r.key, t), '26') },
        mark(r.key, t, 13 * s, r.key === 'unattributed' ? t.faint : famColor(r.key, t))),
      txt({ fontSize: 14 * s, fontWeight: 600, color: t.ink }, r.name),
      txt({ marginLeft: 'auto', fontSize: 14 * s, fontWeight: 700, color: t.ink, fontFamily: t.num }, pct(r.pct))),
    bar(t, r, rankOf(m, r.key), 4 * s, 3 * s));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: 24 * s, ...backdrop({ ...t, glow: true }, keysOf(m), { backgroundColor: undefined }), borderRadius: t.radius * s, ...(t.shadow !== 'none' ? { boxShadow: t.shadow } : {}), color: t.ink, fontFamily: t.sans },
    col({ height: head * s, gap: 4 * s },
      box({ alignItems: 'baseline', justifyContent: 'space-between' },
        withTitle ? txt({ fontFamily: t.display, fontSize: 20 * s, fontWeight: t.displayWeight, color: '#14161C' }, m.project) : null,
        label(t, 'Credits', 10 * s, { color: '#5E6472' })),
      m.director ? txt({ fontSize: 12 * s, color: '#5E6472' }, m.director) : null),
    col({}, ...rows.map(pill)),
    humanLine(m, t, s),
    box({ height: 44 * s, alignItems: 'flex-end', justifyContent: 'space-between', paddingTop: 12 * s },
      txt({ fontSize: 12 * s, color: '#5E6472' }, [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')))) };
}

// ═════════════════════════ 分享图 ═════════════════════════

/** 分享图 1200×630：按家族构图 */
export function share(m: Model, t: Theme): Out {
  const W = 1200, H = 630;
  const ym = (d: string) => d.slice(0, 7).replace('-', '.');
  const range = m.range.from && m.range.to ? `${ym(m.range.from)} — ${ym(m.range.to)}` : '';
  const totals = [range, m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `+${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ');
  switch (t.family) {
    case 'film': {
      const card = tier(m, t, Math.min(1.35, 590 / tier(m, t, 1, true).h), true);
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) }, card.tree) };
    }
    case 'terminal': {
      const card = tier(m, t, Math.min(1.25, 590 / tier(m, t, 1, false).h), false);
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', padding: '0 84px', gap: 64, ...backdrop(t, keysOf(m)), fontFamily: MONO_FONT },
        col({ flex: 1, gap: 16 },
          txt({ fontFamily: MONO_FONT, fontSize: 20, color: t.accent }, '❯ creditia cred'),
          txt({ fontFamily: MONO_FONT, fontSize: 56, fontWeight: 700, color: t.ink, lineHeight: 1.1 }, m.project),
          range ? txt({ fontFamily: MONO_FONT, fontSize: 17, color: t.soft }, `${range}   ${fmt(m.commits)} commits`) : null),
        card.tree) };
    }
    case 'glass': {
      const card = tier(m, t, Math.min(1.4, 590 / tier(m, t, 1, true).h), true);
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop({ ...t, glow: true }, keysOf(m), { backgroundColor: '#EEF0F5' }) }, card.tree) };
    }
    case 'editorial': {
      const ranked = m.rows.filter((r) => r.key !== 'human');
      const half = Math.ceil(ranked.length / 2);
      const line = (r: Contributor) => box({ alignItems: 'baseline', padding: '11px 0', borderBottom: `1px solid ${hair(t)}` },
        txt({ fontSize: 19, fontWeight: 500, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
        box({ flex: 1, margin: '0 10px', ...dotline(t) }),
        txt({ fontFamily: t.num, fontSize: 26, color: t.ink }, pct(r.pct)));
      const colOf = (rs: Contributor[]) => col({ flex: 1, gap: 0 }, ...rs.map(line));
      return { w: W, h: H, tree: col({ width: W, height: H, padding: '42px 68px 36px', backgroundColor: t.bg, color: t.ink, fontFamily: t.sans },
        box({ alignItems: 'baseline', justifyContent: 'space-between' },
          box({ alignItems: 'center', gap: 10 },
            box({ width: 9, height: 9, backgroundColor: t.accent }),
            label(t, 'Creditia — 作品署名', 12)),
          label(t, range ? `${range}  ·  vol. 1` : 'vol. 1', 12)),
        box({ height: 3, backgroundColor: t.ink, marginTop: 10 }),
        txt({ fontFamily: t.display, fontSize: 112, fontWeight: t.displayWeight, lineHeight: 1, marginTop: 18 }, m.project),
        box({ marginTop: 14, borderTop: `2px solid ${t.ink}`, paddingTop: 8, flex: 1, gap: 60 },
          colOf(ranked.slice(0, half)), colOf(ranked.slice(half))),
        m.humans.length ? box({ marginTop: 10, alignItems: 'baseline', gap: 8 }, label(t, '人', 10), txt({ fontSize: 13, color: t.soft }, m.humans.map((x) => `${x.name} ${pct(x.pct)}`).join('　'))) : null,
        box({ borderTop: `3px solid ${t.ink}`, paddingTop: 10, alignItems: 'baseline', justifyContent: 'space-between' },
          txt({ fontSize: 12, letterSpacing: 1.4, textTransform: 'uppercase' }, [m.commits && `${fmt(m.commits)} 次提交`, m.units && `${fmt(m.units)} 项`, m.linesAdded && `+${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ')),
          m.director ? txt({ fontFamily: t.display, fontSize: 16 }, m.director) : null)) };
    }
    case 'mono': {
      const ranked = m.rows.filter((r) => r.key !== 'human');
      const row = (r: Contributor) => box({ height: 62, alignItems: 'center', gap: 14, borderBottom: `1px solid ${hair(t)}` },
        box({ width: 52, height: 28, backgroundColor: r.key === 'unattributed' ? t.faint : t.ink, alignItems: 'center', justifyContent: 'center' },
          txt({ fontFamily: t.num, fontSize: 13, fontWeight: 700, color: '#FFFFFF' }, pct(r.pct))),
        txt({ fontSize: 19, fontWeight: 700, letterSpacing: 0.8, textTransform: 'uppercase', color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
        box({ marginLeft: 'auto' }, mark(r.key, t, 15)));
      return { w: W, h: H, tree: box({ width: W, height: H, backgroundColor: '#FFFFFF', color: '#000000', fontFamily: t.sans },
        col({ width: 430, height: H, backgroundColor: '#000000', padding: '46px 40px', justifyContent: 'space-between' },
          txt({ color: '#FFFFFF', fontSize: 44, fontWeight: 700, letterSpacing: 1, lineHeight: 1.08, textTransform: 'uppercase' }, m.project),
          col({ gap: 12 },
            txt({ color: 'rgba(255,255,255,0.62)', fontSize: 11, letterSpacing: 2.4, textTransform: 'uppercase' }, 'Credits'),
            txt({ color: '#FFFFFF', fontSize: 13, letterSpacing: 1.2 }, [range, m.commits && `${fmt(m.commits)} commits`].filter(Boolean).join('  ·  ')),
            m.director ? txt({ color: '#FFFFFF', fontSize: 15 }, m.director) : null)),
        col({ flex: 1, padding: '46px 44px 30px' },
          box({ flex: 1 }, col({ width: '100%' }, ...ranked.map(row), m.humans.length ? txt({ marginTop: 10, fontSize: 12, color: '#6B6B6B', letterSpacing: 0.6 }, `人 · ${m.humans.map((x) => `${x.name} ${pct(x.pct)}`).join('　')}`) : null)),
          box({ alignItems: 'center', justifyContent: 'space-between', paddingTop: 14 },
            label(t, 'with creditia', 10, { color: '#6B6B6B' }),
            txt({ fontSize: 11, letterSpacing: 1.6, textTransform: 'uppercase', color: '#6B6B6B' }, m.linesAdded ? `+${fmt(m.linesAdded)} 行` : '')))) };
    }
    case 'cinema': {
      const names = castOf(m, 4);
      return { w: W, h: H, tree: plate(t, { width: W, height: H }, [
        col({ flexGrow: 1, alignItems: 'center', justifyContent: 'center', gap: 22 },
          label(t, 'creditia presents', 11, { color: t.accent, letterSpacing: 6 }),
          txt({ fontFamily: t.deco, fontSize: 82, color: t.ink, textAlign: 'center', lineHeight: 1.05, letterSpacing: 2 }, m.project),
          ornament(t, 14),
          box({ gap: 22, alignItems: 'center' }, ...names.flatMap((n, i) => (i ? [txt({ color: t.accent, fontSize: 15 }, '◆')] : []).concat(txt({ fontFamily: t.display, fontSize: 25, color: t.ink, letterSpacing: 1.5 }, n)))),
          txt({ fontFamily: t.labelFont, fontSize: 11, color: t.soft, letterSpacing: 3, textTransform: 'uppercase' }, totals)),
      ]) };
    }
    case 'playbill': {
      const rows = m.rows.filter((r) => r.key !== 'human').slice(0, 5);
      return { w: W, h: H, tree: plate(t, { width: W, height: H }, [
        box({ flexGrow: 1, alignItems: 'center', gap: 52, padding: '0 8px' },
          col({ width: 300, backgroundColor: t.accent, padding: 26, gap: 22, justifyContent: 'space-between' },
            txt({ fontFamily: t.labelFont, fontSize: 13, color: t.surface, letterSpacing: 4 }, 'PLAYBILL 节目单'),
            txt({ fontFamily: t.display, fontSize: 42, fontWeight: 900, color: t.surface, lineHeight: 1.08 }, m.project),
            ornament(t, 12, { color: t.surface })),
          col({ flex: 1, gap: 6 },
            box({ marginBottom: 8, alignItems: 'baseline', justifyContent: 'space-between' }, label(t, '今晚演出', 12), label(t, m.director || 'creditia', 11)),
            box({ height: 3, backgroundColor: t.ink, marginBottom: 12 }),
            ...rows.map((r, i) => box({ alignItems: 'baseline', padding: '10px 0', borderBottom: `1px solid ${hair(t)}` },
              txt({ width: 34, fontFamily: t.num, fontSize: 18, color: t.accent }, String(i + 1).padStart(2, '0')),
              txt({ fontSize: 18, fontWeight: 700 }, r.name),
              box({ flex: 1, margin: '0 10px', ...dotline(t) }),
              txt({ fontFamily: t.num, fontSize: 20 }, pct(r.pct)))),
            txt({ marginTop: 14, fontFamily: t.sans, fontSize: 12, fontStyle: 'italic', color: t.soft }, totals))),
      ]) };
    }
    case 'gala': {
      const names = castOf(m, 5);
      return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 20, ...backdrop(t, keysOf(m)) },
        txt({ fontFamily: t.labelFont, fontSize: 13, color: t.accent, letterSpacing: 10, textTransform: 'uppercase' }, 'the credit awards'),
        box({ width: 420, height: 1, backgroundColor: t.accent, opacity: 0.6 }),
        txt({ fontFamily: t.display, fontSize: 92, color: t.ink, textAlign: 'center', lineHeight: 1.05, letterSpacing: 4 }, m.project),
        box({ width: 420, height: 1, backgroundColor: t.accent, opacity: 0.6 }),
        box({ gap: 18, alignItems: 'center' }, ...names.flatMap((n, i) => (i ? [txt({ fontFamily: t.deco, color: t.accent, fontSize: 14 }, '◆')] : []).concat(txt({ fontFamily: t.labelFont, fontSize: 22, color: t.ink, letterSpacing: 3 }, n)))),
        txt({ fontFamily: t.labelFont, fontSize: 11, color: t.soft, letterSpacing: 5, textTransform: 'uppercase', marginTop: 6 }, m.director ? `授予 ${m.director}` : totals)) };
    }
    case 'broadsheet': {
      const ranked = m.rows.filter((r) => r.key !== 'human');
      const half = Math.ceil(ranked.length / 2);
      const line = (r: Contributor) => box({ alignItems: 'baseline', padding: '9px 0', borderBottom: `1px solid ${hair(t)}` },
        txt({ fontFamily: t.display, fontSize: 19, fontWeight: 700, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name),
        box({ flex: 1, margin: '0 10px', ...dotline(t) }),
        txt({ fontFamily: t.num, fontSize: 21 }, pct(r.pct)));
      return { w: W, h: H, tree: col({ width: W, height: H, padding: '34px 60px 28px', backgroundColor: t.bg, color: t.ink, fontFamily: t.sans },
        box({ justifyContent: 'space-between', alignItems: 'baseline' }, label(t, 'creditia daily', 11), label(t, (m.range.to || '').replaceAll('-', '.'), 11)),
        box({ borderTop: `2px solid ${t.ink}`, borderBottom: `1px solid ${t.ink}`, padding: '8px 0 6px', alignItems: 'center' },
          txt({ fontFamily: t.deco, fontSize: 56, lineHeight: 1 }, m.project)),
        box({ borderBottom: `5px solid ${t.ink}`, marginBottom: 12 }),
        box({ flex: 1, gap: 54 }, col({ flex: 1 }, ...ranked.slice(0, half).map(line)), col({ flex: 1 }, ...ranked.slice(half).map(line))),
        box({ borderTop: `5px solid ${t.ink}`, paddingTop: 9, justifyContent: 'space-between', alignItems: 'baseline' },
          label(t, totals, 11), label(t, m.director || 'creditia', 11))) };
    }
    case 'crt': {
      const ranked = m.rows.filter((r) => r.key !== 'human');
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
        col({ width: W - 220, backgroundColor: t.surface, border: `4px solid ${t.edge}`, padding: 20, gap: 4, backgroundImage: t.pattern },
          txt({ fontFamily: t.sans, fontSize: 22, color: t.accent }, `> creditia cred ${m.project}`),
          txt({ fontFamily: t.sans, fontSize: 18, color: t.faint, whiteSpace: 'pre' }, '─'.repeat(44)),
          ...ranked.slice(0, 5).map((r) => box({ alignItems: 'center', gap: 10 },
            txt({ fontFamily: t.sans, fontSize: 20, color: r.key === 'unattributed' ? t.soft : t.ink, width: 220 }, r.name),
            box({ flex: 1 }, blocks(r, t, rankOf(m, r.key), 24, 18)),
            txt({ fontFamily: t.sans, fontSize: 20, color: t.soft, width: 70, justifyContent: 'flex-end' }, pct(r.pct)))),
          txt({ fontFamily: t.sans, fontSize: 20, color: t.ink, marginTop: 6 }, `> ${totals}`))) };
    }
    case 'arcade': {
      const ranked = m.rows.filter((r) => r.key !== 'human').slice(0, 4);
      return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 22, backgroundColor: t.bg, color: t.ink, fontFamily: t.sans, border: `6px solid ${t.accent}` },
        txt({ fontFamily: t.display, fontSize: 22, color: t.accent, letterSpacing: 2, lineHeight: 1.7 }, 'HIGH SCORES'),
        txt({ fontFamily: t.sans, fontSize: 60, color: t.ink, textAlign: 'center' }, m.project),
        col({ width: 640, gap: 12, marginTop: 6 },
          ...ranked.map((r, i) => box({ alignItems: 'center', gap: 16 },
            txt({ fontFamily: t.display, fontSize: 13, color: i === 0 ? t.accent : t.faint, width: 40 }, String(i + 1).padStart(2, '0')),
            txt({ fontFamily: t.sans, fontSize: 24 }, r.name),
            box({ flex: 1, ...dotline(t, t.edge) }),
            txt({ fontFamily: t.display, fontSize: 14, color: i === 0 ? t.accent : t.ink }, pct(r.pct))))),
        txt({ fontFamily: t.display, fontSize: 13, color: t.faint, letterSpacing: 3, marginTop: 4 }, 'PRESS START')) };
    }
    case 'neon': {
      const names = castOf(m, 4);
      return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 26, ...backdrop(t, keysOf(m)), fontFamily: t.sans },
        box({ position: 'relative', alignItems: 'center', justifyContent: 'center' },
          txt({ position: 'absolute', fontFamily: t.display, fontSize: 92, color: t.accent, letterSpacing: 4, opacity: 0.55 }, m.project),
          txt({ fontFamily: t.display, fontSize: 92, color: t.ink, letterSpacing: 4 }, m.project)),
        box({ gap: 20, alignItems: 'center' }, ...names.flatMap((n, i) => (i ? [txt({ fontFamily: t.deco, color: t.accent, fontSize: 16 }, '★')] : []).concat(txt({ fontFamily: t.sans, fontSize: 26, color: t.ink, letterSpacing: 3 }, n)))),
        box({ padding: '6px 22px', border: `1px solid ${t.accent}`, borderRadius: 999 }, txt({ fontFamily: t.sans, fontSize: 13, color: t.accent, letterSpacing: 5 }, m.director ? `主理 ${m.director}` : totals))) };
    }
    case 'poster': {
      const ranked = m.rows.filter((r) => r.key !== 'human').slice(0, 5);
      return { w: W, h: H, tree: box({ width: W, height: H, backgroundColor: t.bg, color: t.ink, fontFamily: t.sans, border: `7px solid ${t.ink}` },
        col({ width: 520, backgroundColor: t.ink, color: '#FFFFFF', padding: 40, justifyContent: 'space-between' },
          box({ alignItems: 'center', gap: 14 },
            box({ width: 18, height: 18, backgroundColor: t.accent }),
            txt({ color: '#FFFFFF', fontSize: 16, fontWeight: 700, letterSpacing: 3 }, 'CREDITS')),
          txt({ fontFamily: t.display, fontSize: 62, color: '#FFFFFF', lineHeight: 1, textTransform: 'uppercase' }, m.project),
          txt({ color: 'rgba(255,255,255,0.7)', fontSize: 15, letterSpacing: 2, textTransform: 'uppercase' }, totals)),
        col({ flex: 1, padding: '40px 44px', gap: 0, justifyContent: 'center' },
          ...ranked.map((r, i) => box({ alignItems: 'center', padding: '12px 0', borderBottom: `2px solid ${t.line}` },
            txt({ width: 46, fontFamily: t.num, fontSize: 22, color: t.accent }, String(i + 1).padStart(2, '0')),
            txt({ fontSize: 22, fontWeight: 700 }, r.name),
            txt({ marginLeft: 'auto', fontFamily: t.num, fontSize: 24 }, pct(r.pct)))))) };
    }
    default: {
      const card = tier(m, t, Math.min(1.25, 590 / tier(m, t, 1, false).h), false);
      const left = col({ flex: 1, justifyContent: 'center', gap: 22 },
        txt({ fontFamily: t.display, fontSize: 84, fontWeight: t.displayWeight, color: t.ink, lineHeight: 1, letterSpacing: t.display === t.sans ? -2 : -1 }, m.project),
        range ? txt({ fontFamily: MONO_FONT, fontSize: 19, color: t.soft, letterSpacing: 1 }, range) : null);
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', padding: '0 88px', gap: 72, ...backdrop(t, keysOf(m)) }, left, card.tree) };
    }
  }
}

// ═════════════════════════ 署名行 ═════════════════════════

/** 署名行：一句话。按家族用不同排版 */
export function byline(m: Model, t: Theme, lang: 'zh' | 'en'): Out {
  const tools = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => r.name);
  const list = (a: string[], and: string, sep: string) => (a.length < 2 ? a.join('') : `${a.slice(0, -1).join(sep)}${and}${a[a.length - 1]}`);
  const who = m.director;
  const s = lang === 'zh'
    ? (who ? `${who} 与 ${list(tools, ' 和 ', '、')} 共同完成` : `与 ${list(tools, ' 和 ', '、')} 共同完成`)
    : (who ? `Made by ${who} with ${list(tools, ' & ', ', ')}` : `Made with ${list(tools, ' & ', ', ')}`);
  const W = 1200, H = 120;
  const logos = (size: number, color?: string) => box({ gap: 10, alignItems: 'center' }, ...m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => mark(r.key, t, size, color)));
  switch (t.family) {
    case 'film':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', gap: 12, ...backdrop(t, keysOf(m)) },
        box({ height: 1, backgroundColor: t.line, margin: '0 120px' }),
        box({ justifyContent: 'center', gap: 14, alignItems: 'center' },
          txt({ fontSize: 11, color: t.accent, letterSpacing: 4 }, '· · ·'),
          txt({ fontFamily: t.display, fontSize: 25, color: t.ink }, s),
          txt({ fontSize: 11, color: t.accent, letterSpacing: 4 }, '· · ·')),
        box({ height: 1, backgroundColor: t.line, margin: '0 120px' })) };
    case 'terminal':
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 18, ...backdrop(t, keysOf(m)), fontFamily: MONO_FONT },
        txt({ fontFamily: MONO_FONT, fontSize: 20, color: t.accent }, '❯ creditia byline'),
        txt({ fontFamily: MONO_FONT, fontSize: 20, color: t.ink }, s)) };
    case 'glass':
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop({ ...t, glow: true }, keysOf(m), { backgroundColor: '#EEF0F5' }) },
        box({ alignItems: 'center', gap: 22, padding: '16px 30px', borderRadius: 999, backgroundColor: 'rgba(255,255,255,0.62)', border: '1px solid rgba(255,255,255,0.9)', boxShadow: t.shadow },
          logos(22), txt({ fontSize: 20, color: '#14161C', fontWeight: 500 }, s))) };
    case 'editorial':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', backgroundColor: t.bg },
        box({ height: 2, backgroundColor: t.ink }),
        box({ justifyContent: 'center', alignItems: 'center', gap: 16, padding: '14px 0' },
          label(t, 'colophon', 10),
          txt({ fontFamily: t.display, fontSize: 25, color: t.ink }, s)),
        box({ height: 2, backgroundColor: t.ink })) };
    case 'mono':
      return { w: W, h: H, tree: box({ width: W, height: H, backgroundColor: '#000000', alignItems: 'center', justifyContent: 'center', gap: 22, padding: '0 40px' },
        logos(20, '#FFFFFF'),
        txt({ fontSize: 19, fontWeight: 700, letterSpacing: 1, textTransform: 'uppercase', color: '#FFFFFF' }, s)) };
    case 'cinema':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', alignItems: 'center', gap: 12, ...backdrop(t, keysOf(m)) },
        box({ height: 1, backgroundColor: t.edge, width: 980 }),
        box({ alignItems: 'center', gap: 18 }, ornament(t, 10), txt({ fontFamily: t.display, fontSize: 25, color: t.ink, letterSpacing: 1 }, s), ornament(t, 10)),
        box({ height: 1, backgroundColor: t.edge, width: 980 })) };
    case 'playbill':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', alignItems: 'center', gap: 10, backgroundColor: t.bg, borderTop: `1px dashed ${t.edge}`, borderBottom: `1px dashed ${t.edge}` },
        ornament(t, 12),
        txt({ fontFamily: t.sans, fontSize: 23, fontStyle: 'italic', color: t.ink }, s)) };
    case 'gala':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', alignItems: 'center', gap: 12, ...backdrop(t, keysOf(m)) },
        txt({ fontFamily: t.labelFont, fontSize: 11, color: t.accent, letterSpacing: 8, textTransform: 'uppercase' }, 'the credit awards'),
        txt({ fontFamily: t.labelFont, fontSize: 24, color: t.ink, letterSpacing: 3 }, s),
        box({ width: 620, height: 1, backgroundColor: t.accent, opacity: 0.7 })) };
    case 'broadsheet':
      return { w: W, h: H, tree: col({ width: W, height: H, justifyContent: 'center', backgroundColor: t.bg },
        box({ height: 4, backgroundColor: t.ink }),
        box({ justifyContent: 'center', alignItems: 'center', gap: 16, padding: '14px 0' },
          label(t, 'colophon 版权页', 10),
          txt({ fontFamily: t.display, fontSize: 24, fontWeight: 700, color: t.ink }, s),
          label(t, 'creditia', 10)),
        box({ height: 2, backgroundColor: t.ink })) };
    case 'crt':
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 16, backgroundColor: t.bg, backgroundImage: t.pattern, fontFamily: t.sans },
        txt({ fontSize: 22, color: t.accent }, '> creditia byline'),
        txt({ fontSize: 22, color: t.ink }, s),
        txt({ fontSize: 20, color: t.faint }, '█')) };
    case 'arcade':
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 20, backgroundColor: t.bg, borderTop: `4px solid ${t.accent}`, borderBottom: `4px solid ${t.accent}`, fontFamily: t.sans },
        txt({ fontFamily: t.display, fontSize: 12, color: t.faint }, '*  *  *'),
        txt({ fontSize: 24, color: t.ink }, s)) };
    case 'neon':
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)), fontFamily: t.sans },
        box({ alignItems: 'center', gap: 20, padding: '12px 34px', border: `1px solid ${t.accent}`, borderRadius: 999, backgroundColor: t.surface },
          ornament(t, 11), txt({ fontSize: 22, color: t.ink, letterSpacing: 2 }, s), ornament(t, 11))) };
    case 'poster':
      return { w: W, h: H, tree: box({ width: W, height: H, backgroundColor: t.ink, alignItems: 'center', justifyContent: 'center', gap: 22, padding: '0 44px' },
        box({ width: 16, height: 16, backgroundColor: t.accent }),
        txt({ fontFamily: t.sans, fontSize: 20, fontWeight: 700, letterSpacing: 1.6, textTransform: 'uppercase', color: '#FFFFFF' }, s)) };
    default:
      return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 22, ...backdrop(t, keysOf(m)), fontFamily: t.sans },
        logos(20, t.soft),
        txt({ fontSize: 20, color: t.soft }, s)) };
  }
}

// ═════════════════════════ 徽章 ═════════════════════════

/** 徽章：原生 SVG（README），不经 Satori。每种家族一种画法 */
export function badge(m: Model, t: Theme): string {
  const top = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key) && r.pct >= 1).slice(0, 3);
  const names = top.map((r) => r.name);
  const vtext = top.length ? top.map((r) => `${r.name} ${Math.round(r.pct)}%`).join('  ·  ') : `${fmt(m.linesAdded)} 行`;
  const list = (a: string[], and: string, sep: string) => (a.length < 2 ? a.join('') : `${a.slice(0, -1).join(sep)}${and}${a[a.length - 1]}`);
  const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
  const meas = (s: string, size: number, f: number) => [...s].reduce((a, ch) => a + (/[一-鿿]/.test(ch) ? size : /[ilj.,:;'!|·]/.test(ch) ? size * 0.32 : /[A-Z%]/.test(ch) ? size * 0.68 : size * f), 0);
  const icon = (key: string) => {
    const s = logoSvg(key);
    if (!s) return '';
    return s.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '').replace(/<title>.*?<\/title>/, '');
  };

  switch (t.family) {
    case 'film': {
      const text = `with ${list(names, ' · ', ' · ') || m.project}`.toUpperCase();
      const tw = meas(text, 10, 0.5) + Math.max(text.length - 1, 0) * 2.4;
      const W = Math.ceil(tw + 56), H = 22;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" rx="2" fill="#12110F"/>
<circle cx="16" cy="11" r="1.4" fill="#BFA46A"/><circle cx="${W - 16}" cy="11" r="1.4" fill="#BFA46A"/>
<text x="${W / 2}" y="14.5" text-anchor="middle" font-family="Georgia, 'Times New Roman', serif" font-size="10" letter-spacing="2.4" fill="#EDE6D8" font-style="italic">${esc(text)}</text></svg>`;
    }
    case 'terminal': {
      const pre = '❯ creditia  ', fs = 11;
      const W = Math.ceil(meas(pre, fs, 0.6) + meas(vtext, fs, 0.6) + 26), H = 20;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(vtext)}">
<rect width="${W}" height="${H}" fill="#0B0D0F"/>
<text x="12" y="14" font-family="Menlo, Consolas, monospace" font-size="${fs}" fill="#7FD08C">${esc(pre)}</text>
<text x="${Math.ceil(12 + meas(pre, fs, 0.6))}" y="14" font-family="Menlo, Consolas, monospace" font-size="${fs}" fill="#D7DEE5">${esc(vtext)}</text></svg>`;
    }
    case 'glass': {
      const c1 = top[0] ? colorOf(top[0].key, t) : '#5B7CFA', c2 = top[1] ? colorOf(top[1].key, t) : '#8EA2FF';
      const fs = 11.5, W = Math.ceil(meas(vtext, fs, 0.56) + (top.length ? 26 : 0) + 40), H = 24;
      const chip = top.length ? `<circle cx="16" cy="12" r="7.5" fill="rgba(255,255,255,0.28)"/><svg x="11" y="7" width="10" height="10" viewBox="0 0 24 24" fill="#FFFFFF">${icon(top[0].key)}</svg>` : '';
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(vtext)}">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${c1}"/><stop offset="1" stop-color="${c2}"/></linearGradient></defs>
<rect x=".5" y=".5" width="${W - 1}" height="${H - 1}" rx="12" fill="url(#g)" stroke="rgba(255,255,255,0.45)"/>
${chip}<text x="${top.length ? 30 : 20}" y="15.5" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="${fs}" font-weight="600" fill="#FFFFFF">${esc(vtext)}</text></svg>`;
    }
    case 'editorial': {
      const text = `${list(names, ' & ', ', ').toUpperCase() || m.project}${top.length ? `  —  ${top.map((r) => `${Math.round(r.pct)}%`).join(' · ')}` : ''}`;
      const fs = 10.5, W = Math.ceil(meas(text, fs, 0.5) + Math.max(text.length - 1, 0) * 1.2 + 54), H = 22;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" fill="#F2EDE3"/>
<rect width="${W}" height="1.5" fill="#121110"/><rect y="${H - 1.5}" width="${W}" height="1.5" fill="#121110"/>
<rect x="13" y="8" width="6" height="6" fill="#C8442E"/>
<text x="27" y="15" font-family="Georgia, 'Times New Roman', serif" font-size="${fs}" letter-spacing="1.2" fill="#121110">${esc(text)}</text></svg>`;
    }
    case 'mono': {
      const fs = 10;
      const l1 = 'CREDITIA', w1 = Math.ceil(meas(l1, fs, 0.62) + 18 + (l1.length - 1) * 0.8);
      const l2 = vtext.toUpperCase(), w2 = Math.ceil(meas(l2, fs, 0.62) + 18 + (l2.length - 1) * 0.8);
      const W = w1 + w2, H = 20;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(l2)}">
<rect width="${w1}" height="${H}" fill="#000000"/>
<rect x="${w1}" y=".5" width="${w2 - 1}" height="${H - 1}" fill="#FFFFFF" stroke="#000000"/>
<text x="${w1 / 2}" y="14" text-anchor="middle" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="${fs}" font-weight="700" letter-spacing="0.8" fill="#FFFFFF">${l1}</text>
<text x="${w1 + w2 / 2}" y="14" text-anchor="middle" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="${fs}" font-weight="700" letter-spacing="0.8" fill="#000000">${esc(l2)}</text></svg>`;
    }
    case 'cinema': {
      const text = `with ${list(names, ' ◆ ', ' ◆ ') || m.project}`.toUpperCase();
      const fs = 11, ls = 2.6;
      const W = Math.ceil(meas(text, fs, 0.6) + Math.max(text.length - 1, 0) * ls + 76), H = 28;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" fill="#08070A"/>
<rect x=".5" y=".5" width="${W - 1}" height="${H - 1}" fill="none" stroke="#D6B26A"/>
<rect x="5.5" y="5.5" width="${W - 11}" height="${H - 11}" fill="none" stroke="#D6B26A" stroke-opacity="0.5"/>
<text x="${W / 2}" y="19" text-anchor="middle" font-family="Georgia, 'Times New Roman', serif" font-size="${fs}" letter-spacing="${ls}" fill="#F2E8D2">${esc(text)}</text></svg>`;
    }
    case 'playbill': {
      const text = `a ${list(names, ' & ', ', ') || m.project} production`.toUpperCase();
      const fs = 11, ls = 1.6;
      const W = Math.ceil(meas(text, fs, 0.58) + Math.max(text.length - 1, 0) * ls + 66), H = 26;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" fill="#F1E6CC"/>
<rect width="${W}" height="3" fill="#8C1C13"/><rect y="${H - 3}" width="${W}" height="3" fill="#8C1C13"/>
<rect x="13" y="9.5" width="7" height="7" fill="#8C1C13"/>
<text x="28" y="17.5" font-family="Georgia, 'Times New Roman', serif" font-size="${fs}" font-weight="700" letter-spacing="${ls}" fill="#241A12">${esc(text)}</text></svg>`;
    }
    case 'gala': {
      const text = list(names, ' · ', ' · ').toUpperCase() || m.project.toUpperCase();
      const fs = 10.5, ls = 3.4;
      const tw = meas(text, fs, 0.6) + Math.max(text.length - 1, 0) * ls;
      const W = Math.ceil(tw + 58), H = 26;
      const dx = W / 2 - tw / 2 - 12;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="built with ${esc(text)}">
<rect width="${W}" height="${H}" fill="#0A0918"/>
<rect width="${W}" height="1" y="3" fill="#D4AF37" stroke-opacity="0.6"/><rect width="${W}" height="1" y="${H - 4}" fill="#D4AF37" stroke-opacity="0.6"/>
<path d="M${dx} 13 l4 -5 l4 5 l-4 5z" fill="#D4AF37"/>
<path d="M${W - dx} 13 l4 -5 l4 5 l-4 5z" fill="#D4AF37"/>
<text x="${W / 2}" y="16.5" text-anchor="middle" font-family="Georgia, 'Times New Roman', serif" font-size="${fs}" letter-spacing="${ls}" fill="#F6EFDD">${esc(text)}</text></svg>`;
    }
    case 'broadsheet': {
      const text = `${list(names, ' & ', ', ').toUpperCase() || m.project.toUpperCase()}${top.length ? `  §  ${top.map((r) => `${Math.round(r.pct)}%`).join(' · ')}` : ''}`;
      const fs = 10.5, ls = 0.9;
      const W = Math.ceil(meas(text, fs, 0.52) + Math.max(text.length - 1, 0) * ls + 64), H = 24;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" fill="#EDE6D4"/>
<rect y="2" width="${W}" height="2" fill="#1A1713"/><rect y="${H - 4}" width="${W}" height="1" fill="#1A1713"/><rect y="${H - 2}" width="${W}" height="2" fill="#1A1713"/>
<text x="14" y="16" font-family="Georgia, 'Times New Roman', serif" font-size="${fs}" font-variant="small-caps" letter-spacing="${ls}" fill="#1A1713">${esc(text)}</text>
<text x="${W - 14}" y="16" text-anchor="end" font-family="Georgia, 'Times New Roman', serif" font-size="9" fill="#8C1C13">creditia</text></svg>`;
    }
    case 'crt': {
      const pre = '> creditia  ', fs = 12;
      const W = Math.ceil(meas(pre, fs, 0.6) + meas(vtext, fs, 0.6) + 30), H = 22;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(vtext)}">
<rect width="${W}" height="${H}" fill="#04120A" stroke="#50FF8C" stroke-opacity="0.4"/>
<text x="11" y="16" font-family="Menlo, Consolas, monospace" font-size="${fs}" fill="#C6FF5C">${esc(pre)}</text>
<text x="${Math.ceil(11 + meas(pre, fs, 0.6))}" y="16" font-family="Menlo, Consolas, monospace" font-size="${fs}" fill="#8CFFB0">${esc(vtext)}</text></svg>`;
    }
    case 'arcade': {
      const text = `BUILT WITH ${list(names, ' + ', ' + ').toUpperCase() || m.project.toUpperCase()}`;
      const fs = 11, ls = 1.4;
      const W = Math.ceil(meas(text, fs, 0.72) + Math.max(text.length - 1, 0) * ls + 62), H = 26;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="${esc(text)}">
<rect width="${W}" height="${H}" fill="#120726"/>
<rect x="1" y="1" width="${W - 2}" height="${H - 2}" fill="none" stroke="#FF4FA3" stroke-width="2"/>
<text x="${W / 2}" y="18" text-anchor="middle" font-family="Menlo, Consolas, monospace" font-size="${fs}" font-weight="700" letter-spacing="${ls}" fill="#4FE3FF">${esc(text)}</text></svg>`;
    }
    case 'neon': {
      const text = (list(names, ' · ', ' · ') || m.project).toUpperCase();
      const fs = 11, ls = 3.2;
      const W = Math.ceil(meas(text, fs, 0.62) + Math.max(text.length - 1, 0) * ls + 60), H = 26;
      const mid = W / 2;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="built with ${esc(text)}">
<rect width="${W}" height="${H}" fill="#0A0710"/>
<rect x=".5" y=".5" width="${W - 1}" height="${H - 1}" rx="13" fill="none" stroke="#FF5D8F"/>
<text x="${mid}" y="17" text-anchor="middle" font-family="Georgia, serif" font-size="${fs}" letter-spacing="${ls}" fill="none" stroke="#FF5D8F" stroke-width="3" stroke-opacity="0.45">${esc(text)}</text>
<text x="${mid}" y="17" text-anchor="middle" font-family="Georgia, serif" font-size="${fs}" letter-spacing="${ls}" fill="#FFFFFF" stroke="#4FE3FF" stroke-width="0.4" stroke-opacity="0.35">${esc(text)}</text></svg>`;
    }
    case 'poster': {
      const text = (list(names, ' × ', ' × ') || m.project).toUpperCase();
      const fs = 12, ls = 1.1;
      const tw = Math.ceil(meas(text, fs, 0.72) + Math.max(text.length - 1, 0) * ls);
      const W = tw + 74, H = 28;
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="built with ${esc(text)}">
<rect width="${W}" height="${H}" fill="#F0EDE6"/>
<rect x="3" y="3" width="${W - 6}" height="${H - 6}" fill="none" stroke="#141414" stroke-width="3"/>
<rect x="3" y="3" width="30" height="22" fill="#E23B2E"/>
<text x="18" y="18" text-anchor="middle" font-family="Helvetica, Arial, sans-serif" font-size="11" font-weight="700" fill="#F0EDE6">C</text>
<text x="41" y="18.5" font-family="Helvetica, Arial, sans-serif" font-size="${fs}" font-weight="700" letter-spacing="${ls}" fill="#141414">${esc(text)}</text></svg>`;
    }
    default: {
      const fs = 11;
      const lw = meas('built with · ', fs, 0.56), vw = meas(vtext, fs, 0.56);
      const W = Math.ceil(12 + (top.length ? 18 : 0) + lw + vw + 14), H = 20;
      const bg = t.dark ? '#171716' : '#FFFFFF', bd = t.dark ? 'rgba(255,255,255,0.16)' : 'rgba(0,0,0,0.12)';
      const ink = t.dark ? '#F0EEE9' : '#161514', soft = t.dark ? '#8F8B84' : '#76726B';
      return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" role="img" aria-label="built with ${esc(vtext)}">
<rect x=".5" y=".5" width="${W - 1}" height="${H - 1}" rx="5" fill="${bg}" stroke="${bd}"/>
${top.length ? `<svg x="10" y="4" width="12" height="12" viewBox="0 0 24 24" fill="${ink}">${icon(top[0].key)}</svg>` : ''}
<text x="${10 + (top.length ? 18 : 0)}" y="14" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="${fs}" fill="${soft}">built with · </text>
<text x="${Math.ceil(10 + (top.length ? 18 : 0) + lw)}" y="14" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif" font-size="${fs}" font-weight="600" fill="${ink}">${esc(vtext)}</text></svg>`;
    }
  }
}

// ═════════════════════════ 片尾字幕 ═════════════════════════

interface RollPerson { key?: string | null; name: string; model?: string }

/** 角色块：主导 / 主笔 / 协作 / 参与（人单独一层）/ 审阅 */
function rollBlocks(m: Model): { role: string; people: RollPerson[] }[] {
  const authors = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key));
  const lead = authors[0], rest = authors.slice(1);
  return [
    m.director ? { role: '主导', people: [{ name: m.director }] } : null,
    lead ? { role: '主笔', people: [{ key: lead.key, name: lead.name, model: lead.models[0] }] } : null,
    rest.length ? { role: '协作', people: rest.map((r) => ({ key: r.key, name: r.name, model: r.models[0] })) } : null,
    m.humans.length ? { role: '参与', people: m.humans.map((r) => ({ name: r.name })) } : null,
    m.reviewers.length ? { role: '审阅', people: m.reviewers.map((r) => ({ key: r.key, name: r.name, model: r.model })) } : null,
  ].filter(Boolean) as { role: string; people: RollPerson[] }[];
}

export const ROLL_VARIANTS = ['classic', 'columns', 'billing', 'title', 'crawl', 'cast', 'scroll', 'cards', 'end', 'block', 'program'];
export const ROLL_DEFAULT: Record<string, string> = {
  terminal: 'columns', mono: 'columns', editorial: 'billing',
  cinema: 'cast', playbill: 'program', gala: 'end', broadsheet: 'columns', crt: 'scroll', arcade: 'cards', neon: 'title', poster: 'block',
};
/** 宽字体（像素、霓虹）要缩一档，否则名字会被裁 */
const rollScale = (t: Theme) => ({ arcade: 0.68, neon: 0.9, crt: 1.05 } as Record<string, number>)[t.family] ?? 1;

/** 幕次：把版本当作幕，每幕列前三位贡献者；没有版本就只有一幕 */
function actsOf(m: Model): { title: string; date?: string; names: string[] }[] {
  const tags: { tag: string; date: string }[] = m.raw.versions ?? [];
  if (!tags.length) return [{ title: '全剧', names: castOf(m, 4) }];
  const g = groupFn(m);
  const by: Record<string, Record<string, number>> = {};
  for (const b of m.raw.breakdown ?? []) {
    const k = g(b.contributor);
    for (const [v, num] of Object.entries<number>(b.byVersion ?? {})) { by[v] ??= {}; by[v][k] = (by[v][k] ?? 0) + num; }
  }
  const nm = (k: string) => m.rows.find((r) => r.key === k)?.name ?? nameOf(k);
  return tags.slice(-6).map((v) => ({
    title: v.tag, date: v.date?.replaceAll('-', '.'),
    names: Object.entries(by[v.tag] ?? {}).filter(([k]) => k !== 'unattributed' && k !== 'human' && k !== 'other').sort((a, b) => b[1] - a[1]).slice(0, 3).map(([k]) => nm(k)),
  })).filter((a) => a.names.length);
}

/** 片尾字幕：只写角色和名字。十一种版式：
 *  classic 居中堆叠 / columns 双栏 / billing 海报演职员表 / title 标题卡 / crawl 爬行定格
 *  cast 卡司表 / scroll 双栏滚屏 / cards 卡串 / end 终卡 / block 压缩 billing block / program 剧场节目单 */
export function roll(m: Model, t: Theme, variant?: string): Out {
  const v = variant ?? ROLL_DEFAULT[t.family] ?? 'classic';
  const blocks = rollBlocks(m);
  const W = 1080, H = 1350;
  const serif = t.display !== t.sans;
  const fs = rollScale(t);
  const projSize = (serif ? 96 : 72) * fs;
  const personSize = (serif ? 46 : 38) * fs;
  const person = (p: RollPerson) => box({ alignItems: 'center', gap: 16 },
    p.key ? mark(p.key, t, 30) : null,
    txt({ fontFamily: t.display, fontSize: personSize, fontWeight: t.displayWeight, color: t.ink, letterSpacing: serif ? 0 : -0.5 }, p.name),
    p.model ? txt({ fontFamily: MONO_FONT, fontSize: 16, color: t.faint, marginTop: 8 }, p.model) : null);
  const totals = [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `+${fmt(m.linesAdded)} 行`].filter(Boolean).join('  ·  ');

  if (v === 'columns') {
    const rowB = (b: { role: string; people: RollPerson[] }) => box({ width: '100%', padding: '30px 0', borderTop: `1px solid ${hair(t)}`, gap: 48 },
      box({ width: 200, justifyContent: 'flex-end' }, label(t, b.role, 13)),
      col({ flex: 1, gap: 16 }, ...b.people.map((p) => box({ alignItems: 'baseline', gap: 14 },
        txt({ fontFamily: t.display, fontSize: serif ? 34 : 28, fontWeight: t.displayWeight, color: t.ink }, p.name),
        p.model ? txt({ fontFamily: MONO_FONT, fontSize: 14, color: t.faint }, p.model) : null,
        p.key ? box({ marginLeft: 'auto' }, mark(p.key, t, 20)) : null))));
    return { w: W, h: H, tree: col({ width: W, height: H, padding: '120px 110px 90px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      label(t, 'Credits', 12),
      txt({ fontFamily: t.display, fontSize: serif ? 76 : 58, fontWeight: t.displayWeight, margin: '18px 0 30px', lineHeight: 1.05 }, m.project),
      col({ flex: 1, justifyContent: 'center' }, ...blocks.map(rowB)),
      box({ borderTop: `2px solid ${t.ink}`, paddingTop: 16 },
        txt({ fontSize: 12, color: t.soft, letterSpacing: 1.6, textTransform: 'uppercase' }, totals))) };
  }

  if (v === 'billing') {
    // 海报底部的演职员块：标题大字在上，角色小字 + 名字大字挤成一段居中的文字块
    const item = (b: { role: string; people: RollPerson[] }, p: RollPerson) => box({ alignItems: 'baseline', gap: 10, margin: '0 18px 14px' },
      label(t, b.role, 11, { color: t.soft, letterSpacing: 2.5 }),
      txt({ fontFamily: t.display, fontSize: serif ? 36 : 30, fontWeight: t.displayWeight, color: t.ink }, p.name));
    const items = blocks.flatMap((b) => b.people.map((p) => item(b, p)));
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', padding: '0 100px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      txt({ fontFamily: t.display, fontSize: serif ? 110 : 84, fontWeight: t.displayWeight, letterSpacing: serif ? 2 : -2, lineHeight: 1, textAlign: 'center' }, m.project),
      box({ width: 80, height: 2, backgroundColor: t.accent, margin: '40px 0 46px' }),
      box({ flexWrap: 'wrap', justifyContent: 'center', alignItems: 'baseline', maxWidth: 680 }, ...items),
      txt({ marginTop: 40, fontSize: 11, color: t.faint, letterSpacing: 2.6, textTransform: 'uppercase' }, totals)) };
  }

  if (v === 'title') {
    const stars = m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).slice(0, 4).map((r) => r.name);
    const namesRow = box({ alignItems: 'center', gap: 22 },
      ...stars.flatMap((name, i) => (i ? [txt({ color: t.accent, fontSize: 22 }, '·')] : []).concat(txt({ fontFamily: t.display, fontSize: serif ? 30 : 26, color: t.ink }, name))));
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 30, ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      label(t, 'creditia presents', 11, { color: t.soft, letterSpacing: 5 }),
      m.director ? label(t, `a project by ${m.director}`, 11, { color: t.soft, letterSpacing: 3 }) : null,
      txt({ fontFamily: t.display, fontSize: serif ? 128 : 96, fontWeight: t.displayWeight, letterSpacing: serif ? 2 : -2, lineHeight: 1.05, textAlign: 'center' }, m.project),
      box({ width: 90, height: 2, backgroundColor: t.accent }),
      namesRow,
      box({ marginTop: 60 }, txt({ fontSize: 11, color: t.faint, letterSpacing: 2.4, textTransform: 'uppercase' }, totals))) };
  }

  if (v === 'crawl') {
    const seq: Node[] = [label(t, m.project, 14, { color: t.soft, letterSpacing: 6 })];
    for (const b of blocks) {
      seq.push(label(t, b.role, 14, { letterSpacing: 5 }));
      seq.push(txt({ fontFamily: t.display, fontSize: serif ? 50 : 40, color: t.ink, textAlign: 'center', lineHeight: 1.2 }, b.people.map((p) => p.name).join('   ·   ')));
    }
    const n = seq.length;
    const lines = seq.map((node, i) => {
      const d = n > 1 ? Math.abs(i - (n - 1) / 2) / ((n - 1) / 2) : 0;
      const o = d <= 0.5 ? 1 : 1 - Math.pow((d - 0.5) / 0.5, 1.5) * 0.82;
      return box({ justifyContent: 'center', opacity: Math.round(o * 100) / 100 }, node);
    });
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: W, height: H, position: 'relative', justifyContent: 'center', gap: 40, padding: '0 80px' },
        ...lines,
        box({ position: 'absolute', top: 0, left: 0, width: W, height: 220, backgroundImage: `linear-gradient(to bottom, ${t.bg} 0%, ${t.bg}00 100%)` }),
        box({ position: 'absolute', bottom: 0, left: 0, width: W, height: 220, backgroundImage: `linear-gradient(to top, ${t.bg} 0%, ${t.bg}00 100%)` }))) };
  }

  if (v === 'cast') {
    // 卡司表：左列角色靠中缝右对齐，右列名字靠中缝左对齐，像 "in order of appearance"
    const rows = blocks.flatMap((b) => b.people.map((p) => ({ role: b.role, p })));
    const row = (x: { role: string; p: RollPerson }) => box({ alignItems: 'baseline', height: 60 },
      box({ width: 430, justifyContent: 'flex-end' }, txt({ fontFamily: t.labelFont, fontSize: 18, color: t.soft, letterSpacing: 3, textTransform: 'uppercase' }, x.role)),
      box({ width: 100, justifyContent: 'center' }, txt({ fontFamily: t.deco ?? t.display, fontSize: 14, color: t.accent }, t.family === 'cinema' || t.family === 'gala' ? '◆' : '·')),
      box({ width: 430, alignItems: 'baseline', gap: 12 },
        x.p.key ? mark(x.p.key, t, 22) : null,
        txt({ fontFamily: t.display, fontSize: 30 * fs, fontWeight: t.displayWeight, color: t.ink }, x.p.name),
        x.p.model ? txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, x.p.model) : null));
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', padding: '0 60px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      label(t, 'cast', 13, { letterSpacing: 8 }),
      txt({ fontFamily: t.display, fontSize: 60 * fs, fontWeight: t.displayWeight, margin: '14px 0 10px', textAlign: 'center' }, m.project),
      ornament(t, 12),
      col({ marginTop: 40 }, ...rows.map(row)),
      label(t, 'in order of appearance', 12, { marginTop: 46 })) };
  }

  if (v === 'scroll') {
    // 双栏滚屏：两列并排往上走，中缝一条线，上下渐隐
    const L: Node[] = [], R: Node[] = [];
    blocks.forEach((b, i) => {
      const node = col({ alignItems: 'center', gap: 14, marginBottom: 44 },
        label(t, b.role, 13, { letterSpacing: 5 }),
        ...b.people.map((p) => col({ alignItems: 'center', gap: 4 },
          txt({ fontFamily: t.display, fontSize: 34 * fs, fontWeight: t.displayWeight, color: t.ink, textAlign: 'center' }, p.name),
          p.model ? txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, p.model) : null)));
      (i % 2 ? R : L).push(node);
    });
    const fade = (top: boolean) => box({ position: 'absolute', [top ? 'top' : 'bottom']: 0, left: 0, width: W, height: 200, backgroundImage: `linear-gradient(to ${top ? 'bottom' : 'top'}, ${t.bg} 0%, ${t.bg}00 100%)` });
    return { w: W, h: H, tree: box({ width: W, height: H, position: 'relative', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      col({ position: 'absolute', top: 0, left: 0, width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 36, padding: '0 70px' },
        txt({ fontFamily: t.display, fontSize: 48 * fs, fontWeight: t.displayWeight, textAlign: 'center' }, m.project),
        box({ width: '100%', justifyContent: 'center', alignItems: 'flex-start' },
          col({ flex: 1, alignItems: 'center' }, ...L),
          box({ width: 1, alignSelf: 'stretch', backgroundColor: hair(t), margin: '0 10px' }),
          col({ flex: 1, alignItems: 'center' }, ...R))),
      fade(true), fade(false)) };
  }

  if (v === 'cards') {
    // 卡串：每个角色一张卡，像片尾一张张切过去的卡
    const rad = Math.min(t.radius, 14);
    const card = (b: { role: string; people: RollPerson[] }) => col({ width: 440, padding: '26px 30px', ...surface(t, { borderRadius: rad }), gap: 14 },
      label(t, b.role, 12, { letterSpacing: 4 }),
      ...b.people.map((p) => box({ alignItems: 'baseline', gap: 12, flexWrap: 'wrap' },
        p.key ? mark(p.key, t, 24) : null,
        txt({ fontFamily: t.display, fontSize: 32 * fs, fontWeight: t.displayWeight, color: t.ink }, p.name),
        p.model ? txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, p.model) : null)));
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 24, padding: '0 80px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      col({ width: 904, padding: '34px 40px', ...surface(t, { borderRadius: rad }), alignItems: 'center', gap: 10 },
        label(t, 'credits', 12, { letterSpacing: 6 }),
        txt({ fontFamily: t.display, fontSize: 56 * fs, fontWeight: t.displayWeight, textAlign: 'center' }, m.project)),
      box({ flexWrap: 'wrap', gap: 24, justifyContent: 'center', width: 904 }, ...blocks.map(card)),
      label(t, totals, 11, { marginTop: 8 })) };
  }

  if (v === 'end') {
    // 终卡：The End，底下一行 with 谁谁谁
    const names = castOf(m, 5);
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 30, padding: '0 90px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      label(t, '剧终', 13, { letterSpacing: 10 }),
      txt({ fontFamily: t.display, fontSize: 150 * fs, fontWeight: t.displayWeight, lineHeight: 1, textAlign: 'center', letterSpacing: serif ? 2 : -3 }, 'The End'),
      ornament(t, 14),
      txt({ fontFamily: t.display, fontSize: 40 * fs, textAlign: 'center' }, m.project),
      m.director ? label(t, `a project by ${m.director}`, 13, { letterSpacing: 4 }) : null,
      names.length ? txt({ fontSize: 18, color: t.soft, textAlign: 'center', marginTop: 6 }, `with ${names.join(' · ')}`) : null,
      label(t, totals, 11, { marginTop: 50, color: t.faint })) };
  }

  if (v === 'block') {
    // 压缩 billing block：海报底部那坨又高又窄的大写字（行规：名字至少是片名的 15%，角色不小于名字一半）
    const COND = 'Oswald, Noto Sans SC';
    const seg = (role: string, names: string[]) => box({ alignItems: 'baseline', gap: 7, margin: '0 13px' },
      role ? txt({ fontFamily: COND, fontSize: 12, color: t.soft, letterSpacing: 1.6, textTransform: 'uppercase' }, role) : null,
      names.length ? txt({ fontFamily: COND, fontSize: 27, fontWeight: 600, color: t.ink, letterSpacing: 0.6, textTransform: 'uppercase' }, names.join('   ')) : null);
    return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 40, padding: '80px 70px', ...backdrop(t, keysOf(m)), color: t.ink },
      txt({ fontFamily: t.display, fontSize: 104 * fs, fontWeight: t.displayWeight, lineHeight: 1.08, textAlign: 'center' }, m.project),
      col({ alignItems: 'center', gap: 8, marginTop: 6 },
        txt({ fontFamily: COND, fontSize: 12, color: t.faint, letterSpacing: 4, textTransform: 'uppercase' }, 'creditia presents'),
        m.director ? txt({ fontFamily: COND, fontSize: 15, color: t.soft, letterSpacing: 2, textTransform: 'uppercase' }, `a ${m.director} production`) : null,
        ornament(t, 13)),
      box({ flexWrap: 'wrap', justifyContent: 'center', alignItems: 'baseline', width: 940, rowGap: 10 }, ...blocks.map((b) => seg(b.role, b.people.map((p) => p.name)))),
      txt({ fontFamily: COND, fontSize: 12, color: t.faint, letterSpacing: 2.4, textTransform: 'uppercase', marginTop: 4 }, totals)) };
  }

  if (v === 'program') {
    // 剧场节目单：版本当作幕，每幕写谁出场
    const acts = actsOf(m);
    const roman = ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII'];
    const act = (a: { title: string; date?: string; names: string[] }, i: number) => col({ gap: 8, padding: '18px 0', borderTop: `1px solid ${hair(t)}` },
      box({ alignItems: 'baseline', justifyContent: 'space-between' },
        label(t, `Act ${roman[i] ?? i + 1} · ${a.title}`, 13, { letterSpacing: 3 }),
        a.date ? txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, a.date) : null),
      txt({ fontFamily: t.display, fontSize: 28 * fs, fontWeight: t.displayWeight, color: t.ink }, a.names.join('  ·  ')));
    return { w: W, h: H, tree: col({ width: W, height: H, padding: '90px 100px 80px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
      col({ alignItems: 'center', gap: 10 },
        label(t, 'programme 节目单', 13, { letterSpacing: 6 }),
        txt({ fontFamily: t.display, fontSize: 64 * fs, fontWeight: t.displayWeight, textAlign: 'center' }, m.project),
        m.director ? txt({ fontSize: 16, color: t.soft }, `导演 ${m.director}`) : null,
        ornament(t, 12, { marginTop: 6 })),
      col({ marginTop: 40, flexGrow: 1, justifyContent: 'center' }, ...acts.map(act), box({ borderTop: `1px solid ${hair(t)}` })),
      col({ alignItems: 'center', gap: 8, marginTop: 30 },
        label(t, 'cast 演职员', 11, { letterSpacing: 4 }),
        txt({ fontSize: 16, color: t.soft, textAlign: 'center', lineHeight: 1.6 }, blocks.map((b) => `${b.role} ${b.people.map((p) => p.name).join('、')}`).join('   ·   ')))) };
  }

  // classic：居中堆叠
  const role = (name: string, ...people: Node[]) => col({ alignItems: 'center', gap: 18 }, label(t, name, 15, { letterSpacing: 6 }), ...people);
  const body = blocks.map((b) => role(b.role, box({ gap: 44, flexWrap: 'wrap', justifyContent: 'center' }, ...b.people.map(person))));
  return { w: W, h: H, tree: col({ width: W, height: H, alignItems: 'center', justifyContent: 'center', gap: 58, ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    txt({ fontFamily: t.display, fontSize: projSize, fontWeight: t.displayWeight, letterSpacing: serif ? -1 : -2, marginBottom: 6 }, m.project),
    txt({ fontSize: 12, color: t.accent, letterSpacing: 6 }, '· · · · ·'),
    ...body) };
}

// ═════════════════════════ 精力小票 ═════════════════════════

export const RECEIPT_VARIANTS = ['slip', 'thermal', 'folio', 'stub', 'ticket', 'ledger', 'scroll'];
export const RECEIPT_DEFAULT: Record<string, string> = {
  terminal: 'thermal', mono: 'thermal', editorial: 'folio', film: 'stub',
  cinema: 'ticket', playbill: 'ticket', gala: 'folio', broadsheet: 'ledger', crt: 'scroll', arcade: 'scroll', neon: 'slip', poster: 'ledger',
};

/** 精力小票：只回答"花了多少精力"。四种版式：清单 / 热敏纸 / 财务单 / 电影票 */
export function receipt(m: Model, t: Theme, variant?: string): Out {
  const v = variant ?? RECEIPT_DEFAULT[t.family] ?? 'slip';
  const W = 1080, H = 1350;
  const items: [string, string, string?][] = [];
  if (m.spanDays) items.push(['工期', `${m.spanDays} 天`, `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}`]);
  if (m.activeDays) items.push(['动工', `${m.activeDays} 天`]);
  if (m.sessions) items.push(['会话', fmt(m.sessions)]);
  if (m.tokens) items.push(['Token', compact(m.tokens)]);
  if (m.commits) items.push(['提交', fmt(m.commits)]);
  if (m.linesAdded) items.push(['写下', `${fmt(m.linesAdded)} 行`]);
  if (m.linesSurviving) items.push(['留下', `${fmt(m.linesSurviving)} 行`]);
  if (m.director) items.push(['主导', m.director]);

  if (v === 'thermal') {
    const paper = '#FCFBF6', ink = '#1A1713';
    const dot = (k: string, val: string) => {
      const n = Math.max(3, 33 - [...k].reduce((a, c) => a + (/[一-鿿]/.test(c) ? 2 : 1), 0) - [...val].length);
      return box({ justifyContent: 'space-between' },
        txt({ fontFamily: MONO_FONT, fontSize: 21, color: ink }, k),
        txt({ fontFamily: MONO_FONT, fontSize: 21, color: ink, whiteSpace: 'pre' }, `${'.'.repeat(n)} ${val}`));
    };
    const bars = Array.from({ length: 42 }, (_, i) => { const hsh = (m.project.charCodeAt(i % Math.max(m.project.length, 1)) * 31 + i * 7) % 10; return box({ width: 1 + (hsh % 3), height: 54, backgroundColor: ink, marginRight: 2 }); });
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: 640, padding: '70px 58px', backgroundColor: paper, color: ink, fontFamily: MONO_FONT, boxShadow: '0 30px 80px rgba(0,0,0,0.45)' },
        col({ alignItems: 'center', gap: 10 },
          txt({ fontFamily: MONO_FONT, fontSize: 13, letterSpacing: 4, color: ink }, 'C R E D I T I A'),
          txt({ fontFamily: MONO_FONT, fontSize: 40, fontWeight: 700, color: ink, textAlign: 'center' }, m.project)),
        txt({ fontFamily: MONO_FONT, fontSize: 18, color: ink, margin: '26px 0 10px', whiteSpace: 'pre' }, '- '.repeat(19) + '-'),
        col({ gap: 12 }, ...items.map(([k, val]) => dot(k, val))),
        txt({ fontFamily: MONO_FONT, fontSize: 18, color: ink, margin: '10px 0', whiteSpace: 'pre' }, '- '.repeat(19) + '-'),
        dot('合计', m.linesAdded ? `+${fmt(m.linesAdded)}` : `${fmt(m.commits)}`),
        col({ alignItems: 'center', gap: 8, marginTop: 30 },
          box({ marginTop: 4 }, ...bars),
          txt({ fontFamily: MONO_FONT, fontSize: 15, color: ink, letterSpacing: 2 }, `NO.${String(m.commits % 10000).padStart(4, '0')}`),
          txt({ fontFamily: MONO_FONT, fontSize: 13, color: ink, marginTop: 8 }, 'THANK YOU · 请保留小票')))) };
  }

  if (v === 'folio') {
    const leader = box({ flex: 1, margin: '0 12px', ...dotline(t) });
    const row = (k: string, val: string, sub?: string) => box({ alignItems: 'baseline', padding: '13px 0', borderBottom: `1px solid ${hair(t)}` },
      txt({ fontSize: 17, color: t.soft }, k), leader,
      col({ alignItems: 'flex-end' },
        txt({ fontFamily: t.num, fontSize: 26, color: t.ink }, val),
        sub ? txt({ fontFamily: MONO_FONT, fontSize: 12, color: t.faint, marginTop: 3 }, sub) : null));
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: 680, padding: '64px 60px', ...surface(t, { borderRadius: 0, border: `2px solid ${t.ink}` }), color: t.ink, fontFamily: t.sans },
        box({ alignItems: 'baseline', justifyContent: 'space-between' },
          box({ alignItems: 'center', gap: 10 }, box({ width: 9, height: 9, backgroundColor: t.accent }), label(t, 'Folio', 12)),
          txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.soft }, m.range.from.replaceAll('-', '.'))),
        txt({ fontFamily: t.display, fontSize: 52, fontWeight: t.displayWeight, margin: '16px 0 6px' }, m.project),
        txt({ fontSize: 13, color: t.soft, marginBottom: 18 }, '工作记录 · 精力清单'),
        box({ borderTop: `3px solid ${t.ink}` }),
        col({}, ...items.map(([k, val, sub]) => row(k, val, sub))),
        box({ marginTop: 6, borderTop: `1px solid ${hair(t)}`, paddingTop: 14, alignItems: 'baseline', justifyContent: 'space-between' },
          label(t, 'Total', 12),
          txt({ fontFamily: t.num, fontSize: 30, color: t.ink }, m.linesSurviving ? `${fmt(m.linesSurviving)} 行至今仍在` : `${fmt(m.linesAdded)} 行`)),
        txt({ marginTop: 26, fontSize: 12, color: t.faint, letterSpacing: 1.4, textTransform: 'uppercase' }, `由 creditia 开具 · ${m.range.to}`))) };
  }

  if (v === 'stub') {
    const paper = '#EDE6D8', pInk = '#12110F';
    const row = (k: string, val: string) => box({ alignItems: 'baseline', padding: '9px 0', borderBottom: '1px dashed rgba(18,17,16,0.28)' },
      txt({ fontFamily: t.sans, fontSize: 15, color: 'rgba(18,17,16,0.62)' }, k),
      box({ flex: 1, margin: '0 10px', ...dotline(t, 'rgba(18,17,16,0.4)') }),
      txt({ fontFamily: MONO_FONT, fontSize: 17, fontWeight: 700, color: pInk }, val));
    const half = Math.ceil(items.length / 2);
    const colOf = (its: [string, string, string?][]) => col({ flex: 1 }, ...its.map(([k, val]) => row(k, val)));
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      box({ width: 1000, height: 560, backgroundColor: paper, color: pInk, fontFamily: t.sans, borderRadius: 4, overflow: 'hidden' },
        col({ flex: 1, padding: '44px 48px' },
          box({ alignItems: 'baseline', justifyContent: 'space-between' },
            label(t, 'Creditia · 精力小票', 11, { color: 'rgba(18,17,16,0.55)', letterSpacing: 3 }),
            txt({ fontFamily: MONO_FONT, fontSize: 13, color: 'rgba(18,17,16,0.55)' }, `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}`)),
          txt({ fontFamily: t.display, fontSize: 62, color: pInk, marginTop: 12 }, m.project),
          box({ marginTop: 18, gap: 56 }, colOf(items.slice(0, half)), colOf(items.slice(half)))),
        col({ width: 240, borderLeft: '2px dashed rgba(18,17,16,0.4)', alignItems: 'center', justifyContent: 'center', gap: 12 },
          txt({ fontFamily: t.sans, fontSize: 11, letterSpacing: 4, color: pInk }, 'ADMIT ONE'),
          txt({ fontFamily: t.display, fontSize: 74, color: pInk }, compact(m.linesAdded)),
          txt({ fontFamily: t.sans, fontSize: 11, letterSpacing: 2, color: 'rgba(18,17,16,0.6)', textTransform: 'uppercase' }, '行'),
          box({ marginTop: 14, width: 90, height: 1, backgroundColor: 'rgba(18,17,16,0.3)' }),
          txt({ fontFamily: MONO_FONT, fontSize: 12, color: pInk }, `no. ${String(m.commits % 1000).padStart(3, '0')}`)))) };
  }

  if (v === 'ticket') {
    // 电影票：主票面 + 撕线 + 存根，左侧一道红边
    const dark = t.dark;
    const paper = dark ? '#141210' : '#F7F0E1';
    const pInk = dark ? '#EDE6D8' : '#1A1713';
    const red = dark ? t.accent : '#8C1C13';
    const dim = (a: string) => hexA(pInk, a);
    const row = (k: string, val: string) => box({ alignItems: 'baseline', padding: '9px 0' },
      txt({ fontSize: 16, color: dim('99') }, k),
      box({ flex: 1, margin: '0 10px', ...dotline(t, dim('44')) }),
      txt({ fontFamily: MONO_FONT, fontSize: 19, color: pInk }, val));
    const mid = Math.ceil(items.length / 2);
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: 720, backgroundColor: paper, color: pInk, borderLeft: `16px solid ${red}`, boxShadow: '0 30px 80px rgba(0,0,0,0.45)' },
        col({ padding: '46px 52px 34px', gap: 16 },
          box({ justifyContent: 'space-between', alignItems: 'baseline' },
            txt({ fontFamily: t.labelFont, fontSize: 12, letterSpacing: 5, color: red }, 'CREDITIA CINEMA'),
            txt({ fontFamily: MONO_FONT, fontSize: 13, color: dim('77') }, `NO.${String(m.commits % 100000).padStart(5, '0')}`)),
          txt({ fontFamily: t.display, fontSize: 62, color: pInk, lineHeight: 1.05 }, m.project),
          box({ alignItems: 'baseline', gap: 16, marginTop: 6 },
            txt({ fontFamily: t.display, fontSize: 30, color: red, letterSpacing: 2 }, 'ADMIT ONE'),
            txt({ fontFamily: t.num, fontSize: 46, color: pInk }, compact(m.linesAdded || m.commits)),
            txt({ fontSize: 14, color: dim('88') }, m.linesAdded ? '行' : '次提交')),
          m.director ? txt({ fontSize: 14, color: dim('88') }, `主导 ${m.director}`) : null),
        box({ borderTop: `2px dashed ${dim('55')}` }),
        box({ padding: '24px 52px 40px', gap: 60 },
          col({ flex: 1 }, ...items.slice(0, mid).map(([k, val]) => row(k, val))),
          col({ flex: 1 }, ...items.slice(mid).map(([k, val]) => row(k, val)))))) };
  }

  if (v === 'ledger') {
    // 账簿：横格纸、红竖线、合计双下划线
    const pInk = '#20242E';
    const paper = '#F7F3E8';
    const rules = 'repeating-linear-gradient(0deg, transparent 0px, transparent 45px, rgba(32,36,46,0.13) 45px, rgba(32,36,46,0.13) 46px)';
    const row = (k: string, val: string) => col({ height: 46, justifyContent: 'center' },
      box({ alignItems: 'baseline' },
        txt({ fontSize: 17, color: 'rgba(32,36,46,0.72)' }, k),
        box({ flex: 1 }),
        txt({ fontFamily: MONO_FONT, fontSize: 20, color: pInk }, val)));
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: 760, backgroundColor: paper, color: pInk, boxShadow: '0 30px 80px rgba(0,0,0,0.45)', border: '1px solid rgba(32,36,46,0.25)', backgroundImage: rules, borderLeft: '3px solid #A8352A' },
        col({ padding: '40px 44px 10px', gap: 8 },
          box({ justifyContent: 'space-between', alignItems: 'baseline' },
            txt({ fontFamily: t.labelFont, fontSize: 12, letterSpacing: 4, color: '#A8352A', textTransform: 'uppercase' }, 'ledger 账簿'),
            txt({ fontFamily: MONO_FONT, fontSize: 12, color: 'rgba(32,36,46,0.6)' }, `no.${String(m.commits % 10000).padStart(4, '0')}`)),
          txt({ fontFamily: t.display, fontSize: 50, color: pInk }, m.project)),
        col({ padding: '0 44px 32px' },
          box({ borderTop: `2px solid ${pInk}`, marginBottom: 6 }),
          ...items.map(([k, val]) => row(k, val)),
          box({ borderTop: `2px solid ${pInk}`, marginTop: 6, paddingTop: 12, alignItems: 'baseline', justifyContent: 'space-between' },
            txt({ fontSize: 14, letterSpacing: 2, textTransform: 'uppercase', color: 'rgba(32,36,46,0.7)' }, 'Total 合计'),
            txt({ fontFamily: t.num, fontSize: 26, color: pInk }, `${fmt(m.linesSurviving || m.linesAdded)} 行`)),
          box({ height: 0, borderTop: `1px solid ${pInk}`, marginTop: 5 }),
          txt({ marginTop: 20, fontSize: 12, color: 'rgba(32,36,46,0.5)', letterSpacing: 1.2, textTransform: 'uppercase' }, `creditia 记 · ${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}`)))) };
  }

  if (v === 'scroll') {
    // 长卷纸：窄窄一条拖到底，横线是纸上的格
    const paper = '#FCFBF6', ink = '#1A1713';
    const dark = t.dark;
    const pInk = dark ? '#EDE6D8' : '#1A1713';
    const bg = dark ? '#12110F' : paper;
    const fg = dark ? pInk : ink;
    const row = (k: string, val: string) => box({ justifyContent: 'space-between', padding: '11px 0' },
      txt({ fontFamily: MONO_FONT, fontSize: 19, color: hexA(dark ? fg : ink, 'CC') }, k),
      txt({ fontFamily: MONO_FONT, fontSize: 19, color: fg, whiteSpace: 'pre' }, val));
    const barcode = Array.from({ length: 46 }, (_, i) => { const h = (m.project.charCodeAt(i % Math.max(m.project.length, 1)) * 37 + i * 11) % 9; return box({ width: 1 + (h % 3), height: 60, backgroundColor: fg, marginRight: 2 }); });
    return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)) },
      col({ width: 560, height: 1240, backgroundColor: bg, color: fg, fontFamily: MONO_FONT, boxShadow: dark ? '0 30px 80px rgba(0,0,0,0.5)' : '0 30px 80px rgba(0,0,0,0.35)', padding: '60px 56px' },
        col({ alignItems: 'center', gap: 8 },
          txt({ fontFamily: MONO_FONT, fontSize: 13, letterSpacing: 6, color: fg }, 'C R E D I T I A'),
          txt({ fontFamily: t.labelFont, fontSize: 11, letterSpacing: 3, color: hexA(fg, 'AA') }, 'ENERGY RECEIPT 精力小票')),
        txt({ fontFamily: t.display, fontSize: 46, color: fg, margin: '22px 0 6px', textAlign: 'center' }, m.project),
        txt({ fontFamily: MONO_FONT, fontSize: 15, color: hexA(fg, '88'), textAlign: 'center' }, `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}`),
        txt({ fontFamily: MONO_FONT, fontSize: 17, color: hexA(fg, '66'), whiteSpace: 'pre', marginTop: 18 }, '· '.repeat(30)),
        col({ flexGrow: 1, marginTop: 6 }, ...items.map(([k, val]) => row(k, val))),
        box({ borderTop: `2px solid ${hexA(fg, '66')}`, paddingTop: 12, justifyContent: 'space-between' },
          txt({ fontFamily: MONO_FONT, fontSize: 20, color: fg, fontWeight: 700 }, '合计 TOTAL'),
          txt({ fontFamily: MONO_FONT, fontSize: 20, color: fg }, m.linesAdded ? `+${fmt(m.linesAdded)}` : `${fmt(m.commits)}`)),
        col({ alignItems: 'center', gap: 8, marginTop: 26 },
          box({ alignItems: 'flex-end' }, ...barcode),
          txt({ fontFamily: MONO_FONT, fontSize: 13, color: hexA(fg, '99'), letterSpacing: 4 }, `NO.${String(m.commits % 100000).padStart(5, '0')}`),
          txt({ fontFamily: t.labelFont, fontSize: 11, color: hexA(fg, '77'), letterSpacing: 2, marginTop: 4 }, '由 creditia 开具')))) };
  }

  // slip：干净的纸质清单
  const rule = box({ height: 0, borderTop: `1.5px dashed ${t.line}`, margin: '8px 0' });
  const paper = col({ width: 620, padding: '64px 56px 56px', ...surface(t, { borderRadius: Math.min(t.radius, 10) }), gap: 0 },
    col({ alignItems: 'center', gap: 14, marginBottom: 40 },
      label(t, 'Receipt', 13),
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 44 : 54, fontWeight: t.displayWeight, color: t.ink }, m.project)),
    rule,
    col({ padding: '20px 0' },
      ...items.map(([k, val, sub]) => col({ padding: '13px 0' },
        box({ justifyContent: 'space-between', alignItems: 'baseline' },
          txt({ fontSize: 20, color: t.soft }, k),
          txt({ fontSize: 26, fontFamily: t.num === t.sans ? MONO_FONT : t.num, color: t.ink }, val)),
        sub ? txt({ alignSelf: 'flex-end', fontSize: 15, color: t.faint, marginTop: 6, fontFamily: MONO_FONT }, sub) : null))),
    rule,
    box({ justifyContent: 'center', marginTop: 28, gap: 14 }, ...m.rows.filter((r) => r.kind === 'agent' && isAi(r.key)).map((r) => mark(r.key, t, 22, t.soft))));
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans }, paper) };
}

// ═════════════════════════ 终端截图 ═════════════════════════

export interface Seg { text: string; color?: string; rank?: number; dim?: boolean; bold?: boolean }

/** 终端截图：fetch 和 blame 共用。按家族换窗口样式 */
export function term(cmd: string, lines: Seg[][], t: Theme, m?: Model): Out {
  const W = 1200, lh = 32, H = 64 + 44 + 32 + (lines.length + 1) * lh + 40 + 64;
  const dark = t.dark || t.family === 'glass';
  const fg = dark ? (t.dark ? t.ink : '#E7E9EE') : '#161514';
  const soft = dark ? (t.dark ? t.soft : '#8A909C') : '#76726B';
  const tt: Theme = { ...t, dark };
  const chrome = (() => {
    switch (t.family) {
      case 'terminal':
        return box({ height: 40, alignItems: 'center', padding: '0 22px', gap: 8, borderBottom: `1px solid ${t.edge}` },
          txt({ fontFamily: MONO_FONT, fontSize: 14, color: t.accent }, '❯'),
          txt({ fontFamily: MONO_FONT, fontSize: 14, color: fg }, cmd));
      case 'editorial':
        return box({ height: 44, alignItems: 'center', padding: '0 22px', justifyContent: 'space-between', borderBottom: `2px solid ${t.ink}` },
          label(t, 'Creditia — 命令行', 11),
          txt({ fontFamily: MONO_FONT, fontSize: 12, color: t.soft }, 'terminal'));
      case 'mono':
        return box({ height: 40, alignItems: 'center', padding: '0 22px', backgroundColor: '#000000' },
          txt({ fontFamily: MONO_FONT, fontSize: 13, fontWeight: 700, letterSpacing: 1.4, color: '#FFFFFF', textTransform: 'uppercase' }, `❯ ${cmd}`));
      default:
        return box({ height: 44, alignItems: 'center', padding: '0 18px', gap: 8 },
          ...['#FF5F57', '#FEBC2E', '#28C840'].map(() => box({ width: 12, height: 12, borderRadius: 99, backgroundColor: dark ? 'rgba(255,255,255,0.14)' : 'rgba(0,0,0,0.12)' })));
    }
  })();
  const winBorder = t.family === 'mono' ? '2px solid #000000' : t.family === 'editorial' ? `2px solid ${t.ink}` : `1px solid ${dark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}`;
  const winBg = t.family === 'editorial' ? t.bg : t.family === 'mono' ? '#FFFFFF' : dark ? (t.dark ? t.surface : '#15171C') : '#FFFFFF';
  return { w: W, h: H, tree: box({ width: W, height: H, alignItems: 'center', justifyContent: 'center', ...backdrop(t, m ? keysOf(m) : []) },
    col({ width: W - 160, backgroundColor: winBg, borderRadius: t.family === 'mono' || t.family === 'editorial' ? 0 : Math.min(t.radius, 12), border: winBorder, boxShadow: t.shadow === 'none' ? undefined : t.shadow, fontFamily: MONO_FONT, overflow: 'hidden' },
      chrome,
      col({ padding: '16px 36px 40px', fontSize: 19, color: fg },
        t.family === 'terminal' || t.family === 'mono' ? null : box({ height: lh, alignItems: 'center' }, txt({ color: soft, whiteSpace: 'pre' }, '❯ '), txt({}, cmd)),
        ...lines.map((ln) => box({ height: lh, alignItems: 'center' },
          ...ln.map((s) => txt({ whiteSpace: 'pre', color: s.color ? famColor(s.color, tt, s.rank ?? 0) : s.dim ? soft : fg, fontWeight: s.bold ? 700 : 400 }, s.text || ' '))))))) };
}

// ═════════════════════════ App 内致谢页 ═════════════════════════

/** App 内致谢页（iOS 设置页样式） */
export function appPage(m: Model, t: Theme): Out {
  const s = 2, W = 390 * s;
  const rows = m.rows.filter((r) => r.key !== 'human');
  const H = (54 + 44 + 20 + (m.director ? 92 : 0) + 30 + rows.length * 50 + (m.humans.length ? 40 : 0) + 40 + 40) * s;
  const ios = t.dark ? { bg: '#000000', cell: '#1C1C1E', sep: 'rgba(84,84,88,0.6)', blue: '#0A84FF' } : { bg: '#F2F2F7', cell: '#FFFFFF', sep: 'rgba(60,60,67,0.18)', blue: '#007AFF' };
  const head = (s2: string) => txt({ fontSize: 13 * s, color: t.soft, margin: `${22 * s}px ${16 * s}px ${7 * s}px`, letterSpacing: 0.4 }, s2);
  const cellRow = (r: Contributor, border: boolean) => box({ alignItems: 'center', gap: 12 * s, height: 50 * s, paddingRight: 16 * s, borderBottom: border ? `${0.5 * s}px solid ${ios.sep}` : 'none' },
    mark(r.key, { ...t, ink: t.dark ? '#FFFFFF' : '#000000' } as Theme, 22 * s),
    txt({ fontSize: 17 * s, color: r.key === 'unattributed' ? t.soft : t.dark ? '#FFFFFF' : '#000000' }, r.name),
    txt({ marginLeft: 'auto', fontSize: 17 * s, color: t.soft }, pct(r.pct)));
  return { w: W, h: H, tree: col({ width: W, height: H, backgroundColor: ios.bg, color: t.ink, fontFamily: 'Inter, Noto Sans SC', padding: `${54 * s}px ${16 * s}px 0` },
    box({ height: 44 * s, alignItems: 'center', position: 'relative' },
      txt({ fontSize: 17 * s, color: ios.blue }, '‹ 关于'),
      txt({ position: 'absolute', left: 0, right: 0, justifyContent: 'center', fontSize: 17 * s, fontWeight: 600 }, '致谢')),
    m.director ? head('主导') : null,
    m.director ? box({ backgroundColor: ios.cell, borderRadius: 10 * s, padding: `${13 * s}px ${16 * s}px` }, txt({ fontSize: 17 * s }, m.director)) : null,
    head(m.project),
    col({ backgroundColor: ios.cell, borderRadius: 10 * s, paddingLeft: 16 * s },
      ...rows.map((r, i) => cellRow(r, i < rows.length - 1))),
    m.humans.length ? box({ margin: `${8 * s}px ${16 * s}px 0`, fontSize: 13 * s, color: t.soft }, `人 · ${m.humans.map((x) => `${x.name} ${pct(x.pct)}`).join('　')}`) : null,
    txt({ fontSize: 13 * s, color: t.soft, margin: `${8 * s}px ${16 * s}px` }, `${fmt(m.commits)} 次提交 · ${fmt(m.linesAdded)} 行`)) };
}

// ═════════════════════════ 统计图 ═════════════════════════

/** 统计图通用框：标题、图例按家族换排版 */
function frame(m: Model, t: Theme, W: number, H: number, caption: string, body: Node, legend = true): Out {
  const leg = legend
    ? box({ gap: 26, alignItems: 'center', flexWrap: 'wrap' },
        ...m.rows.map((r, i) => box({ alignItems: 'center', gap: 9 },
          box({ width: 10, height: 10, borderRadius: t.family === 'glass' ? 4 : t.barRadius ? 3 : 0, ...fill(r, t, i, 2) }),
          txt({ fontSize: 15, color: r.key === 'unattributed' ? t.soft : t.ink }, r.name))))
    : null;
  if (t.family === 'mono') return { w: W, h: H, tree: col({ width: W, height: H, backgroundColor: t.bg, color: t.ink, fontFamily: t.sans },
    box({ height: 56, backgroundColor: '#000000', alignItems: 'center', padding: '0 28px', justifyContent: 'space-between', flexShrink: 0 },
      txt({ color: '#FFFFFF', fontSize: 19, fontWeight: 700, letterSpacing: 1.1, textTransform: 'uppercase' }, m.project),
      txt({ color: 'rgba(255,255,255,0.66)', fontSize: 11, letterSpacing: 2, textTransform: 'uppercase' }, caption)),
    col({ flex: 1, padding: '40px 56px 24px' }, body),
    leg ? box({ padding: '0 56px 34px' }, leg) : null) };
  const head = (() => {
    switch (t.family) {
      case 'cinema': return col({ alignItems: 'center', gap: 10, marginBottom: 34 },
        txt({ fontFamily: t.deco, fontSize: 46, color: t.ink }, m.project),
        txt({ fontFamily: t.labelFont, fontSize: 11, color: t.soft, letterSpacing: 4, textTransform: 'uppercase' }, caption),
        ornament(t, 11));
      case 'playbill': return col({ marginBottom: 32, gap: 10 },
        box({ alignItems: 'center', gap: 14 },
          box({ backgroundColor: t.accent, padding: '3px 12px' }, txt({ fontFamily: t.labelFont, fontSize: 11, color: t.surface, letterSpacing: 3 }, caption.toUpperCase())),
          txt({ fontFamily: t.display, fontSize: 40, fontWeight: 900, color: t.ink }, m.project)),
        box({ borderTop: `3px solid ${t.ink}`, borderBottom: `1px solid ${t.ink}`, height: 4 }));
      case 'gala': return col({ alignItems: 'center', gap: 10, marginBottom: 34 },
        txt({ fontFamily: t.labelFont, fontSize: 11, color: t.accent, letterSpacing: 7, textTransform: 'uppercase' }, caption),
        txt({ fontFamily: t.display, fontSize: 44, color: t.ink, textAlign: 'center' }, m.project),
        box({ width: 300, height: 1, backgroundColor: t.accent, opacity: 0.6 }));
      case 'broadsheet': return col({ marginBottom: 28, gap: 8 },
        box({ justifyContent: 'space-between', alignItems: 'baseline' }, label(t, 'creditia daily', 12), label(t, caption, 12)),
        box({ borderTop: `2px solid ${t.ink}`, borderBottom: `1px solid ${t.ink}`, padding: '8px 0 6px' },
          txt({ fontFamily: t.deco, fontSize: 50, color: t.ink, lineHeight: 1 } , m.project)),
        box({ borderBottom: `5px solid ${t.ink}` }));
      case 'crt': return col({ marginBottom: 34, gap: 6 },
        txt({ fontFamily: t.sans, fontSize: 22, color: t.ink }, `> creditia stats ${m.project}  # ${caption}`),
        txt({ fontFamily: t.sans, fontSize: 20, color: t.faint, whiteSpace: 'pre' }, '─'.repeat(52)));
      case 'arcade': return col({ alignItems: 'center', gap: 10, marginBottom: 32 },
        txt({ fontFamily: t.display, fontSize: 20, color: t.accent, letterSpacing: 2, lineHeight: 1.7 }, caption === '' ? 'HIGH SCORES' : caption.toUpperCase()),
        txt({ fontFamily: t.sans, fontSize: 34, color: t.ink }, m.project));
      case 'neon': return col({ alignItems: 'center', gap: 14, marginBottom: 34 },
        box({ position: 'relative', alignItems: 'center', justifyContent: 'center' },
          txt({ position: 'absolute', fontFamily: t.display, fontSize: 44, color: t.accent, opacity: 0.55 }, m.project),
          txt({ fontFamily: t.display, fontSize: 44, color: t.ink }, m.project)),
        box({ padding: '3px 14px', border: `1px solid ${t.accent}`, borderRadius: 999 }, txt({ fontFamily: t.sans, fontSize: 12, color: t.accent, letterSpacing: 3 }, caption)));
      case 'poster': return col({ marginBottom: 30, gap: 8 },
        box({ alignItems: 'center', gap: 12 },
          box({ width: 14, height: 14, backgroundColor: t.accent }),
          txt({ fontFamily: t.display, fontSize: 42, color: t.ink, textTransform: 'uppercase', lineHeight: 1 }, m.project)),
        box({ height: 5, backgroundColor: t.ink }),
        label(t, caption, 12));
      case 'film': return col({ alignItems: 'center', gap: 10, marginBottom: 34 },
        txt({ fontFamily: t.display, fontSize: 42, color: t.ink }, m.project),
        txt({ fontSize: 11, color: t.soft, letterSpacing: 4, textTransform: 'uppercase' }, caption));
      case 'terminal': return box({ alignItems: 'baseline', gap: 12, marginBottom: 40 },
        txt({ fontFamily: MONO_FONT, fontSize: 18, color: t.accent }, '❯'),
        txt({ fontFamily: MONO_FONT, fontSize: 18, color: t.ink }, `creditia stats ${m.project}`),
        txt({ fontFamily: MONO_FONT, fontSize: 15, color: t.faint }, `# ${caption}`));
      case 'editorial': return col({ marginBottom: 30 },
        box({ borderTop: `3px solid ${t.ink}`, paddingTop: 12, alignItems: 'baseline', justifyContent: 'space-between' },
          txt({ fontFamily: t.display, fontSize: 44, fontWeight: t.displayWeight, color: t.ink }, m.project),
          label(t, caption, 12)),
        box({ marginTop: 12, height: 2, backgroundColor: t.ink }));
      default: return box({ alignItems: 'baseline', justifyContent: 'space-between', marginBottom: 44 },
        txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight }, m.project),
        label(t, caption, 13));
    }
  })();
  if (['cinema', 'playbill', 'gala', 'broadsheet', 'crt', 'arcade', 'neon', 'poster'].includes(t.family))
    return { w: W, h: H, tree: plate(t, { width: W, height: H }, [
      box({ padding: '34px 0 0' }, head),
      col({ flex: 1, padding: '0 46px' }, body),
      leg ? box({ padding: '0 46px 30px' }, leg) : null,
    ]) };
  return { w: W, h: H, tree: col({ width: W, height: H, padding: '64px 80px 56px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    head,
    col({ flex: 1 }, body),
    leg ? box({ marginTop: 36 }, leg) : null) };
}

/** 华夫饼：100 格，一格 1% */
export function waffle(m: Model, t: Theme): Out {
  const W = 1200, H = 680;
  const cells = allot(m.rows.map((r) => r.pct), 100);
  const seq: (Contributor | null)[] = m.rows.flatMap((r, i) => Array(cells[i]).fill(r));
  const top = m.rows[0];
  if (t.family === 'terminal') {
    const grid = col({},
      ...Array.from({ length: 10 }, (_, y) => box({ height: 44, alignItems: 'center' },
        ...Array.from({ length: 10 }, (_, x) => {
          const r = seq[y * 10 + x];
          return txt({ fontFamily: MONO_FONT, fontSize: 44, lineHeight: 1, letterSpacing: 14, color: r ? (r.key === 'unattributed' ? t.faint : famColor(r.key, t, rankOf(m, r.key))) : 'rgba(215,222,229,0.14)' }, r ? '■' : '·');
        }))));
    return frame(m, t, W, H, '一格 = 1%', box({ flex: 1, gap: 96, alignItems: 'center' },
      grid,
      col({ flex: 1, justifyContent: 'center', gap: 26 },
        col({ gap: 6 },
          txt({ fontFamily: MONO_FONT, fontSize: 84, color: t.ink, lineHeight: 1 }, `${Math.round(top.pct)}`),
          txt({ fontFamily: MONO_FONT, fontSize: 17, color: t.soft }, `格是 ${top.name}`)),
        col({ gap: 14 },
          ...m.rows.slice(1).map((r, i) => box({ alignItems: 'center', gap: 12 },
            txt({ fontFamily: MONO_FONT, fontSize: 14, color: t.faint }, '▪'),
            txt({ fontSize: 17, color: r.key === 'unattributed' ? t.soft : t.ink, fontFamily: MONO_FONT }, r.name),
            txt({ marginLeft: 'auto', fontFamily: MONO_FONT, fontSize: 16, color: t.soft }, `${cells[i + 1]} 格`)))))));
  }
  const S = 38, G = 6;
  const cr = t.family === 'glass' ? 11 : t.barRadius ? 7 : 0;
  const grid = col({ gap: G },
    ...Array.from({ length: 10 }, (_, y) => box({ gap: G },
      ...Array.from({ length: 10 }, (_, x) => {
        const r = seq[y * 10 + x];
        return box({ width: S, height: S, borderRadius: cr, ...(r ? fill(r, t, rankOf(m, r.key), 4) : { backgroundColor: t.track }) });
      }))));
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
  const W = 1200, H = 630, R = 210;
  const w = t.family === 'film' ? 18 : t.family === 'editorial' || t.family === 'mono' ? 58 : t.barH >= 8 ? 52 : 34;
  const r0 = R - w / 2, C = 2 * Math.PI * r0;
  let acc = 0;
  const pat = `<defs><pattern id="u" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="8" height="8" fill="${t.stripeB}"/><rect width="4" height="8" fill="${t.stripeA}"/></pattern></defs>`;
  const famGap: Record<string, number> = { film: 12, editorial: 6, mono: 8, terminal: 6, glass: 8, minimal: 4 };
  const gap = famGap[t.family] ?? 4;
  const cap = t.family === 'minimal' || t.family === 'glass' ? ' stroke-linecap="round"' : '';
  const arcs = m.rows.map((r, i) => {
    const len = (r.pct / 100) * C;
    const col = r.key === 'unattributed' ? 'url(#u)' : famColor(r.key, t, i);
    const s = `<circle cx="${R}" cy="${R}" r="${r0}" fill="none" stroke="${col}" stroke-width="${w}" stroke-dasharray="${Math.max(len - gap, 0.8)} ${C}" stroke-dashoffset="${-acc}" transform="rotate(-90 ${R} ${R})"${cap}/>`;
    acc += len;
    return s;
  }).join('');
  const ring = box({ position: 'relative', width: 2 * R, height: 2 * R, alignItems: 'center', justifyContent: 'center' },
    svgImage(`<svg xmlns="http://www.w3.org/2000/svg" width="${2 * R}" height="${2 * R}">${pat}<circle cx="${R}" cy="${R}" r="${r0}" fill="none" stroke="${t.track}" stroke-width="${w}"/>${arcs}</svg>`, 2 * R, 2 * R, { position: 'absolute', left: 0, top: 0 }),
    col({ alignItems: 'center', gap: 6 },
      txt({ fontFamily: t.num, fontSize: t.family === 'film' ? 56 : 48, color: t.ink, letterSpacing: -1 }, compact(m.linesAdded)),
      label(t, '行', 13)));
  const legend = col({ flex: 1, gap: 18, justifyContent: 'center' },
    txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight, marginBottom: 20 }, m.project),
    ...m.rows.map((r) => box({ alignItems: 'center', gap: 14, paddingBottom: 14, borderBottom: `1px solid ${hair(t)}` },
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
  const step = ['film', 'terminal', 'editorial', 'mono'].includes(t.family);
  const smooth = (pts: [number, number][]) => pts.map((p, i) => {
    if (i === 0) return `M${p[0]},${p[1]}`;
    if (step) return `L${p[0]},${p[1]}`;
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
    paths.push(`<path d="${d}" fill="${k === 'unattributed' ? 'url(#u)' : famColor(k, t, rankOf(m, k))}"/>`);
    base = top;
  }
  const grid = [0.25, 0.5, 0.75].map((f) => `<line x1="0" x2="${CW}" y1="${CH * f}" y2="${CH * f}" stroke="${t.family === 'editorial' ? 'rgba(18,17,16,0.25)' : t.line}" stroke-width="1"/>`).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${CW}" height="${CH}">${pat}${grid}${paths.join('')}</svg>`;
  const from = m.range.from ? Date.parse(m.range.from) : 0;
  const lbl = (i: number) => txt({ fontSize: 14, color: t.soft, letterSpacing: 1 }, weekLabel(weeks[i].week, i === 0 ? from : 0));
  return frame(m, t, 1200, 700, '每周新增代码', col({ gap: 14 }, svgImage(svg, CW, CH), box({ justifyContent: 'space-between', width: CW }, lbl(0), lbl(Math.floor(weeks.length / 2)), lbl(weeks.length - 1))));
}

/** 泳道：每个工具一条，会话画成横条 */
export function lanes(m: Model, t: Theme): Out | null {
  const ss: { contributor: string; start: number; end: number; review: boolean }[] = (m.raw.sessions ?? []).filter((s: any) => !s.review);
  if (!ss.length) return null;
  const g = groupFn(m);
  const term = t.family === 'terminal';
  const keys = m.rows.filter((r) => r.kind === 'agent' && ss.some((s) => g(s.contributor) === r.key)).map((r) => r.key);
  const t0 = Math.min(...ss.map((s) => s.start)), t1 = Math.max(...ss.map((s) => s.end));
  const f = new Date((t0 + 8 * 3600) * 1000);
  const from = Date.UTC(f.getUTCFullYear(), f.getUTCMonth(), 1) / 1000 - 8 * 3600;
  const LW = 1040 - 170, LH = 64;
  const X = (ts: number) => ((ts - from) / (t1 - from)) * LW;
  const rows = keys.map((k) => box({ alignItems: 'center', height: LH, borderBottom: `1px solid ${hair(t)}` },
    box({ width: 175, alignItems: 'center', gap: 10 },
      term ? txt({ fontFamily: MONO_FONT, fontSize: 13, color: t.faint }, '├─') : null,
      mark(k, t, term ? 15 : 18),
      txt({ fontSize: term ? 15 : 17, fontFamily: term ? MONO_FONT : t.sans }, nameOf(k))),
    box({ position: 'relative', width: LW, height: 20 },
      ...ss.filter((s) => g(s.contributor) === k).map((s) =>
        box({ position: 'absolute', left: X(s.start), top: 0, width: Math.max(X(s.end) - X(s.start), 2), height: 20, borderRadius: term ? 0 : t.barRadius ? 3 : 0, backgroundColor: famColor(k, t, rankOf(m, k)), opacity: 0.9 })))));
  const ticks: Node[] = [];
  for (let d = from; d <= t1 - 4 * 86400; d += 86400) {
    const cn = new Date((d + 8 * 3600) * 1000);
    if (cn.getUTCDate() === 1) ticks.push(txt({ position: 'absolute', left: X(d), fontSize: 14, color: t.soft, letterSpacing: 1 }, `${cn.getUTCMonth() + 1}月`));
  }
  return frame(m, t, 1200, 64 + 80 + keys.length * LH + 50 + 56, `${fmt(ss.length)} 次会话`,
    col({}, box({ borderTop: `1px solid ${hair(t)}` }), ...rows, box({ marginLeft: 175, position: 'relative', height: 30, marginTop: 14 }, ...ticks)), false);
}

/** 作息打卡：星期 × 小时 */
export function clock(m: Model, t: Theme): Out | null {
  const c: number[][] = m.raw.clock;
  if (!c) return null;
  const max = Math.max(...c.flat());
  const S = 34, G = 6;
  const wd = ['一', '二', '三', '四', '五', '六', '日'];
  const top = m.rows.find((r) => r.kind === 'agent');
  const base = t.family === 'terminal' ? hex(t.accent)
    : t.family === 'film' ? '237,230,216'
    : t.family === 'editorial' ? '18,17,16'
    : t.family === 'mono' ? '0,0,0'
    : top ? hex(famColor(top.key, t)) : t.dark ? '240,238,233' : '22,21,20';
  const cr = t.family === 'glass' ? 9 : t.barRadius ? 7 : 0;
  const ff = t.family === 'terminal' ? MONO_FONT : t.sans;
  const grid = col({ gap: G, justifyContent: 'center', flex: 1 },
    ...c.map((row, i) => box({ alignItems: 'center', gap: G },
      txt({ width: 40, fontSize: 14, color: t.soft, fontFamily: ff }, `周${wd[i]}`),
      ...row.map((v) => box({ width: S, height: S, borderRadius: cr, backgroundColor: v ? `rgba(${base},${(0.12 + 0.88 * Math.pow(v / max, 0.9)).toFixed(3)})` : t.track })))),
    box({ marginLeft: 40 + G, gap: G, marginTop: 6 },
      ...Array.from({ length: 24 }, (_, hr) => txt({ width: S, justifyContent: 'center', fontSize: 13, color: t.soft, fontFamily: ff }, hr % 6 === 0 ? `${hr}时` : ''))));
  return frame(m, t, 1200, 620, `${fmt(m.commits)} 次提交`, grid, false);
}

/** 版本柱状图 */
export function versions(m: Model, t: Theme): Out | null {
  const tags: { tag: string; date: string }[] = m.raw.versions ?? [];
  if (!tags.length) return null;
  const g = groupFn(m);
  const by: Record<string, Record<string, number>> = {};
  for (const b of m.raw.breakdown ?? []) {
    const k = g(b.contributor);
    if ((m.raw.directors ?? []).some((d: any) => d.id === b.contributor)) continue;
    for (const [v, num] of Object.entries<number>(b.byVersion ?? {})) { by[v] ??= {}; by[v][k] = (by[v][k] ?? 0) + num; }
  }
  const order = [...tags.map((x) => x.tag), 'unreleased'].filter((v) => by[v]);
  const stack = m.rows.map((r) => r.key);
  const totals = order.map((v) => Object.values(by[v]).reduce((a, b) => a + b, 0));
  const max = Math.max(...totals), BH = 340;
  const bars = order.map((v, i) => col({ alignItems: 'center', width: 200 },
    box({ height: BH, alignItems: 'flex-end' },
      col({ width: 96, height: (totals[i] / max) * BH, borderRadius: t.family === 'glass' ? 10 : t.barRadius ? 8 : 0, overflow: 'hidden', flexDirection: 'column-reverse', gap: 2 },
        ...stack.map((k, j) => (by[v][k] ? box({ height: `${(by[v][k] / totals[i]) * 100}%`, minHeight: 2, ...fill({ key: k }, t, j, 3) }) : null)))),
    txt({ marginTop: 22, fontFamily: t.num === t.sans ? MONO_FONT : t.num, fontSize: 22, color: t.ink }, v === 'unreleased' ? '未发布' : v),
    txt({ marginTop: 6, fontSize: 14, color: t.soft }, v === 'unreleased' ? `${compact(totals[i])} 行` : `${tags.find((x) => x.tag === v)?.date.replaceAll('-', '.')} · ${compact(totals[i])} 行`)));
  return frame(m, t, 1200, 740, '各版本新增代码', box({ justifyContent: 'center', gap: 64, flex: 1, alignItems: 'flex-end' }, ...bars));
}

/** 沉淀：写下的代码至今还剩多少 */
export function settle(m: Model, t: Theme): Out | null {
  if (!m.linesAdded || !m.linesSurviving) return null;
  const keep = (m.linesSurviving / m.linesAdded) * 100;
  const W = 1200, H = 560;
  if (t.family === 'terminal') {
    const bar = (p: number, color: string) => box({ alignItems: 'center' },
      txt({ fontFamily: MONO_FONT, fontSize: 28, color, whiteSpace: 'pre' }, '■'.repeat(Math.max(1, Math.round(p / 5)))),
      txt({ fontFamily: MONO_FONT, fontSize: 28, color: t.faint, whiteSpace: 'pre' }, '·'.repeat(20 - Math.max(1, Math.round(p / 5)))));
    const row = (k: string, b: Node, v: string) => box({ alignItems: 'center', gap: 22 },
      txt({ fontFamily: MONO_FONT, fontSize: 20, color: t.soft, width: 120 }, k), b,
      txt({ fontFamily: MONO_FONT, fontSize: 20, color: t.ink }, v));
    return { w: W, h: H, tree: col({ width: W, height: H, padding: '64px 96px', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: MONO_FONT },
      box({ alignItems: 'baseline', gap: 14 },
        txt({ fontFamily: MONO_FONT, fontSize: 18, color: t.accent }, '❯'),
        txt({ fontFamily: MONO_FONT, fontSize: 18 }, 'creditia survive'),
        txt({ fontFamily: MONO_FONT, fontSize: 15, color: t.faint }, '# 写下的代码留下多少')),
      col({ flex: 1, justifyContent: 'center', gap: 36 },
        row('写下', bar(100, t.ink), fmt(m.linesAdded)),
        row('至今仍在', bar(keep, t.accent), `${fmt(m.linesSurviving)}  ·  ${Math.round(keep)}%`))) };
  }
  const step = (k: string, v: string, p: number, solid: boolean) => col({ gap: 16 },
    box({ justifyContent: 'space-between', alignItems: 'baseline' },
      label(t, k, 14),
      txt({ fontFamily: t.num, fontSize: 30, color: t.ink }, v)),
    box({ height: Math.max(t.barH * 3, 14), borderRadius: t.family === 'glass' ? 99 : t.barRadius, backgroundColor: t.track },
      box({ width: `${p}%`, height: '100%', borderRadius: t.family === 'glass' ? 99 : t.barRadius, backgroundColor: solid ? t.ink : t.faint })));
  return { w: W, h: H, tree: col({ width: W, height: H, padding: '72px 96px', justifyContent: 'space-between', ...backdrop(t, keysOf(m)), color: t.ink, fontFamily: t.sans },
    box({ alignItems: 'baseline', justifyContent: 'space-between' },
      txt({ fontFamily: t.display, fontSize: t.display === t.sans ? 30 : 38, fontWeight: t.displayWeight }, m.project),
      box({ alignItems: 'baseline', gap: 8 }, txt({ fontFamily: t.num, fontSize: 64, letterSpacing: -1 }, `${Math.round(keep)}%`), txt({ fontSize: 17, color: t.soft }, '留到了今天'))),
    col({ gap: 40 },
      step('写下', `${fmt(m.linesAdded)} 行`, 100, false),
      step('至今仍在', `${fmt(m.linesSurviving)} 行`, keep, true))) };
}

/** 时间织带：每天一格。胶片 / 终端 / 常规三种画法 */
export function ribbon(m: Model, t: Theme): Out | null {
  const days: { date: string; by: Record<string, number> }[] = m.raw.days ?? [];
  if (!days.length) return null;
  const start = Date.parse(days[0].date), end = Date.parse(days[days.length - 1].date);
  const n = Math.round((end - start) / 86400000) + 1;
  const map = new Map(days.map((d) => [d.date, d.by]));
  const W = 1200, inner = W - 160, gap = n > 120 ? 2 : 4;
  const cw = (inner - gap * (n - 1)) / n;
  const dates = Array.from({ length: n }, (_, i) => new Date(start + i * 86400000).toISOString().slice(0, 10));
  const keyOf = (d: string) => {
    const by = map.get(d);
    if (!by) return '';
    const top = Object.entries(by).filter(([k]) => k !== 'unattributed').sort((a, b) => b[1] - a[1])[0];
    if (!top) return 'unattributed';
    return top[0].startsWith('h:') ? 'human' : top[0];
  };
  const stOf = (k: string) => (k ? fill({ key: k }, t, rankOf(m, k), 2) : { backgroundColor: t.track });

  if (t.family === 'terminal') {
    const months = new Map<string, string[]>();
    for (const d of dates) {
      const mk = `${d.slice(0, 4)}.${d.slice(5, 7)}`;
      if (!months.has(mk)) months.set(mk, []);
      months.get(mk)!.push(d);
    }
    const rows = [...months.entries()].map(([mk, ds]) => box({ height: 34, alignItems: 'center' },
      txt({ fontFamily: MONO_FONT, fontSize: 12, color: t.faint, width: 92, flexShrink: 0 }, mk),
      ...ds.map((d) => {
        const k = keyOf(d);
        return txt({ fontFamily: MONO_FONT, fontSize: 15, lineHeight: 1, letterSpacing: 6, color: k ? (k === 'unattributed' ? t.faint : famColor(k, t, rankOf(m, k))) : 'rgba(215,222,229,0.15)' }, k ? '■' : '·');
      })));
    return frame(m, t, W, 280 + rows.length * 38, `${n} 天`, col({ gap: 6, justifyContent: 'center', flex: 1 }, ...rows));
  }

  if (t.family === 'film') {
    const hole = () => {
      const cnt = Math.floor((inner - 28) / 38);
      const g2 = ((inner - 28) - cnt * 16) / Math.max(cnt - 1, 1);
      return box({ gap: g2 }, ...Array.from({ length: cnt }, () => box({ width: 16, height: 12, borderRadius: 3, backgroundColor: t.bg, flexShrink: 0 })));
    };
    const frames = box({ gap, height: 168 }, ...dates.map((d) => box({ width: cw, height: 168, ...stOf(keyOf(d)) })));
    const strip = col({ width: inner, backgroundColor: '#1B1914', padding: '9px 14px', gap: 9, borderRadius: 4, alignSelf: 'center' }, hole(), frames, hole());
    const marks: Node[] = [];
    for (let i = 0; i < n; i++) { const dd = new Date(start + i * 86400000); if (dd.getUTCDate() === 1 || i === 0) marks.push(txt({ position: 'absolute', left: 14 + i * (cw + gap), fontSize: 13, color: t.soft, letterSpacing: 1 }, `${dd.getUTCMonth() + 1}月`)); }
    return frame(m, t, W, 560, `${n} 天`, col({ gap: 12, justifyContent: 'center', flex: 1 }, strip, box({ position: 'relative', height: 18, width: inner, alignSelf: 'center' }, ...marks)));
  }

  const radius = t.family === 'glass' ? Math.min(8, cw / 2.2) : t.barRadius ? Math.min(3, cw / 3) : 0;
  const cells: Node[] = [], marks: Node[] = [];
  for (let i = 0; i < n; i++) {
    const dd = new Date(start + i * 86400000);
    cells.push(box({ width: cw, height: 180, borderRadius: radius, ...stOf(keyOf(dates[i])) }));
    if (dd.getUTCDate() === 1 || i === 0) marks.push(txt({ position: 'absolute', left: i * (cw + gap), fontSize: 14, color: t.soft, letterSpacing: 1 }, `${dd.getUTCMonth() + 1}月`));
  }
  return frame(m, t, W, 500, `${n} 天`, col({ gap: 14 }, box({ gap }, ...cells), box({ position: 'relative', height: 20 }, ...marks)));
}

/** 代码马赛克：现在仓库里每个文件一块 */
export function mosaic(m: Model, t: Theme): Out | null {
  const files: { lines: number; by: Record<string, number> }[] = (m.raw.files ?? []).filter((f: any) => f.lines > 0);
  if (!files.length) return null;
  const W = 1200, H = 860, bw = W - 160, bh = H - 64 - 56 - 80 - 70;
  const items = files.map((f) => {
    const ent = Object.entries(f.by).sort((a, b) => b[1] - a[1])[0];
    const key = ent ? (ent[0].startsWith('h:') ? 'human' : ent[0]) : 'unattributed';
    return { v: f.lines, key };
  }).sort((a, b) => b.v - a.v);
  const rects = squarify(items, bw, bh);
  const g = ({ mono: 3, film: 2, editorial: 2, terminal: 1 } as Record<string, number>)[t.family] ?? 2;
  const tiles = rects.map((q) => box({ position: 'absolute', left: q.x, top: q.y, width: Math.max(q.w - g, 0.5), height: Math.max(q.h - g, 0.5), borderRadius: t.family === 'glass' && q.w > 10 && q.h > 10 ? 5 : t.barRadius && q.w > 10 && q.h > 10 ? 3 : 0, ...fill({ key: q.key }, t, rankOf(m, q.key), 3) }));
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
      txt({ fontSize: 15, color: t.soft, fontFamily: MONO_FONT }, `${fmt(p.linesAdded)} 行`)),
    box({ height: t.family === 'film' ? 4 : t.barH * 2, borderRadius: t.family === 'glass' ? 99 : t.barRadius, overflow: 'hidden', backgroundColor: t.track, gap: 2 },
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

// ───────────────────────── 辅助 ─────────────────────────

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
  if (c?.kind === 'human') return 'human';
  return m.rows.some((r) => r.key === id) ? id : 'other';
};
const hex = (c: string) => {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(c);
  return m ? `${parseInt(m[1], 16)},${parseInt(m[2], 16)},${parseInt(m[3], 16)}` : '120,120,120';
};

// ═════════════════════════ 第二轮的体裁：tier ═════════════════════════
// 每个体裁有自己的表头语汇（片尾字幕卡、报头、节目单横幅、颁奖台、终端提示、
// 街机高分榜、霓虹招牌、海报大字）和行式（居中、点线、条形、表格、分数），
// 但下面的信息始终是同一条：谁做了什么、各占多少。

type TierHead = 'marquee' | 'masthead' | 'banner' | 'glow' | 'board' | 'scoreboard' | 'neon' | 'poster';
type TierRow = 'plain' | 'leader' | 'bar' | 'cell' | 'score';

interface TierCfg { head: TierHead; row: TierRow; w: number; rowH: number; headH: number }
const TIER_CFG: Record<string, TierCfg> = {
  cinema: { head: 'marquee', row: 'plain', w: 470, rowH: 56, headH: 152 },
  playbill: { head: 'banner', row: 'leader', w: 540, rowH: 58, headH: 138 },
  gala: { head: 'glow', row: 'plain', w: 490, rowH: 60, headH: 156 },
  broadsheet: { head: 'masthead', row: 'cell', w: 580, rowH: 54, headH: 130 },
  crt: { head: 'board', row: 'bar', w: 540, rowH: 46, headH: 96 },
  arcade: { head: 'scoreboard', row: 'score', w: 560, rowH: 56, headH: 140 },
  neon: { head: 'neon', row: 'bar', w: 470, rowH: 56, headH: 158 },
  poster: { head: 'poster', row: 'bar', w: 540, rowH: 64, headH: 148 },
};

export function tierStyled(m: Model, t: Theme, s: number, withTitle: boolean): Out {
  const cfg = TIER_CFG[t.family]!;
  const W = cfg.w * s;
  const ranked = m.rows.filter((r) => r.key !== 'human');
  const headH = withTitle ? cfg.headH : 32;
  const humanH = m.humans.length ? 38 : 0;
  const content = (headH + ranked.length * cfg.rowH + humanH + 64) * s;
  const { pad, border } = plateMetrics(t);
  const H = content + 2 * pad + border;
  const totals = [m.commits && `${fmt(m.commits)} 次提交`, m.linesAdded && `${fmt(m.linesAdded)} 行`].filter(Boolean).join(t.family === 'crt' ? '   ' : '  ·  ');
  return { w: W, h: H, tree: plate(t, { width: W, height: H }, [
    tierHead(cfg.head, m, t, s, withTitle),
    col({}, ...ranked.map((r, i) => tierRowN(cfg.row, r, i, m, t, s))),
    humanLine(m, t, s),
    tierFoot(t, s, totals, m),
  ], { justifyContent: 'space-between' }) };
}

function tierHead(kind: TierHead, m: Model, t: Theme, s: number, on: boolean): Node {
  const dir = m.director;
  switch (kind) {
    case 'marquee':
      return col({ height: (on ? 152 : 32) * s, alignItems: 'center', justifyContent: 'center', gap: 12 * s },
        on ? label(t, 'credits', 10 * s, { color: t.accent, letterSpacing: 8 * s }) : null,
        on ? txt({ fontFamily: t.deco, fontSize: 42 * s, color: t.ink, textAlign: 'center', lineHeight: 1.1, letterSpacing: 1.5 * s }, m.project) : null,
        dir ? txt({ fontFamily: t.display, fontSize: 14 * s, color: t.soft, letterSpacing: 2 * s }, `${dir} 作品`) : null,
        on ? ornament(t, 10 * s, { marginTop: 2 * s }) : null);
    case 'masthead':
      return col({ height: (on ? 130 : 32) * s, justifyContent: 'center', gap: 7 * s },
        box({ justifyContent: 'space-between', alignItems: 'baseline' },
          label(t, (m.range.from || '').slice(0, 10).replaceAll('-', '.'), 11 * s),
          label(t, 'creditia daily', 11 * s),
          label(t, `no.${String(m.commits % 1000).padStart(3, '0')}`, 11 * s)),
        box({ borderTop: `2px solid ${t.ink}`, borderBottom: `1px solid ${t.ink}`, padding: `${9 * s}px 0 ${7 * s}px`, alignItems: 'center', justifyContent: 'center' },
          on ? txt({ fontFamily: t.deco, fontSize: 44 * s, color: t.ink, textAlign: 'center', lineHeight: 1 }, m.project) : txt({ fontFamily: t.labelFont, fontSize: 11 * s, color: t.soft }, '署名')),
        box({ borderBottom: `4px solid ${t.ink}` }));
    case 'banner':
      return col({ height: (on ? 138 : 32) * s, alignItems: 'center', justifyContent: 'center', gap: 8 * s },
        on ? box({ backgroundColor: t.accent, padding: `${6 * s}px ${18 * s}px` },
          txt({ fontFamily: t.labelFont, fontSize: 12 * s, color: t.surface, letterSpacing: 5 * s }, 'PLAYBILL 节目单')) : null,
        on ? txt({ fontFamily: t.display, fontSize: 52 * s, fontWeight: 900, color: t.ink, textAlign: 'center', lineHeight: 1.02 }, m.project) : null,
        dir ? txt({ fontFamily: t.sans, fontSize: 13 * s, fontStyle: 'italic', color: t.soft }, `导演 ${dir}`) : null,
        on ? ornament(t, 12 * s) : null);
    case 'glow':
      return col({ height: (on ? 156 : 32) * s, alignItems: 'center', justifyContent: 'center', gap: 9 * s },
        on ? txt({ fontFamily: t.labelFont, fontSize: 13 * s, color: t.accent, letterSpacing: 9 * s, textTransform: 'uppercase' }, 'the credit awards') : null,
        on ? box({ width: '58%', height: 1, backgroundColor: t.accent, opacity: 0.55 }) : null,
        on ? txt({ fontFamily: t.display, fontSize: 46 * s, color: t.ink, textAlign: 'center', lineHeight: 1.08, letterSpacing: 3 * s }, m.project) : null,
        on ? box({ width: '58%', height: 1, backgroundColor: t.accent, opacity: 0.55 }) : null,
        dir ? label(t, `最佳作品 · ${dir}`, 11 * s, { color: t.soft }) : null);
    case 'board':
      return col({ height: (on ? 96 : 32) * s, gap: 6 * s, justifyContent: 'center' },
        on ? txt({ fontFamily: t.sans, fontSize: 22 * s, color: t.ink }, `> creditia tier ${m.project}`) : null,
        dir ? txt({ fontFamily: t.sans, fontSize: 18 * s, color: t.soft }, `> 主导：${dir}`) : null,
        txt({ fontFamily: t.sans, fontSize: 18 * s, color: t.faint, whiteSpace: 'pre' }, '─'.repeat(48)));
    case 'scoreboard':
      return col({ height: (on ? 140 : 32) * s, alignItems: 'center', justifyContent: 'center', gap: 12 * s },
        on ? txt({ fontFamily: t.display, fontSize: 22 * s, color: t.accent, textAlign: 'center', lineHeight: 1.7, letterSpacing: 2 * s }, 'HIGH SCORES') : null,
        on ? txt({ fontFamily: t.sans, fontSize: 28 * s, color: t.ink, textAlign: 'center' }, m.project) : null,
        on ? txt({ fontFamily: t.display, fontSize: 12 * s, color: t.faint }, '*  *  *') : null);
    case 'neon':
      return col({ height: (on ? 158 : 32) * s, alignItems: 'center', justifyContent: 'center', gap: 14 * s },
        on ? box({ position: 'relative', alignItems: 'center', justifyContent: 'center' },
          txt({ position: 'absolute', fontFamily: t.display, fontSize: 40 * s, color: t.accent, letterSpacing: 2 * s, opacity: 0.55 }, m.project),
          txt({ fontFamily: t.display, fontSize: 40 * s, color: t.ink, letterSpacing: 2 * s }, m.project)) : null,
        on ? box({ padding: `${4 * s}px ${15 * s}px`, border: `1px solid ${t.accent}`, borderRadius: 999 },
          txt({ fontFamily: t.sans, fontSize: 12 * s, color: t.accent, letterSpacing: 4 * s }, 'OPEN 24H · 署名')) : null,
        dir ? label(t, dir, 11 * s, { color: t.soft }) : null);
    case 'poster':
      return col({ height: (on ? 148 : 32) * s, justifyContent: 'center', gap: 9 * s },
        on ? box({ alignItems: 'center', gap: 12 * s },
          box({ width: 16 * s, height: 16 * s, backgroundColor: t.accent }),
          txt({ fontFamily: t.display, fontSize: 52 * s, color: t.ink, lineHeight: 1, textTransform: 'uppercase', letterSpacing: -1 * s }, m.project)) : null,
        on ? box({ height: 5 * s, backgroundColor: t.ink, width: '100%' }) : null,
        box({ justifyContent: 'space-between', alignItems: 'baseline' },
          label(t, 'credits 署名', 11 * s), label(t, dir ? `lead ${dir}` : 'creditia', 11 * s)));
  }
}

function tierRowN(kind: TierRow, r: Contributor, i: number, m: Model, t: Theme, s: number): Node {
  const dim = r.key === 'unattributed';
  switch (kind) {
    case 'plain': // 居中，无条形（片尾字幕、颁奖礼）
      return box({ height: TIER_CFG[t.family === 'cinema' ? 'cinema' : 'gala'].rowH * s, alignItems: 'baseline', justifyContent: 'center', gap: 14 * s },
        mark(r.key, t, 15 * s, t.faint),
        txt({ fontFamily: t.display, fontSize: 27 * s, fontWeight: t.displayWeight, color: dim ? t.soft : t.ink, letterSpacing: t.family === 'gala' ? 2 * s : 0 }, r.name),
        txt({ fontFamily: t.num, fontSize: 14 * s, color: t.faint }, pct(r.pct)));
    case 'leader': // 名 + 点线 + 百分比（节目单）
      return box({ height: TIER_CFG.playbill.rowH * s, alignItems: 'baseline', borderBottom: `1px solid ${hair(t)}` },
        txt({ width: 40 * s, fontFamily: t.num, fontSize: 22 * s, color: t.accent }, String(i + 1).padStart(2, '0')),
        txt({ fontSize: 19 * s, fontWeight: 700, color: dim ? t.soft : t.ink }, r.name),
        box({ flex: 1, margin: `0 ${10 * s}px`, ...dotline(t) }),
        txt({ fontFamily: t.num, fontSize: 24 * s, color: dim ? t.soft : t.ink }, pct(r.pct)));
    case 'cell': // 表格行（报纸）
      return box({ height: TIER_CFG.broadsheet.rowH * s, alignItems: 'center', gap: 12 * s, borderBottom: `1px solid ${hair(t)}` },
        mark(r.key, t, 16 * s, t.ink),
        txt({ fontFamily: t.display, fontSize: 21 * s, fontWeight: 700, color: dim ? t.soft : t.ink }, r.name),
        box({ flex: 1, margin: `0 ${10 * s}px`, ...dotline(t) }),
        txt({ fontFamily: t.num, fontSize: 22 * s, color: dim ? t.soft : t.ink }, pct(r.pct)));
    case 'score': // 高分榜
      return box({ height: TIER_CFG.arcade.rowH * s, alignItems: 'center', gap: 14 * s },
        txt({ fontFamily: t.display, fontSize: 13 * s, color: i === 0 ? t.accent : t.faint, width: 34 * s }, String(i + 1).padStart(2, '0')),
        mark(r.key, t, 18 * s, t.ink),
        txt({ fontFamily: t.sans, fontSize: 22 * s, color: dim ? t.soft : t.ink, letterSpacing: 1 * s }, r.name),
        box({ flex: 1, margin: `0 ${12 * s}px`, ...dotline(t, t.edge) }),
        txt({ fontFamily: t.display, fontSize: 14 * s, color: i === 0 ? t.accent : t.ink }, pct(r.pct)));
    default: // bar（终端、霓虹、海报）
      return col({ height: TIER_CFG[t.family === 'crt' ? 'crt' : t.family === 'neon' ? 'neon' : 'poster'].rowH * s, justifyContent: 'center', gap: 8 * s },
        box({ alignItems: 'baseline', gap: 10 * s },
          mark(r.key, t, 15 * s, t.faint),
          txt({ fontSize: t.family === 'crt' ? 20 * s : 19 * s, fontWeight: t.family === 'neon' ? 400 : 500, color: dim ? t.soft : t.ink, letterSpacing: 0.5 * s }, r.name),
          txt({ marginLeft: 'auto', fontFamily: t.num, fontSize: 15 * s, color: dim ? t.faint : t.soft }, pct(r.pct))),
        bar(t, r, rankOf(m, r.key), t.barH * s, 3 * s));
  }
}

function tierFoot(t: Theme, s: number, totals: string, m: Model): Node {
  const mark_ = () => ornament(t, 10 * s);
  switch (t.family) {
    case 'cinema':
      return col({ height: 64 * s, alignItems: 'center', justifyContent: 'flex-end', gap: 8 * s },
        mark_(), txt({ fontFamily: t.labelFont, fontSize: 10 * s, color: t.soft, letterSpacing: 3 * s, textTransform: 'uppercase' }, totals));
    case 'playbill':
      return box({ height: 64 * s, alignItems: 'flex-end', justifyContent: 'space-between', borderTop: `1px dashed ${t.edge}`, paddingTop: 10 * s },
        txt({ fontFamily: t.sans, fontSize: 12 * s, color: t.soft, fontStyle: 'italic' }, totals), mark_());
    case 'gala':
      return col({ height: 64 * s, alignItems: 'center', justifyContent: 'flex-end', gap: 9 * s },
        box({ width: '54%', height: 1, backgroundColor: t.accent, opacity: 0.6 }),
        txt({ fontFamily: t.labelFont, fontSize: 11 * s, color: t.soft, letterSpacing: 5 * s, textTransform: 'uppercase' }, totals));
    case 'broadsheet':
      return box({ height: 64 * s, alignItems: 'center', justifyContent: 'space-between', borderTop: `5px solid ${t.ink}`, marginTop: 8 * s, paddingTop: 8 * s },
        label(t, totals, 11 * s), label(t, 'continued on page 2', 10 * s));
    case 'crt':
      return col({ height: 64 * s, justifyContent: 'flex-end', gap: 4 * s },
        txt({ fontFamily: t.sans, fontSize: 18 * s, color: t.faint }, '> 统计'),
        txt({ fontFamily: t.sans, fontSize: 20 * s, color: t.ink }, totals));
    case 'arcade':
      return col({ height: 64 * s, alignItems: 'center', justifyContent: 'flex-end', gap: 10 * s },
        txt({ fontFamily: t.display, fontSize: 13 * s, color: t.faint, textAlign: 'center', lineHeight: 1.7 }, 'INSERT COIN'),
        txt({ fontFamily: t.sans, fontSize: 17 * s, color: t.soft }, totals));
    case 'neon':
      return col({ height: 64 * s, alignItems: 'center', justifyContent: 'flex-end', gap: 9 * s },
        mark_(), txt({ fontFamily: t.sans, fontSize: 12 * s, color: t.soft, letterSpacing: 3 * s }, totals));
    default:
      return box({ height: 64 * s, alignItems: 'flex-end', justifyContent: 'space-between', borderTop: `6px solid ${t.ink}` },
        label(t, totals, 11 * s), label(t, m.project ? 'creditia' : '', 11 * s));
  }
}

export { stripes };
