// 形式清单：id → 渲染函数。Node 和浏览器端共用（index.ts 给 Node 端加字体和 PNG）。
import * as F from './forms.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';
import type { Out } from './ui.ts';
import { fetchSegs, blameSegs } from './text.ts';

export interface Ctx { lang: 'zh' | 'en'; projects?: Model[]; person?: string; blame?: any; variant?: string }

/** null 表示数据不够，不出图。variants 是可选的版式 */
export const FORMS: Record<string, { name: string; q: string; size: string; variants?: string[]; render: (m: Model, t: Theme, ctx: Ctx) => Out | null }> = {
  tier: { name: 'Tier', q: '谁做的、各占多少', size: '800×自适应', render: (m, t) => F.tier(m, t) },
  share: { name: '分享图', q: '链接预览、社交媒体', size: '1200×630', render: (m, t) => F.share(m, t) },
  byline: { name: '署名行', q: '页脚、关于页', size: '1200×120', render: (m, t, c) => F.byline(m, t, c.lang) },
  receipt: { name: '精力小票', q: '花了多少精力', size: '1080×1350', variants: F.RECEIPT_VARIANTS, render: (m, t, c) => F.receipt(m, t, c.variant) },
  roll: { name: '片尾字幕', q: '谁担任了什么角色', size: '1080×1350', variants: F.ROLL_VARIANTS, render: (m, t, c) => F.roll(m, t, c.variant) },
  ribbon: { name: '时间织带', q: '怎么一步步做出来的', size: '1200×500', render: (m, t) => F.ribbon(m, t) },
  mosaic: { name: '代码马赛克', q: '现在由谁构成', size: '1200×860', render: (m, t) => F.mosaic(m, t) },
  profile: { name: '个人履历卡', q: '我和 AI 一起做了什么', size: '1200×自适应', render: (m, t, c) => (c.projects?.length ? F.profile(c.person ?? m.director, c.projects, t) : null) },
  fetch: { name: '终端名片', q: 'creditia fetch', size: '1200×自适应', render: (m, t) => F.term('creditia fetch', fetchSegs(m), t, m) },
  blame: { name: '逐行溯源', q: 'creditia blame', size: '1200×自适应', render: (m, t, c) => (c.blame ? F.term(`creditia blame ${c.blame.file.split('/').pop()}:${c.blame.line}`, blameSegs(c.blame), t, m) : null) },
  app: { name: 'App 内致谢页', q: '原生 App 的 credits', size: '780×自适应', render: (m, t) => F.appPage(m, t) },
  waffle: { name: '华夫饼图', q: '一格 1%', size: '1200×680', render: (m, t) => F.waffle(m, t) },
  donut: { name: '环形图', q: '占比', size: '1200×630', render: (m, t) => F.donut(m, t) },
  area: { name: '堆叠面积图', q: '每周新增代码', size: '1200×700', render: (m, t) => F.area(m, t) },
  lanes: { name: '泳道图', q: '各 agent 的会话', size: '1200×自适应', render: (m, t) => F.lanes(m, t) },
  clock: { name: '作息打卡图', q: '星期 × 小时', size: '1200×600', render: (m, t) => F.clock(m, t) },
  versions: { name: '版本柱状图', q: '每个版本谁做的', size: '1200×720', render: (m, t) => F.versions(m, t) },
  settle: { name: '沉淀', q: '写下的代码留下多少', size: '1200×560', render: (m, t) => F.settle(m, t) },
};

/** 清单分组，App 和样张页都按这个顺序 */
export const GROUPS: [string, string[]][] = [
  ['一眼看懂', ['tier', 'share', 'byline']],
  ['有情绪、适合传播', ['roll', 'receipt', 'ribbon', 'mosaic', 'profile', 'fetch']],
  ['融进工作流', ['blame', 'app']],
  ['统计图', ['waffle', 'donut', 'area', 'lanes', 'clock', 'versions', 'settle']],
];
