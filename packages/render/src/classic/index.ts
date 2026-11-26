// 渲染入口：credits.json → SVG / PNG。CLI、批量样张都走这里；App 在浏览器端直接用 forms。
import { readFileSync } from 'node:fs';
import * as F from './forms.ts';
import { buildModel, type Model, type Options, DEFAULTS } from './data.ts';
import { THEMES, themeById, type Theme } from './themes.ts';
import type { Node } from './ui.ts';
import { fontsFor, type ClassicFont } from './fonts.ts';
import { closestFace } from '../fonts.ts';
import { fontPath } from '../font-files.ts';
import { classicArt, classicFrames, type Satori } from './art.ts';
import type { Raster } from '../export.ts';
import { flattenSvg } from '../flatten.ts';
import { straightRgba } from '../pixels.ts';
import { rasterViewport, viewportPixels, viewportPngSvg, retainRasterExtent } from '../raster-viewport.ts';
import { resvg } from '../resvg.ts';


const draw: Satori = async (tree, options) => (await import('satori')).default(tree, options);
const fontCache = new Map<string, Buffer>();
// Bounded by the finite, pinned CLASSIC_FONTS manifest. Keep buffer identities
// stable throughout the WASM engine's lifetime: a per-face LRU smaller than one
// theme's CJK set reloads the same fonts repeatedly during a single wall. Native
// renderer recycling after idle/cancellation releases the engine and its fonts.
// 字体文件在包目录里找 .ttf，不靠拼文件名：各包的大小写不一致，在大小写敏感的文件系统（Linux）上拼出来的名字会找不到
function fontData(face: ClassicFont): Buffer {
  const file = fontPath(face);
  let data = fontCache.get(file);
  if (!data) {
    data = readFileSync(file);
    fontCache.set(file, data);
  }
  return data;
}

/** 只给这个主题用到的字体 */
function fonts(theme: Theme, tree: Node) {
  const weights = new Set<number>();
  function visit(node: Node, inherited: number) {
    if (typeof node === 'string' || typeof node === 'number') {
      weights.add(inherited);
    } else if (Array.isArray(node)) {
      for (const child of node) visit(child, inherited);
    } else if (node?.props) {
      const weight = node.props.style?.fontWeight ?? inherited;
      visit(node.props.children, weight === 'bold' ? 700 : weight === 'normal' ? 400 : Number(weight));
    }
  }
  visit(tree, 400);
  if (!weights.size) weights.add(400);

  // Satori tries every family in this list as a glyph fallback, in order.
  // English artwork can contain CJK user text. Keep the same winning face
  // for each family and inherited weight, including families absent from CSS.
  const faces = fontsFor(theme);
  const families = Map.groupBy(faces, face => face.family);
  const selected = new Set<ClassicFont>();
  for (const family of families.values()) {
    for (const weight of weights) selected.add(closestFace(family, weight)!);
  }
  return faces.filter(face => selected.has(face))
    .map(face => ({ name: face.family, weight: face.weight, data: fontData(face) }));
}

export { FORMS, GROUPS } from './registry.ts';
export type { Ctx } from './registry.ts';
import { FORMS, type Ctx } from './registry.ts';

export async function renderSvg(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }, size?: { w: number; h: number }): Promise<string | null> {
  const f = FORMS[form];
  if (!f) throw new Error(`未知形式：${form}`);
  const out = f.render(m, theme, ctx);
  if (!out) return null;
  return draw(out.tree, { width: out.w, height: out.h, fonts: fonts(theme, out.tree) });
}

export function svgToPng(svg: string, width?: number): Buffer {
  return resvg(svg, width ? { fitTo: { mode: 'width', value: width } } : undefined).render().asPng();
}

/** 社交尺寸：把形式居中放进指定画布 */
export async function renderSized(form: string, m: Model, theme: Theme, w: number, hh: number, ctx: Ctx = { lang: 'zh' }) {
  const f = FORMS[form];
  const out = f.render(m, theme, ctx);
  if (!out) return null;
  const scale = Math.min((w * 0.92) / out.w, (hh * 0.92) / out.h, 1.6);
  const bg = theme.glow ? undefined : theme.bg;
  const tree = { type: 'div', props: { style: { display: 'flex', width: w, height: hh, alignItems: 'center', justifyContent: 'center', backgroundColor: bg }, children: { type: 'div', props: { style: { display: 'flex', transform: `scale(${scale})` }, children: out.tree } } } };
  return draw(tree, { width: w, height: hh, fonts: fonts(theme, tree) });
}

/** 栅格化：satori 出的 SVG 里文字已经是路径，resvg 不需要字体 */
const ropts = (width?: number) => ({ font: { loadSystemFonts: false }, fitTo: width ? { mode: 'width' as const, value: Math.round(width) } : undefined });
export const raster: Raster = {
  png(svg, width) { const v = rasterViewport(svg, width); const png = resvg(v?.svg ?? svg, ropts(width)).render().asPng(); return new Uint8Array(v ? resvg(viewportPngSvg(png, v), ropts()).render().asPng() : png); },
  rgba(svg, width) { const v = rasterViewport(svg, width); let img = resvg(v?.svg ?? svg, ropts(width)).render(); if (v?.window) { img = resvg(viewportPngSvg(img.asPng(), v), ropts()).render(); return { w: img.width, h: img.height, data: straightRgba(img.pixels) }; } return { w: img.width, h: v?.height ?? img.height, data: straightRgba(viewportPixels(img.pixels, v)) }; },
  flat(svg) { return retainRasterExtent(svg, flattenSvg(svg, source => resvg(source, ropts()).toString())); },
};

/** 统一的 Art：命令行导出、官网都走这个 */
export function art(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }) {
  return classicArt(form, m, theme, ctx, draw, tree => fonts(theme, tree));
}
/** 动画帧（GIF） */
export function frames(form: string, m: Model, theme: Theme, ctx: Ctx = { lang: 'zh' }, n = 24, times?: readonly number[]) {
  return classicFrames(form, m, theme, ctx, draw, tree => fonts(theme, tree), n, times);
}
export { animatedClassic } from './art.ts';

export { buildModel, THEMES, themeById, DEFAULTS, F };
export type { Model, Options, Theme };
