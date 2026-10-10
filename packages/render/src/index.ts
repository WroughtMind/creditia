// 渲染入口：credits.json → SVG / PNG。CLI、批量样张都走这里；App 在浏览器端直接用 forms。
import { readFileSync } from 'node:fs';
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
