// 浏览器端导出：resvg-wasm 栅格化（PNG、PDF、GIF、文字转路径的 SVG），和 CLI 共用 export.ts。
import { initWasm, Resvg } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';
import { makeExporter, sized, SIZES, type Art, type Raster } from '../../../packages/render/src/export.ts';
import type { Style } from '../../../packages/render/src/style.ts';
import { fontBytes } from './fonts.ts';

let boot: Promise<void> | null = null;
const init = () => (boot ??= initWasm(fetch(wasmUrl)));

async function deflate(data: Uint8Array): Promise<Uint8Array> {
  const cs = new CompressionStream('deflate');
  const out = new Response(new Blob([data as BlobPart]).stream().pipeThrough(cs));
  return new Uint8Array(await out.arrayBuffer());
}

async function exporterFor(st: Style, svg: string) {
  await init();
  const fonts = await fontBytes(st, svg);
  const opts = (width?: number) => ({ font: { fontBuffers: fonts, loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' }, fitTo: width ? { mode: 'width' as const, value: Math.round(width) } : undefined });
  const raster: Raster = {
    png: (svg, w) => { const r = new Resvg(svg, opts(w)); const out = r.render().asPng(); r.free(); return out; },
    rgba: (svg, w) => { const r = new Resvg(svg, opts(w)); const img = r.render(); const res = { w: img.width, h: img.height, data: img.pixels }; r.free(); return res; },
    flat: (svg) => { const r = new Resvg(svg, opts()); const s = r.toString(); r.free(); return s; },
  };
  return makeExporter(raster, deflate);
}

export type Format = 'png' | 'svg' | 'pdf' | 'gif' | 'html';
/** 按格式出文件。frames 只在 GIF 时用 */
export async function exportArt(fmt: Format, st: Style, a: Art, size: string, frames?: () => Art[]): Promise<{ bytes: Uint8Array; mime: string; ext: string }> {
  const ex = await exporterFor(st, a.svg);
  const sz = SIZES.find((x) => x.id === size);
  const fit = (x: Art) => (sz && sz.w ? sized(x, sz.w, sz.h) : x);
  const enc = new TextEncoder();
  switch (fmt) {
    case 'png': return { bytes: ex.png(fit(a), sz?.w ? 1 : 2), mime: 'image/png', ext: 'png' };
    case 'svg': return { bytes: enc.encode(ex.svg(fit(a))), mime: 'image/svg+xml', ext: 'svg' };
    case 'pdf': return { bytes: await ex.pdf(fit(a), sz?.id === 'a4' ? 1 : Math.min(3, 4000 / Math.max(a.w, a.h))), mime: 'application/pdf', ext: 'pdf' };
    case 'html': return { bytes: enc.encode(ex.html([a], { title: a.title })), mime: 'text/html', ext: 'html' };
    case 'gif': {
      const fr = (frames ? frames() : [a]).map(fit);
      return { bytes: ex.gif(fr, { width: Math.min(fr[0].w, 800) }), mime: 'image/gif', ext: 'gif' };
    }
  }
}
