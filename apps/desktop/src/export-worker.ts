// 栅格化和编码只在本次导出的线程中运行，和 CLI 共用格式实现。
import { initWasm, Resvg } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';
import { type Art, type Raster } from '../../../packages/render/src/export.ts';
import { fontBytes } from './fonts.ts';
import type { ExportJob, Exported } from './engine.ts';
import { flattenSvg } from '../../../packages/render/src/flatten.ts';
import { exportArtwork } from '../../../packages/render/src/export-job.ts';
import { timelineFor } from '../../../packages/render/src/timeline.ts';
import { straightRgba } from '../../../packages/render/src/pixels.ts';
import { rasterViewport, viewportPixels, viewportPngSvg, retainRasterExtent } from '../../../packages/render/src/raster-viewport.ts';

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
    png: (svg, w) => {
      const v = rasterViewport(svg, w);
      const png = using(new Resvg(v?.svg ?? svg, opts(w)), r => using(r.render(), img => img.asPng()));
      return v ? using(new Resvg(viewportPngSvg(png, v), opts()), r => using(r.render(), img => img.asPng())) : png;
    },
    rgba: (svg, w) => {
      const v = rasterViewport(svg, w);
      if (v?.window) {
        const png = using(new Resvg(v.svg, opts(w)), r => using(r.render(), img => img.asPng()));
        return using(new Resvg(viewportPngSvg(png, v), opts()), r => using(r.render(), img => ({ w: img.width, h: img.height, data: straightRgba(img.pixels) })));
      }
      return using(new Resvg(v?.svg ?? svg, opts(w)), r => using(r.render(), img => ({ w: img.width, h: v?.height ?? img.height, data: straightRgba(viewportPixels(img.pixels, v)) })));
    },
    flat: svg => retainRasterExtent(svg, flattenSvg(svg, source => using(new Resvg(source, opts()), r => r.toString()))),
  };
  const timeline = timelineFor(a.selection?.style ?? 'minimal-light', a.selection?.piece ?? '', a);
  const animation = frames?.length ? { frames, times: frames.map((frame, i) => frame.crop?.progress ?? (frames.length === 1 ? 1 : i / (frames.length - 1))), timeline } : undefined;
  return exportArtwork(a, { format: fmt, size, lang: a.selection?.lang ?? 'zh' }, raster, deflate, animation);
}

self.onmessage = async ({ data }: MessageEvent<ExportJob>) => {
  try {
    const result = await run(data);
    self.postMessage({ result }, [result.bytes.buffer]);
  } catch (error) {
    self.postMessage({ error: { message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined } });
  }
};
