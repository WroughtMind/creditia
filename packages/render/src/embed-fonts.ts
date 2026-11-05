// Node font embedding. Text remains selectable; all referenced font bytes travel
// inside the SVG/HTML, so opening the result never fetches a font or local path.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { nativeFonts, portableFontFamilies, type FontFace } from './fonts.ts';
import { portableTextFonts, portableFontBindings, fontKey } from './portable-text.ts';
const require = createRequire(import.meta.url);
const cached = new Map<string, string>();
let cacheBytes = 0;
const buffers = new Map<string, Uint8Array>();
let bufferBytes = 0;
function fontData(face: FontFace): Uint8Array {
  const key = fontKey(face);
  let bytes = buffers.get(key);
  if (!bytes) {
    const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`)), face.dir);
    const file = readdirSync(dir).find(file => file.endsWith('.ttf'));
    if (!file) throw new Error(`Missing embedded font: ${face.family} ${face.weight}`);
    bytes = readFileSync(path.join(dir, file));
    while (bufferBytes + bytes.length > 24 * 1024 * 1024 && buffers.size) {
      const first = buffers.keys().next().value!; bufferBytes -= buffers.get(first)!.length; buffers.delete(first);
    }
    if (bytes.length <= 24 * 1024 * 1024) { buffers.set(key, bytes); bufferBytes += bytes.length; }
  }
  return bytes;
}

export function embeddedFonts(svg: string): string {
  svg = portableFontFamilies(svg);
  const data = new Map(nativeFonts(svg).map(face => [fontKey(face), fontData(face)]));
  if (!data.size) return svg;
  svg = portableTextFonts(svg, data);
  const css = portableFontBindings(svg).map(({ family, face }) => {
    const key = `${face.pkg}/${face.dir}`;
    let data = cached.get(key);
    if (!data) {
      data = Buffer.from(fontData(face)).toString('base64');
      while (cacheBytes + data.length > 48 * 1024 * 1024 && cached.size) {
        const first = cached.keys().next().value!; cacheBytes -= cached.get(first)!.length; cached.delete(first);
      }
      if (data.length <= 48 * 1024 * 1024) { cached.set(key, data); cacheBytes += data.length; }
    }
    return `@font-face{font-family:'${family}';font-weight:${face.weight};font-style:${face.italic ? 'italic' : 'normal'};src:url(data:font/ttf;base64,${data}) format('truetype')}`;
  }).join('');
  return svg.replace(/^(<svg\b[^>]*>)/, `$1<style>${css}</style>`);
}
