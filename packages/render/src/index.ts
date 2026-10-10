// 渲染入口：credits.json → SVG / PNG。CLI、批量样张都走这里；App 在浏览器端直接用 forms。
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import satori from 'satori';
import { Resvg } from '@resvg/resvg-js';
import * as F from './forms.ts';
import { buildModel, type Model, type Options, DEFAULTS } from './data.ts';
import { THEMES, themeById, type Theme } from './themes.ts';
import { useLogoSource } from './brand.ts';
import type { Out } from './ui.ts';

// logo 从磁盘读；App / 网页端用 useLogoSource 注册自己的来源
useLogoSource({
  svg: (name) => {
    try { return readFileSync(path.join(import.meta.dirname, '../assets/logos', `${name}.svg`), 'utf8'); } catch { return null; }
  },
});

const require = createRequire(import.meta.url);
let fontsCache: any[] | null = null;
function fonts() {
  if (fontsCache) return fontsCache;
  // prefix 用于包名与字体文件大小写不一致的包（ZCOOLXiaoWei、UnifrakturMaguntia）
  // Linux 文件系统区分大小写：按目录里的真实文件名不分大小写地找（JetBrainsMono、NotoSansSC 都和包名拼法不同）
  const f = (pkg: string, file: string, prefix?: string) => {
    const want = `${prefix ?? pkg.split('-').map((s) => s[0].toUpperCase() + s.slice(1)).join('')}_${file}.ttf`;
    const dir = path.dirname(require.resolve(`@expo-google-fonts/${pkg}/package.json`));
    const real = readdirSync(path.join(dir, file)).find((n) => n.toLowerCase() === want.toLowerCase()) ?? want;
    return readFileSync(path.join(dir, file, real));
  };
  fontsCache = [
    ...[[400, '400Regular'], [500, '500Medium'], [600, '600SemiBold'], [700, '700Bold']].map(([w, x]) => ({ name: 'Inter', weight: w, data: f('inter', x as string) })),
    ...[[400, '400Regular'], [700, '700Bold']].map(([w, x]) => ({ name: 'JetBrains Mono', weight: w, data: f('jetbrains-mono', x as string) })),
    { name: 'Instrument Serif', weight: 400, data: f('instrument-serif', '400Regular') },
    ...[[400, '400Regular'], [500, '500Medium']].map(([w, x]) => ({ name: 'Newsreader', weight: w, data: f('newsreader', x as string) })),
    ...[[400, '400Regular'], [500, '500Medium'], [700, '700Bold']].map(([w, x]) => ({ name: 'Noto Sans SC', weight: w, data: f('noto-sans-sc', x as string) })),
    ...[[400, '400Regular'], [500, '500Medium']].map(([w, x]) => ({ name: 'Noto Serif SC', weight: w, data: f('noto-serif-sc', x as string) })),
    // ── 第二轮体裁的字体 ──
    ...[[500, '500Medium'], [700, '700Bold']].map(([w, x]) => ({ name: 'Cinzel', weight: w, data: f('cinzel', x as string) })),
    { name: 'Cinzel Decorative', weight: 700, data: f('cinzel-decorative', '700Bold') },
    ...[[500, '500Medium'], [700, '700Bold'], [900, '900Black']].map(([w, x]) => ({ name: 'Playfair Display', weight: w, data: f('playfair-display', x as string) })),
    ...[[400, '400Regular'], [700, '700Bold']].map(([w, x]) => ({ name: 'Libre Baskerville', weight: w, data: f('libre-baskerville', x as string) })),
    ...[[400, '400Regular'], [600, '600SemiBold']].map(([w, x]) => ({ name: 'Oswald', weight: w, data: f('oswald', x as string) })),
    { name: 'Poiret One', weight: 400, data: f('poiret-one', '400Regular') },
    { name: 'Limelight', weight: 400, data: f('limelight', '400Regular') },
    { name: 'UnifrakturMaguntia', weight: 400, data: f('unifrakturmaguntia', '400Regular', 'UnifrakturMaguntia') },
    { name: 'VT323', weight: 400, data: f('vt323', '400Regular') },
    { name: 'Press Start 2P', weight: 400, data: f('press-start-2p', '400Regular') },
    { name: 'Monoton', weight: 400, data: f('monoton', '400Regular') },
    { name: 'Archivo Black', weight: 400, data: f('archivo-black', '400Regular') },
    { name: 'Anton', weight: 400, data: f('anton', '400Regular') },
    { name: 'ZCOOL XiaoWei', weight: 400, data: f('zcool-xiaowei', '400Regular', 'ZCOOLXiaoWei') },
    { name: 'ZCOOL QingKe HuangYou', weight: 400, data: f('zcool-qingke-huangyou', '400Regular', 'ZCOOLQingKeHuangYou') },
    { name: 'Ma Shan Zheng', weight: 400, data: f('ma-shan-zheng', '400Regular') },
  ];
  return fontsCache;
}

export { FORMS, GROUPS } from './registry.ts';
export type { Ctx } from './registry.ts';
import { FORMS, type Ctx } from './registry.ts';

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
