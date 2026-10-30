// Node 端入口：CLI、样张、测试都走这里。字体从 node_modules 读，resvg-js 出 PNG / 扁平 SVG。
import { readdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { FONTS } from './fonts.ts';
import { makeExporter, type Raster } from './export.ts';
import { flattenSvg } from './flatten.ts';

const require = createRequire(import.meta.url);
const { Resvg } = require('@resvg/resvg-js');

let files: string[] | null = null;
/** 全部字体文件路径。包按名字找，目录里找 .ttf，不依赖文件名大小写 */
export function fontFiles(): string[] {
  if (files) return files;
  files = [];
  for (const f of FONTS) {
    let dir: string;
    try { dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${f.pkg}/package.json`)), f.dir); }
    catch { dir = path.join(import.meta.dirname, '../node_modules/@expo-google-fonts', f.pkg, f.dir); }
    if (!existsSync(dir)) continue;
    const ttf = readdirSync(dir).find((x) => x.endsWith('.ttf'));
    if (ttf) files.push(path.join(dir, ttf));
  }
  return files;
}

const opts = (width?: number) => ({
  font: { fontFiles: fontFiles(), loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' },
  fitTo: width ? { mode: 'width', value: Math.round(width) } : undefined,
  shapeRendering: 2, textRendering: 1, imageRendering: 0,
});

export const raster: Raster = {
  png(svg, width) { return new Uint8Array(new Resvg(svg, opts(width)).render().asPng()); },
  rgba(svg, width) { const img = new Resvg(svg, opts(width)).render(); return { w: img.width, h: img.height, data: new Uint8Array(img.pixels) }; },
  flat(svg) { return flattenSvg(svg, source => new Resvg(source, opts()).toString()); },
};

export const exporter = makeExporter(raster);
export * from './registry.ts';
export { buildModel, DEFAULTS, type Model, type Options } from './data.ts';
export { STYLES, styleById } from './styles/index.ts';
