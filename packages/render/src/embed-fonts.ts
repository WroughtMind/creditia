// Node font embedding. Text remains selectable; all referenced font bytes travel
// inside the SVG/HTML, so opening the result never fetches a font or local path.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { referencedFonts, portableFontFamilies } from './fonts.ts';
const require = createRequire(import.meta.url);
const cached = new Map<string, string>();
let cacheBytes = 0;

export function embeddedFonts(svg: string): string {
  svg = portableFontFamilies(svg);
  const refs = referencedFonts(svg);
  if (!refs.length) return svg;
  const css = refs.map(face => {
    const key = `${face.pkg}/${face.dir}`;
    let data = cached.get(key);
    if (!data) {
      const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`)), face.dir);
      const file = readdirSync(dir).find(file => file.endsWith('.ttf'));
      if (!file) throw new Error(`Missing embedded font: ${face.family} ${face.weight}`);
      data = readFileSync(path.join(dir, file)).toString('base64');
      while (cacheBytes + data.length > 48 * 1024 * 1024 && cached.size) {
        const first = cached.keys().next().value!; cacheBytes -= cached.get(first)!.length; cached.delete(first);
      }
      if (data.length <= 48 * 1024 * 1024) { cached.set(key, data); cacheBytes += data.length; }
    }
    return `@font-face{font-family:'${face.family}';font-weight:${face.weight};font-style:${face.italic ? 'italic' : 'normal'};src:url(data:font/ttf;base64,${data}) format('truetype')}`;
  }).join('');
  return svg.replace(/^(<svg\b[^>]*>)/, `$1<style>${css}</style>`);
}
