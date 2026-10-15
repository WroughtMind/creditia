// 作品清单：id → 渲染函数与说明。Node 和浏览器共用。
import type { Model } from './data.ts';
import type { Style } from './style.ts';
import type { Art } from './export.ts';
import type { Scene } from './svg.ts';
import type { PieceOpts } from './pieces/common.ts';
import { tier, TIER_VARIANTS } from './pieces/tier.ts';
import { roll, ROLL_VARIANTS } from './pieces/roll.ts';
import { donut, waffle, stream, lanes, clock, versions, survival, mosaic, calendar, evidence, areas } from './pieces/charts.ts';
import { receipt, RECEIPT_VARIANTS, share, badge, BADGE_VARIANTS, byline, hand } from './pieces/cards.ts';
import { codemap } from './pieces/map.ts';

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
  tier: { name: '署名榜', en: 'Credits', q: '各工具写了多少', qEn: 'How much each tool wrote', variants: TIER_VARIANTS, animated: true, render: tier },
  share: { name: '分享图', en: 'Share Card', q: '链接预览、社交媒体', qEn: 'Link previews, social posts', animated: true, render: share },
  hand: { name: '人', en: 'People', q: '人做了多少：消息、review、人写的代码', qEn: 'What people did: messages, reviews, hand-written code', animated: true, render: hand },
  receipt: { name: '小票', en: 'Receipt', q: '花了多少时间、会话和 token', qEn: 'Time, sessions and tokens spent', variants: RECEIPT_VARIANTS, render: receipt },
  badge: { name: '徽章', en: 'Badge', q: 'README、个人主页', qEn: 'README, profile page', variants: BADGE_VARIANTS, render: badge },
  byline: { name: '署名行', en: 'Byline', q: '页脚、关于页', qEn: 'Footer, about page', render: byline },
  roll: { name: '片尾字幕', en: 'Credits Roll', q: '谁担任了什么角色', qEn: 'Who played which role', variants: ROLL_VARIANTS, animated: true, render: roll },
  donut: { name: '环形图', en: 'Donut', q: '占比', qEn: 'Shares', animated: true, render: donut },
  waffle: { name: '华夫饼图', en: 'Waffle', q: '一格 1%', qEn: 'One square is 1%', animated: true, render: waffle },
  stream: { name: '河流图', en: 'Stream', q: '每周新增代码', qEn: 'New code each week', animated: true, render: stream },
  lanes: { name: '会话泳道', en: 'Sessions', q: '各工具什么时候在干活', qEn: 'When each tool was at work', animated: true, render: lanes },
  clock: { name: '作息打卡', en: 'Punch Card', q: '星期 × 小时', qEn: 'Weekday × hour', animated: true, render: clock },
  versions: { name: '版本柱状图', en: 'Releases', q: '每个版本谁写的', qEn: 'Who wrote each release', animated: true, render: versions },
  survival: { name: '留存', en: 'Retention', q: '写的代码现在还剩多少', qEn: 'How much of the written code is still here', animated: true, render: survival },
  mosaic: { name: '文件马赛克', en: 'File Mosaic', q: '现在的代码由谁构成', qEn: 'Who the current code is made of', animated: true, render: mosaic },
  calendar: { name: '日历', en: 'Calendar', q: '哪天谁在写', qEn: 'Who wrote on which day', animated: true, render: calendar },
  evidence: { name: '出处', en: 'Evidence', q: '每一行凭什么算它的', qEn: 'What each line rests on', animated: true, render: evidence },
  areas: { name: '板块', en: 'Areas', q: '哪一块是谁写的', qEn: 'Who wrote which part', animated: true, render: areas },
  codemap: { name: '代码地图', en: 'Code Map', q: '现在的每一行是谁写的', qEn: 'Who wrote every line in the code now', animated: true, render: codemap },
};

/** 版式的中文名 */
export const VARIANT_NAMES: Record<string, [string, string]> = {
  rows: ['条形', 'Rows'], center: ['对称', 'Centered'], podium: ['领奖台', 'Podium'], vertical: ['竖排', 'Vertical'], hud: ['面板', 'HUD'], stats: ['能力', 'Stats'],
  classic: ['双栏', 'Classic'], scroll: ['银幕滚动', 'Scrolling'], card: ['终卡', 'End card'], ed: ['动画片尾', 'Anime ED'], billing: ['海报', 'Billing'], terminal: ['终端', 'Terminal'],
  slip: ['小票', 'Slip'], ticket: ['电影票', 'Ticket'], flat: ['扁平', 'Flat'], stack: ['条形', 'Stack'], emblem: ['徽记', 'Emblem'],
};
export const variantName = (v: string, lang: 'zh' | 'en') => VARIANT_NAMES[v]?.[lang === 'zh' ? 0 : 1] ?? v;

/** 渲染成可导出的作品 */
export function art(id: string, m: Model, st: Style, o: PieceOpts = {}): Art | null {
  const p = PIECES[id];
  if (!p) throw new Error(`未知作品：${id}`);
  const s = p.render(m, st, o);
  if (!s) return null;
  return { w: s.w, h: s.h, svg: s.toString(), hits: s.hits, bg: s.bg, title: `${m.project} · ${m.lang === 'en' ? p.en : p.name}`, motion: (s as any).motion };
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
