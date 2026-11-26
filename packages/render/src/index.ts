// Node 端入口：CLI、样张、测试都走这里。字体从 node_modules 读，resvg-js 出 PNG / 扁平 SVG。
import { FONTS, nativeFonts } from './fonts.ts';
import { fontPath } from './font-files.ts';
import { makeExporter, type Raster } from './export.ts';
import { flattenSvg } from './flatten.ts';
import { straightRgba } from './pixels.ts';
import { rasterViewport, viewportPixels, viewportPngSvg, retainRasterExtent } from './raster-viewport.ts';
import { resvg } from './resvg.ts';

/** 全部字体文件路径。包按名字找，目录里找 .ttf，不依赖文件名大小写 */
export function fontFiles(): string[] {
  return FONTS.map(fontPath);
}

// Preserve the manifest order; fallback can replace an entire text run.
const opts = (svg: string, width?: number) => ({
  font: { fontFiles: nativeFonts(svg).map(fontPath), loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' },
  fitTo: width ? { mode: 'width', value: Math.round(width) } : undefined,
  shapeRendering: 2, textRendering: 1, imageRendering: 0,
});

export const raster: Raster = {
  png(svg, width) { const v = rasterViewport(svg, width), source = v?.svg ?? svg; const png = resvg(source, opts(source, width)).render().asPng(); return new Uint8Array(v ? resvg(viewportPngSvg(png, v), { font: { loadSystemFonts: false } }).render().asPng() : png); },
  rgba(svg, width) { const v = rasterViewport(svg, width), source = v?.svg ?? svg; let img = resvg(source, opts(source, width)).render(); if (v?.window) { img = resvg(viewportPngSvg(img.asPng(), v), { font: { loadSystemFonts: false } }).render(); return { w: img.width, h: img.height, data: straightRgba(img.pixels) }; } return { w: img.width, h: v?.height ?? img.height, data: straightRgba(viewportPixels(img.pixels, v)) }; },
  flat(svg) { return retainRasterExtent(svg, flattenSvg(svg, source => resvg(source, opts(source)).toString())); },
};

export const exporter = makeExporter(raster);
export * from './registry.ts';
export { buildModel, DEFAULTS, type Model, type Options } from './data.ts';
export { STYLES, styleById } from './styles/index.ts';
