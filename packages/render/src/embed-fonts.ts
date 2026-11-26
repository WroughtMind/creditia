// Node font embedding. Text remains selectable; all referenced font bytes travel
// inside the SVG/HTML, so opening the result never fetches a font or local path.
import { readFileSync } from 'node:fs';
import { nativeFonts, portableFontFamilies } from './fonts.ts';
import { fontPath } from './font-files.ts';
import { portableTextFonts, portableFontBindings, fontKey } from './portable-text.ts';
const cached = new Map<string, string>();
let cacheBytes = 0;
const buffers = new Map<string, Buffer>();
let bufferBytes = 0;
function fontData(file: string): Buffer {
  let bytes = buffers.get(file);
  if (!bytes) {
    bytes = readFileSync(file);
    while (bufferBytes + bytes.length > 24 * 1024 * 1024 && buffers.size) {
      const first = buffers.keys().next().value!; bufferBytes -= buffers.get(first)!.length; buffers.delete(first);
    }
    if (bytes.length <= 24 * 1024 * 1024) { buffers.set(file, bytes); bufferBytes += bytes.length; }
  }
  return bytes;
}

export function embeddedFonts(svg: string): string {
  svg = portableFontFamilies(svg);
  const data = new Map(nativeFonts(svg).map(face => [fontKey(face), fontData(fontPath(face))]));
  if (!data.size) return svg;
  svg = portableTextFonts(svg, data);
  const css = portableFontBindings(svg).map(({ family, face }) => {
    const file = fontPath(face);
    let data = cached.get(file);
    if (!data) {
      data = fontData(file).toString('base64');
      while (cacheBytes + data.length > 48 * 1024 * 1024 && cached.size) {
        const first = cached.keys().next().value!; cacheBytes -= cached.get(first)!.length; cached.delete(first);
      }
      if (data.length <= 48 * 1024 * 1024) { cached.set(file, data); cacheBytes += data.length; }
    }
    const otf = file.endsWith('.otf');
    return `@font-face{font-family:'${family}';font-weight:${face.weight};font-style:${face.italic ? 'italic' : 'normal'};src:url(data:font/${otf ? 'otf' : 'ttf'};base64,${data}) format('${otf ? 'opentype' : 'truetype'}')}`;
  }).join('');
  return svg.replace(/^(<svg\b[^>]*>)/, `$1<style>${css}</style>`);
}
