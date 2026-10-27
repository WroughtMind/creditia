// 卡片清单：id → 渲染函数与说明。Node 和浏览器共用。
import type { Model } from './data.ts';
import type { Style } from './style.ts';
import type { Art } from './export.ts';
import type { Scene } from './svg.ts';
import type { PieceOpts } from './pieces/common.ts';
import { tier, TIER_VARIANTS } from './pieces/tier.ts';
import { roll, ROLL_VARIANTS } from './pieces/roll.ts';
import { DONUT_VARIANTS, WAFFLE_VARIANTS, donut, waffle, stream, lanes, clock, versions, survival, mosaic, calendar, evidence, areas } from './pieces/charts.ts';
import { receipt, RECEIPT_VARIANTS, share, badge, BADGE_VARIANTS, byline, BYLINE_VARIANTS, hand } from './pieces/cards.ts';
import { codemap } from './pieces/map.ts';
import { asterisk } from './pieces/asterisk.ts';
import { people } from './pieces/people.ts';
import { historyLabels } from './history.ts';

export interface PieceDef {
  name: string;
  en: string;
  /** 回答的问题 */
  q: string;
  qEn: string;
  variants?: string[];
  /** 能做成动画（GIF / HTML） */
  animated?: boolean;
  render: (m: Model, st: Style, o: PieceOpts) => Scene | null;
}

export const PIECES: Record<string, PieceDef> = {
  tier: { name: '署名榜', en: 'Credits', q: '工具与模型占比', qEn: 'Contribution shares by tool and model', variants: TIER_VARIANTS, animated: true, render: tier },
  asterisk: { name: '星号', en: 'Asterisk', q: '各家份额长成的 *', qEn: 'Shares grown into an asterisk', animated: true, render: asterisk },
  share: { name: '分享图', en: 'Share Card', q: '链接预览、社交媒体', qEn: 'Link previews, social posts', animated: true, render: share },
  hand: { name: '开发者', en: 'Developers', q: '提示词与审阅', qEn: 'Prompts and reviews', animated: true, render: hand },
  people: { name: '贡献者', en: 'Contributors', q: '每位提交者的提交里各工具占多少', qEn: 'What each committer\'s commits were written with', animated: true, render: people },
  receipt: { name: '小票', en: 'Receipt', q: '工期、会话与 Token', qEn: 'Time, sessions, and tokens spent', variants: RECEIPT_VARIANTS, render: receipt },
  badge: { name: '徽章', en: 'Badge', q: 'README、个人主页', qEn: 'README, profile page', variants: BADGE_VARIANTS, render: badge },
  byline: { name: '署名行', en: 'Byline', q: '页脚、关于页', qEn: 'Footer, about page', variants: BYLINE_VARIANTS, render: byline },
  roll: { name: '片尾字幕', en: 'Credits Roll', q: '分工与角色表', qEn: 'Contributors and roles', variants: ROLL_VARIANTS, animated: true, render: roll },
  donut: { name: '环形图', en: 'Donut', q: '代码占比', qEn: 'Shares', variants: DONUT_VARIANTS, animated: true, render: donut },
  waffle: { name: '华夫饼图', en: 'Waffle', q: '百格占比', qEn: 'One square is 1%', variants: WAFFLE_VARIANTS, animated: true, render: waffle },
  stream: { name: '河流图', en: 'Stream', q: '每周新增', qEn: 'New code each week', animated: true, render: stream },
  lanes: { name: '会话泳道', en: 'Sessions', q: '会话时间线', qEn: 'Session timeline by tool', animated: true, render: lanes },
  clock: { name: '作息打卡', en: 'Punch Card', q: '活跃时段', qEn: 'Weekday × hour', animated: true, render: clock },
  versions: { name: '版本柱状图', en: 'Releases', q: '各版本贡献', qEn: 'Code contributions per release', animated: true, render: versions },
  survival: { name: '留存', en: 'Retention', q: '新增与留存对比', qEn: 'Code retention comparison', animated: true, render: survival },
  mosaic: { name: '文件马赛克', en: 'File Mosaic', q: '各文件代码归属', qEn: 'Current codebase composition', animated: true, render: mosaic },
  calendar: { name: '日历', en: 'Calendar', q: '活跃日历', qEn: 'Daily activity calendar', animated: true, render: calendar },
  evidence: { name: '出处', en: 'Evidence', q: '归因依据', qEn: 'Evidence levels for each line', animated: true, render: evidence },
  areas: { name: '板块', en: 'Areas', q: '各模块代码归属', qEn: 'Contributions by area and folder', animated: true, render: areas },
  codemap: { name: '代码地图', en: 'Code Map', q: '代码归属地图', qEn: 'Line-by-line attribution map', animated: true, render: codemap },
};

