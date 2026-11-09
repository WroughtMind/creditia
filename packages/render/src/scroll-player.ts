import { scrollPaperPose, scrollPaperTexture } from './scroll-paper.ts';
import { scrollY, type ScrollMotion } from './scroll-motion.ts';
import { createScrollCompositor, type ScrollAtlas } from './scroll-compositor.ts';

/** 一次解码、一次 RGBA 读取。之后每帧只写可见窗；App、预览和离线 HTML 共用。 */
export async function prepareScrollCanvas(blob: Blob, atlas: Pick<ScrollAtlas, 'originalW' | 'originalH' | 'layers'>, motion: ScrollMotion, width: number, signal?: AbortSignal, make = createScrollCompositor, positionAt = scrollY, paperPose = scrollPaperPose, paperTexture = scrollPaperTexture) {
  const began = performance.now(), url = URL.createObjectURL(blob), image = new Image();
  const abort = () => { image.src = ''; };
  signal?.addEventListener('abort', abort, { once: true });
  let staging: HTMLCanvasElement | undefined;
  try {
    image.src = url; await image.decode();
    const decoded = performance.now();
    if (signal?.aborted) throw new DOMException('Scroll cancelled', 'AbortError');
    const w = Math.max(1, Math.round(width));
    staging = document.createElement('canvas'); staging.width = w * 3;
    staging.height = Math.round(image.naturalHeight * staging.width / image.naturalWidth);
    const sourceContext = staging.getContext('2d', { willReadFrequently: true })!;
    sourceContext.drawImage(image, 0, 0, staging.width, staging.height);
    const rgba = sourceContext.getImageData(0, 0, staging.width, staging.height);
    const compositor = make({ w: staging.width, h: staging.height, data: rgba.data }, atlas, motion, positionAt, paperPose, paperTexture);
    staging.width = staging.height = 0;
    const canvas = document.createElement('canvas'); canvas.width = compositor.width; canvas.height = compositor.height;
    canvas.style.cssText = 'display:block;width:100%;height:100%';
    const context = canvas.getContext('2d')!;
    const frame = new ImageData(compositor.render(0).data as Uint8ClampedArray, compositor.width, compositor.height);
    context.putImageData(frame, 0, 0);
    let destroyed = false;
    const metrics = { rgbaReads: 1, pixelWidth: w, sourceBytes: compositor.sourceBytes, frameBytes: compositor.frameBytes, decodeMs: decoded - began, rasterAndPrepareMs: performance.now() - decoded, frames: 0, paintTotalMs: 0, paintMaxMs: 0 };
    return {
      canvas, metrics, rgbaReads: 1, sourceBytes: compositor.sourceBytes, frameBytes: compositor.frameBytes,
      paint(progress: number) { if (!destroyed) { const start = performance.now(); compositor.render(progress); context.putImageData(frame, 0, 0); const elapsed = performance.now() - start; metrics.frames++; metrics.paintTotalMs += elapsed; metrics.paintMaxMs = Math.max(metrics.paintMaxMs, elapsed); } },
      destroy() { destroyed = true; canvas.remove(); canvas.width = canvas.height = 0; },
    };
  } finally {
    signal?.removeEventListener('abort', abort); URL.revokeObjectURL(url); image.src = '';
    if (staging) staging.width = staging.height = 0;
  }
}
