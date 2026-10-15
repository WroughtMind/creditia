// 渲染入口：credits.json → SVG / PNG。CLI、批量样张都走这里；App 在浏览器端直接用 forms。
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import satori from 'satori';
import { Resvg } from '@resvg/resvg-js';
import * as F from './forms.ts';
import { buildModel, type Model, type Options, DEFAULTS } from './data.ts';
import { THEMES, themeById, type Theme } from './themes.ts';
import type { Out } from './ui.ts';
import { fontsFor } from './fonts.ts';
import { classicArt, classicFrames } from './art.ts';
import type { Raster } from '../export.ts';


const require = createRequire(import.meta.url);
const fontCache = new Map<string, Buffer>();
// 字体文件在包目录里找 .ttf，不靠拼文件名：各包的大小写不一致，在大小写敏感的文件系统（Linux）上拼出来的名字会找不到
function fontFile(pkg: string, file: string) {
  const k = `${pkg}/${file}`;
  if (!fontCache.has(k)) {
    const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${pkg}/package.json`)), file);
    const ttf = readdirSync(dir).find((x) => x.endsWith('.ttf') && !x.includes('Italic'));
    fontCache.set(k, readFileSync(path.join(dir, ttf!)));
  }
  return fontCache.get(k)!;
}
/** 只给这个主题用到的字体 */
function fonts(t: Theme) {
  return fontsFor(t).map((x) => ({ name: x.family, weight: x.weight, data: fontFile(x.pkg, x.dir) }));
}

export { FORMS, GROUPS } from './registry.ts';
export type { Ctx } from './registry.ts';
import { FORMS, type Ctx } from './registry.ts';

export async function renderSvg(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }, size?: { w: number; h: number }): Promise<string | null> {
  const f = FORMS[form];
  if (!f) throw new Error(`未知形式：${form}`);
  const out = f.render(m, theme, ctx);
  if (!out) return null;
  return satori(out.tree, { width: out.w, height: out.h, fonts: fonts(theme) });
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
  return satori(tree as any, { width: w, height: hh, fonts: fonts(theme) });
}

/** 栅格化：satori 出的 SVG 里文字已经是路径，resvg 不需要字体 */
const ropts = (width?: number) => ({ font: { loadSystemFonts: false }, fitTo: width ? { mode: 'width' as const, value: Math.round(width) } : undefined });
export const raster: Raster = {
  png(svg, width) { return new Uint8Array(new Resvg(svg, ropts(width)).render().asPng()); },
  rgba(svg, width) { const img = new Resvg(svg, ropts(width)).render(); return { w: img.width, h: img.height, data: new Uint8Array(img.pixels) }; },
  flat(svg) { return new Resvg(svg, ropts()).toString(); },
};

/** 统一的 Art：命令行导出、官网都走这个 */
export function art(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }) {
  return classicArt(form, m, theme, ctx, satori as any, fonts(theme));
}
/** 动画帧（GIF） */
export function frames(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }, n = 24) {
  return classicFrames(form, m, theme, ctx, satori as any, fonts(theme), n);
}
export { animatedClassic } from './art.ts';

export { buildModel, THEMES, themeById, DEFAULTS, F };
export type { Model, Options, Theme };
