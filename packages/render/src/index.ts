// Node 端入口：CLI、样张、测试都走这里。字体从 node_modules 读，resvg-js 出 PNG / 扁平 SVG。
import { readdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { FONTS, nativeFonts } from './fonts.ts';
import { makeExporter, type Raster } from './export.ts';
import { flattenSvg } from './flatten.ts';
import { straightRgba } from './pixels.ts';
import { rasterViewport, viewportPixels, viewportPngSvg, retainRasterExtent } from './raster-viewport.ts';

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

const paths = new Map<string, string>();
function selectedFontFiles(svg: string): string[] {
  // Preserve the manifest order; fallback can replace an entire text run.
  const faces = nativeFonts(svg);
  return faces.map(face => {
    const key = `${face.pkg}/${face.dir}`;
    let file = paths.get(key);
    if (!file) {
      const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`)), face.dir);
      const name = readdirSync(dir).find(name => name.endsWith('.ttf'));
      if (!name) throw new Error(`Missing font: ${face.family} ${face.weight}`);
      file = path.join(dir, name); paths.set(key, file);
    }
    return file;
  });
}
const opts = (svg: string, width?: number) => ({
  font: { fontFiles: selectedFontFiles(svg), loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' },
  fitTo: width ? { mode: 'width', value: Math.round(width) } : undefined,
  shapeRendering: 2, textRendering: 1, imageRendering: 0,
});

export const raster: Raster = {
  png(svg, width) { const v = rasterViewport(svg, width), source = v?.svg ?? svg; const png = new Resvg(source, opts(source, width)).render().asPng(); return new Uint8Array(v ? new Resvg(viewportPngSvg(png, v), { font: { loadSystemFonts: false } }).render().asPng() : png); },
  rgba(svg, width) { const v = rasterViewport(svg, width), source = v?.svg ?? svg; let img = new Resvg(source, opts(source, width)).render(); if (v?.window) { img = new Resvg(viewportPngSvg(img.asPng(), v), { font: { loadSystemFonts: false } }).render(); return { w: img.width, h: img.height, data: straightRgba(img.pixels) }; } return { w: img.width, h: v?.height ?? img.height, data: straightRgba(viewportPixels(img.pixels, v)) }; },
  flat(svg) { return retainRasterExtent(svg, flattenSvg(svg, source => new Resvg(source, opts(source)).toString())); },
};

export const exporter = makeExporter(raster);
export * from './registry.ts';
export { buildModel, DEFAULTS, type Model, type Options } from './data.ts';
export { STYLES, styleById } from './styles/index.ts';
