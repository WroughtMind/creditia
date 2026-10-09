// 渲染入口：credits.json → SVG / PNG。CLI、App 预览、批量样张都走这里。
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import satori from 'satori';
import { Resvg } from '@resvg/resvg-js';
import * as F from './forms.ts';
import { buildModel, type Model, type Options, DEFAULTS } from './data.ts';
import { THEMES, themeById, type Theme } from './themes.ts';
import type { Out } from './ui.ts';
import { fetchSegs, blameSegs } from './text.ts';

const require = createRequire(import.meta.url);
let fontsCache: any[] | null = null;
function fonts() {
  if (fontsCache) return fontsCache;
  const f = (pkg: string, file: string) => readFileSync(require.resolve(`@expo-google-fonts/${pkg}/${file}/${pkg.split('-').map((s) => s[0].toUpperCase() + s.slice(1)).join('')}_${file}.ttf`));
  fontsCache = [
    ...[[400, '400Regular'], [500, '500Medium'], [600, '600SemiBold'], [700, '700Bold']].map(([w, x]) => ({ name: 'Inter', weight: w, data: f('inter', x as string) })),
    ...[[400, '400Regular'], [700, '700Bold']].map(([w, x]) => ({ name: 'JetBrains Mono', weight: w, data: f('jetbrains-mono', x as string) })),
    { name: 'Instrument Serif', weight: 400, data: f('instrument-serif', '400Regular') },
    ...[[400, '400Regular'], [500, '500Medium']].map(([w, x]) => ({ name: 'Newsreader', weight: w, data: f('newsreader', x as string) })),
    ...[[400, '400Regular'], [500, '500Medium'], [700, '700Bold']].map(([w, x]) => ({ name: 'Noto Sans SC', weight: w, data: f('noto-sans-sc', x as string) })),
    ...[[400, '400Regular'], [500, '500Medium']].map(([w, x]) => ({ name: 'Noto Serif SC', weight: w, data: f('noto-serif-sc', x as string) })),
  ];
  return fontsCache;
}

/** 形式清单：id → 渲染函数。null 表示数据不够，不出图 */
export const FORMS: Record<string, { name: string; q: string; size: string; render: (m: Model, t: Theme, ctx: Ctx) => Out | null }> = {
  tier: { name: 'Tier', q: '谁做的、各占多少', size: '800×自适应', render: (m, t) => F.tier(m, t) },
  share: { name: '分享图', q: '链接预览、社交媒体', size: '1200×630', render: (m, t) => F.share(m, t) },
  byline: { name: '署名行', q: '页脚、关于页', size: '1200×120', render: (m, t, c) => F.byline(m, t, c.lang) },
  receipt: { name: '精力小票', q: '花了多少精力', size: '1080×1350', render: (m, t) => F.receipt(m, t) },
  roll: { name: '片尾字幕', q: '谁担任了什么角色', size: '1080×1350', render: (m, t) => F.roll(m, t) },
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

export interface Ctx { lang: 'zh' | 'en'; projects?: Model[]; person?: string; blame?: any }

export async function renderSvg(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }, size?: { w: number; h: number }): Promise<string | null> {
  const f = FORMS[form];
  if (!f) throw new Error(`未知形式：${form}`);
  const out = f.render(m, theme, ctx);
  if (!out) return null;
  return satori(out.tree, { width: out.w, height: out.h, fonts: fonts() });
}

export function svgToPng(svg: string, width?: number): Buffer {
  return new Resvg(svg, width ? { fitTo: { mode: 'width', value: width } } : undefined).render().asPng();
}

/** 社交尺寸：把形式居中放进指定画布 */
export async function renderSized(form: string, m: Model, theme: Theme, w: number, hh: number, ctx: Ctx = { lang: 'zh' }) {
  const f = FORMS[form];
  const out = f.render(m, theme, ctx);
  if (!out) return null;
  const scale = Math.min((w * 0.92) / out.w, (hh * 0.92) / out.h, 1.6);
  const bg = theme.glow ? undefined : theme.bg;
  const tree = { type: 'div', props: { style: { display: 'flex', width: w, height: hh, alignItems: 'center', justifyContent: 'center', backgroundColor: bg }, children: { type: 'div', props: { style: { display: 'flex', transform: `scale(${scale})` }, children: out.tree } } } };
  return satori(tree as any, { width: w, height: hh, fonts: fonts() });
}

export { buildModel, THEMES, themeById, DEFAULTS, F };
export type { Model, Options, Theme };
