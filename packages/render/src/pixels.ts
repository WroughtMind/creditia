/** resvg exposes premultiplied RGBA; PDF soft masks and GIF palettes require
 * straight RGB. Copy before releasing WASM images, preserving alpha exactly. */
export function straightRgba(pixels: Uint8Array): Uint8Array {
  const out = new Uint8Array(pixels);
  for (let i = 0; i < out.length; i += 4) {
    const alpha = out[i + 3];
    if (alpha === 255) continue;
    if (!alpha) { out[i] = out[i + 1] = out[i + 2] = 0; continue; }
    // Match tiny-skia's PNG demultiply operation order exactly. Multiplying
    // by (255 / alpha) moves some half-integer values across the rounding edge.
    const a = alpha / 255;
    out[i] = Math.min(255, Math.floor(out[i] / a + 0.5));
    out[i + 1] = Math.min(255, Math.floor(out[i + 1] / a + 0.5));
    out[i + 2] = Math.min(255, Math.floor(out[i + 2] / a + 0.5));
  }
  return out;
}
