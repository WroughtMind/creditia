// 作品清单：id → 渲染函数与说明。Node 和浏览器共用。
import type { Model } from './data.ts';
import type { Style } from './style.ts';
import type { Art } from './export.ts';
import type { Scene } from './svg.ts';
import type { PieceOpts } from './pieces/common.ts';
import { tier, TIER_VARIANTS } from './pieces/tier.ts';
import { roll, ROLL_VARIANTS } from './pieces/roll.ts';
import { donut, waffle, stream, lanes, clock, versions, survival, mosaic, calendar } from './pieces/charts.ts';
import { receipt, RECEIPT_VARIANTS, share, badge, BADGE_VARIANTS, byline, hand } from './pieces/cards.ts';

export interface PieceDef {
  name: string;
  en: string;
  /** 回答的问题 */
  q: string;
  variants?: string[];
  /** 能做成动画（GIF / HTML） */
  animated?: boolean;
  render: (m: Model, st: Style, o: PieceOpts) => Scene | null;
}

export const PIECES: Record<string, PieceDef> = {
  tier: { name: '署名榜', en: 'Credits', q: '各工具写了多少', variants: TIER_VARIANTS, animated: true, render: tier },
  share: { name: '分享图', en: 'Share Card', q: '链接预览、社交媒体', animated: true, render: share },
  hand: { name: '人', en: 'People', q: '人做了多少：消息、review、人写的代码', animated: true, render: hand },
  receipt: { name: '小票', en: 'Receipt', q: '花了多少时间、会话和 token', variants: RECEIPT_VARIANTS, render: receipt },
  badge: { name: '徽章', en: 'Badge', q: 'README、个人主页', variants: BADGE_VARIANTS, render: badge },
  byline: { name: '署名行', en: 'Byline', q: '页脚、关于页', render: byline },
  roll: { name: '片尾字幕', en: 'Credits Roll', q: '谁担任了什么角色', variants: ROLL_VARIANTS, animated: true, render: roll },
  donut: { name: '环形图', en: 'Donut', q: '占比', animated: true, render: donut },
  waffle: { name: '华夫饼图', en: 'Waffle', q: '一格 1%', animated: true, render: waffle },
  stream: { name: '河流图', en: 'Stream', q: '每周新增代码', animated: true, render: stream },
  lanes: { name: '会话泳道', en: 'Sessions', q: '各工具什么时候在干活', animated: true, render: lanes },
  clock: { name: '作息打卡', en: 'Punch Card', q: '星期 × 小时', animated: true, render: clock },
  versions: { name: '版本柱状图', en: 'Releases', q: '每个版本谁写的', animated: true, render: versions },
  survival: { name: '留存', en: 'Retention', q: '写的代码现在还剩多少', animated: true, render: survival },
  mosaic: { name: '文件马赛克', en: 'File Mosaic', q: '现在的代码由谁构成', animated: true, render: mosaic },
  calendar: { name: '日历', en: 'Calendar', q: '哪天谁在写', animated: true, render: calendar },
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
  return { w: s.w, h: s.h, svg: s.toString(), hits: s.hits, bg: s.bg, title: `${m.project} · ${p.name}`, motion: (s as any).motion };
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
