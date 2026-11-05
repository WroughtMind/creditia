/** resvg exposes premultiplied RGBA; PDF soft masks and GIF palettes require
 * straight RGB. Copy before releasing WASM images, preserving alpha exactly. */
export function straightRgba(pixels: Uint8Array): Uint8Array {
  const out = new Uint8Array(pixels);
  for (let i = 0; i < out.length; i += 4) {
    const alpha = out[i + 3];
    if (alpha === 255) continue;
    if (!alpha) { out[i] = out[i + 1] = out[i + 2] = 0; continue; }
    const scale = 255 / alpha;
    out[i] = Math.min(255, Math.round(out[i] * scale));
    out[i + 1] = Math.min(255, Math.round(out[i + 1] * scale));
    out[i + 2] = Math.min(255, Math.round(out[i + 2] * scale));
  }
  return out;
}
