// 形式清单：id → 渲染函数。Node 和浏览器端共用（index.ts 给 Node 端加字体和 PNG）。
import * as F from './forms.ts';
import { ROLL_NEW, rollNew } from './rolls.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Out } from './ui.ts';
import { fetchSegs, blameSegs } from './text.ts';

export interface Ctx { lang: 'zh' | 'en'; projects?: Model[]; person?: string; blame?: any; variant?: string }

/** null 表示数据不够，不出图。variants 是可选的版式 */
export const FORMS: Record<string, { name: string; q: string; size: string; variants?: string[]; def?: (t: Theme) => string; render: (m: Model, t: Theme, ctx: Ctx) => Out | null }> = {
  tier: { name: 'Tier', q: '工具与模型占比', size: '800×自适应', render: (m, t) => F.tier(m, t) },
  share: { name: '分享图', q: '链接预览、社交媒体', size: '1200×630', render: (m, t) => F.share(m, t) },
  byline: { name: '署名行', q: '页脚、关于页', size: '1200×120', render: (m, t, c) => F.byline(m, t, c.lang) },
  receipt: { name: '小票', q: '工期、会话与 Token', size: '1080×1350', variants: F.RECEIPT_VARIANTS, def: (t) => F.RECEIPT_DEFAULT[t.family] ?? 'slip', render: (m, t, c) => F.receipt(m, t, c.variant) },
  // 原来的十一种在前，新版式（rolls.ts）排在后面
  roll: { name: '片尾字幕', q: '分工与角色表', size: '1080×1350', variants: [...F.ROLL_VARIANTS, ...ROLL_NEW], def: (t) => F.ROLL_DEFAULT[t.family] ?? 'classic', render: (m, t, c) => (ROLL_NEW.includes(c.variant ?? '') ? rollNew(m, t, c.variant!) : F.roll(m, t, c.variant)) },
  ribbon: { name: '时间织带', q: '时间线', size: '1200×500', render: (m, t) => F.ribbon(m, t) },
  mosaic: { name: '代码马赛克', q: '各文件代码归属', size: '1200×860', render: (m, t) => F.mosaic(m, t) },
  profile: { name: '个人履历卡', q: '我和 AI 一起做了什么', size: '1200×自适应', render: (m, t, c) => (c.projects?.length ? F.profile(c.person ?? m.director, c.projects, t) : null) },
  fetch: { name: '终端名片', q: 'creditia fetch', size: '1200×自适应', render: (m, t) => F.term('creditia fetch', fetchSegs(m), t, m) },
  blame: { name: '溯源', q: 'creditia blame', size: '1200×自适应', render: (m, t, c) => (c.blame ? F.term(`creditia blame ${c.blame.file.split('/').pop()}:${c.blame.line}`, blameSegs(c.blame), t, m) : null) },
  app: { name: 'App 内致谢页', q: '原生 App 的 credits', size: '780×自适应', variants: F.APP_VARIANTS, def: () => 'themed', render: (m, t, c) => F.appPage(m, t, c.variant) },
  waffle: { name: '华夫饼图', q: '百格占比', size: '1200×680', render: (m, t) => F.waffle(m, t) },
  donut: { name: '环形图', q: '代码占比', size: '1200×630', render: (m, t) => F.donut(m, t) },
  area: { name: '堆叠面积图', q: '每周新增', size: '1200×700', render: (m, t) => F.area(m, t) },
  lanes: { name: '泳道图', q: '会话时间线', size: '1200×自适应', render: (m, t) => F.lanes(m, t) },
  clock: { name: '作息打卡图', q: '活跃时段', size: '1200×600', render: (m, t) => F.clock(m, t) },
  versions: { name: '版本柱状图', q: '各版本贡献', size: '1200×720', render: (m, t) => F.versions(m, t) },
  settle: { name: '沉淀', q: '新增与留存对比', size: '1200×560', render: (m, t) => F.settle(m, t) },
};

/** 清单分组，App 和样张页都按这个顺序 */
export const GROUPS: [string, string[]][] = [
  ['一眼看懂', ['tier', 'share', 'byline']],
  ['分工与投入', ['roll', 'receipt', 'ribbon', 'mosaic', 'profile', 'fetch']],
  ['工作流集成', ['blame', 'app']],
  ['统计图', ['waffle', 'donut', 'area', 'lanes', 'clock', 'versions', 'settle']],
];
