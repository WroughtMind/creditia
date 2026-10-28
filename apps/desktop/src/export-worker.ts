// 栅格化和编码只在本次导出的线程中运行，和 CLI 共用格式实现。
import { initWasm, Resvg } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';
import { makeExporter, sized, SIZES, type Art, type Raster } from '../../../packages/render/src/export.ts';
import { fontBytes } from './fonts.ts';
import type { ExportJob, Exported } from './engine.ts';

/** WASM 对象不等 GC 回收；底层异常后的清理失败不能覆盖原始错误。 */
function using<T extends { free(): void }, R>(value: T, read: (value: T) => R): R {
  let failed = false;
  try { return read(value); }
  catch (error) { failed = true; throw error; }
  finally {
    if (failed) { try { value.free(); } catch { /* 整个导出线程随后被丢弃。 */ } }
    else value.free();
  }
}

async function deflate(data: Uint8Array): Promise<Uint8Array> {
  const cs = new CompressionStream('deflate');
  const out = new Response(new Blob([data as BlobPart]).stream().pipeThrough(cs));
  return new Uint8Array(await out.arrayBuffer());
}

async function run({ fmt, fontStyle, art: a, size, frames }: ExportJob): Promise<Exported> {
  await initWasm(fetch(wasmUrl));
  // 原版作品已经是路径；有文字的作品只读取本风格需要的字体。
  const fonts = fontStyle && a.svg.includes('<text') ? await fontBytes(fontStyle, a.svg) : [];
  const opts = (width?: number) => ({ font: { fontBuffers: fonts, loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' }, fitTo: width ? { mode: 'width' as const, value: Math.round(width) } : undefined });
  const raster: Raster = {
    png: (svg, w) => using(new Resvg(svg, opts(w)), r => using(r.render(), img => img.asPng())),
    rgba: (svg, w) => using(new Resvg(svg, opts(w)), r => using(r.render(), img => ({ w: img.width, h: img.height, data: img.pixels }))),
    flat: svg => using(new Resvg(svg, opts()), r => r.toString()),
  };
  const ex = makeExporter(raster, deflate);
  const sz = SIZES.find(x => x.id === size);
  const fit = (x: Art) => sz?.w ? sized(x, sz.w, sz.h) : x;
  const enc = new TextEncoder();
  switch (fmt) {
    case 'png': return { bytes: ex.png(fit(a), sz?.w ? 1 : 2), mime: 'image/png', ext: 'png' };
    case 'svg': return { bytes: enc.encode(ex.svg(fit(a))), mime: 'image/svg+xml', ext: 'svg' };
    case 'pdf': return { bytes: await ex.pdf(fit(a), sz?.id === 'a4' ? 1 : Math.min(3, 4000 / Math.max(a.w, a.h))), mime: 'application/pdf', ext: 'pdf' };
    case 'html': return { bytes: enc.encode(ex.html([a], { title: a.title })), mime: 'text/html', ext: 'html' };
    case 'gif': {
      const fr = (frames ?? [a]).map(fit);
      return { bytes: ex.gif(fr, { width: Math.min(fr[0].w, 800) }), mime: 'image/gif', ext: 'gif' };
    }
  }
}

self.onmessage = async ({ data }: MessageEvent<ExportJob>) => {
  try {
    const result = await run(data);
    self.postMessage({ result }, [result.bytes.buffer]);
  } catch (error) {
    self.postMessage({ error: { message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined } });
  }
};
