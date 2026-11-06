// A scrolling SVG can contain opacity/filter groups outside its visible window.
// resvg 0.35 aborts while intersecting some of those layers with a small canvas.
// Rasterize the declared full vertical extent, then crop on integer pixel rows.
// The artwork's visible viewport and the sampling grid remain unchanged.
export interface RasterViewport { svg: string; width: number; height: number; top: number; fullHeight: number; window?: { x: number; y: number; w: number; h: number; bg: string } }
const marker = /\bdata-creditia-raster="([\d.eE+ -]+)"/;
const rootTag = /^<svg\b[^>]*>/;
export function rasterExtent(svg: string): [number, number] | undefined {
  const value = marker.exec(rootTag.exec(svg)?.[0] ?? '')?.[1].trim().split(/\s+/).map(Number);
  return value?.length === 2 && value.every(Number.isFinite) && value[1] > value[0] ? [value[0], value[1]] : undefined;
}
export function retainRasterExtent(source: string, output: string): string {
  const root = rootTag.exec(source)?.[0] ?? '';
  for (const name of ['raster', 'window', 'bg']) {
    const attr = new RegExp(`\\bdata-creditia-${name}="[^"]*"`).exec(root)?.[0];
    if (attr && !rootTag.exec(output)?.[0].includes(`data-creditia-${name}=`)) output = output.replace(rootTag, tag => tag.replace(/>$/, ` ${attr}>`));
  }
  return output;
}
export function rasterViewport(svg: string, width?: number): RasterViewport | undefined {
  const extent = rasterExtent(svg), root = rootTag.exec(svg)?.[0];
  if (!extent || !root) return;
  const w = Number(/\bwidth="([\d.]+)"/.exec(root)?.[1]), h = Number(/\bheight="([\d.]+)"/.exec(root)?.[1]);
  const box = /\bviewBox="([^"]+)"/.exec(root)?.[1].trim().split(/[ ,]+/).map(Number) ?? [0, 0, w, h];
  if (![w, h, ...box].every(Number.isFinite) || w <= 0 || h <= 0 || box.length !== 4 || box[2] <= 0 || box[3] <= 0) throw Error('Invalid scrolling viewport');
  const outW = Math.round(width ?? w), outH = Math.round(h * outW / w), scale = outW / box[2];
  // Two guard pixels keep subpixel edges and zero-sized outside layers away
  // from the native intersection boundary without moving the visible raster.
  const top = Math.ceil(Math.max(0, -extent[0]) * scale) + 2;
  const bottom = Math.ceil(Math.max(0, extent[1] - box[3]) * scale) + 2;
  const fullHeight = top + outH + bottom;
  if (outW <= 0 || outH <= 0 || outW > 32768 || fullHeight > 32768 || outW * fullHeight > 32_000_000) throw Error('Scrolling bitmap exceeds 32 million pixels; choose a smaller width');
  const alignY = (outH - h * outW / w) / 2;
  const windowBox = /\bdata-creditia-window="([^"]+)"/.exec(root)?.[1].split(/\s+/).map(Number);
  const window = windowBox?.length === 4 && windowBox.every(Number.isFinite) ? { x: windowBox[0] * scale, y: windowBox[1] * scale + alignY, w: windowBox[2] * scale, h: windowBox[3] * scale, bg: /\bdata-creditia-bg="([^"]+)"/.exec(root)?.[1] ?? 'transparent' } : undefined;
  const expanded = root.replace(/\bheight="[^"]*"/, `height="${fullHeight * w / outW}"`)
    .replace(/\bviewBox="[^"]*"/, `viewBox="${box[0]} ${box[1] - (top + alignY) / scale} ${box[2]} ${fullHeight / scale}"`);
  return { svg: (expanded + svg.slice(root.length)).replaceAll(' clip-path="url(#creditia-scroll-viewport)"', ''), width: outW, height: outH, top, fullHeight, window };
}
export function viewportPixels(pixels: Uint8Array, viewport?: RasterViewport): Uint8Array {
  if (!viewport) return pixels;
  const stride = viewport.width * 4;
  return pixels.subarray(viewport.top * stride, (viewport.top + viewport.height) * stride);
}
/** Pixel-aligned image cropping lets PNG adapters reuse their native encoder. */
export function viewportPngSvg(png: Uint8Array, viewport: RasterViewport): string {
  let binary = '';
  for (let i = 0; i < png.length; i += 0x8000) binary += String.fromCharCode(...png.subarray(i, i + 0x8000));
  const clip = viewport.window;
  const mask = clip ? `<rect width="${viewport.width}" height="${viewport.height}" fill="${clip.bg}"/><defs><clipPath id="crop"><rect x="${clip.x}" y="${clip.y}" width="${clip.w}" height="${clip.h}"/></clipPath></defs>` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${viewport.width}" height="${viewport.height}" viewBox="0 0 ${viewport.width} ${viewport.height}">${mask}<image${clip ? ' clip-path="url(#crop)"' : ''} x="0" y="${-viewport.top}" width="${viewport.width}" height="${viewport.fullHeight}" href="data:image/png;base64,${btoa(binary)}"/></svg>`;
}