/** 版式的中文名 */
export const VARIANT_NAMES: Record<string, [string, string]> = {
  rows: ['条形', 'Rows'], center: ['对称', 'Centered'], podium: ['领奖台', 'Podium'], vertical: ['竖排', 'Vertical'], hud: ['面板', 'HUD'], stats: ['能力', 'Stats'],
  classic: ['双栏', 'Classic'], scroll: ['银幕滚动', 'Scrolling'], card: ['终卡', 'End card'], ed: ['动画片尾', 'Anime ED'], billing: ['海报', 'Billing'], terminal: ['终端', 'Terminal'],
  split: ['分段', 'Split'], grouped: ['分组', 'Grouped'], nested: ['内外环', 'Nested'], ring: ['单环', 'Ring'],
  troupe: ['分部', 'Troupe'], itemized: ['明细', 'Itemized'], grid: ['方阵', 'Grid'], blocks: ['分块', 'Blocks'], line: ['一行', 'One line'], models: ['带模型', 'With models'],
  slip: ['小票', 'Slip'], ticket: ['电影票', 'Ticket'], flat: ['扁平', 'Flat'], stack: ['条形', 'Stack'], emblem: ['徽记', 'Emblem'],
};
/** 一个工具里有几个模型时最合适的版式：工具和它下面的模型分层画出来（两套渲染器都有） */
export const MODEL_VARIANTS: Record<string, string[]> = {
  tier: ['split', 'grouped'], donut: ['nested'], roll: ['troupe'], receipt: ['itemized'], waffle: ['blocks'], badge: ['split'], byline: ['models'],
};
export const variantName = (v: string, lang: 'zh' | 'en') => VARIANT_NAMES[v]?.[lang === 'zh' ? 0 : 1] ?? v;

/** 渲染成可导出的卡片 */
export function art(id: string, m: Model, st: Style, o: PieceOpts = {}): Art | null {
  const p = PIECES[id];
  if (!p) throw new Error(`未知卡片：${id}`);
  const s = p.render(m, st, o);
  if (!s) return null;
  const name = id === 'versions' ? historyLabels(m.history.basis, m.lang).name : m.lang === 'en' ? p.en : p.name;
  return { w: s.w, h: s.h, svg: s.toString(), hits: s.hits, bg: s.bg, title: `${m.project} · ${name}`, motion: (s as any).motion };
}
/** 动画帧：t 从 0 到 1 */
export function frames(id: string, m: Model, st: Style, o: PieceOpts = {}, n = 28): Art[] {
  const full = art(id, m, st, o);
  // 银幕滚动：一个固定大小的取景框从上往下扫过整张字幕
  if (full?.motion?.kind === 'scroll') {
    const { viewW, viewH } = full.motion;
    const steps = Math.max(n, 60);
    const travel = Math.max(0, full.h - viewH);
    const inner = full.svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '');
    return Array.from({ length: steps + 1 }, (_, i) => {
      const y = Math.round((travel * i) / steps);
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${viewW}" height="${viewH}" viewBox="0 ${y} ${viewW} ${viewH}"><rect y="${y}" width="${viewW}" height="${viewH}" fill="${full.bg}"/>${inner}</svg>`;
      // crop：导出 GIF 时整卷只栅格化一次，再按 y 裁出每一帧（逐帧裁 viewBox 会碰到 resvg 的滤镜区域 bug，也慢）
      return { ...full, w: viewW, h: viewH, svg, hits: [], motion: undefined, crop: { svg: full.svg, w: full.w, h: full.h, y } };
    });
  }
  const out: Art[] = [];
  for (let i = 0; i <= n; i++) { const a = art(id, m, st, { ...o, t: i / n }); if (a) out.push(a); }
  return out;
}
