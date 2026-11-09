// SVG viewers choose fallback per glyph, whereas usvg can replace an entire
// text run. Resolve that choice with the same pinned cmap data before browser
// display or font embedding, keeping native output and selectable text aligned.
import { familiesIn, matchingFace, nativeFonts, portableFontFamilies, FONTS, type FontFace } from './fonts.ts';

export const fontKey = (face: FontFace) => `${face.pkg}/${face.dir}`;
const faceAlias = (face: FontFace) => `Creditia ${face.pkg} ${face.dir}`;
/** Each fallback alias binds one actual weight, even after other artworks have
 * loaded different weights of that family into the same browser document. */
export function portableFontBindings(svg: string): { family: string; face: FontFace }[] {
  const bindings = new Map<string, { family: string; face: FontFace }>();
  for (const tag of svg.matchAll(/<[^>]+\bfont-family="([^"]+)"[^>]*>/g)) {
    const attrs = attributes(tag[0], { family: '', weight: 400, italic: false });
    for (const family of familiesIn(attrs.family)) {
      const face = FONTS.find(face => faceAlias(face) === family) ?? matchingFace(family, attrs.weight, attrs.italic);
      if (face) bindings.set(`${family}|${fontKey(face)}`, { family, face });
    }
  }
  return [...bindings.values()];
}
export type FontBytes = ReadonlyMap<string, ArrayBuffer | Uint8Array>;
const cmaps = new WeakMap<object, (code: number) => boolean>();

/** Read Unicode cmap 4/12 directly; no DOM, platform font lookup or shaping WASM. */
export function fontHasGlyph(bytes: ArrayBuffer | Uint8Array): (code: number) => boolean {
  const cached = cmaps.get(bytes);
  if (cached) return cached;
  const view = bytes instanceof Uint8Array ? new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) : new DataView(bytes);
  let cmap = 0;
  for (let i = 0; i < view.getUint16(4); i++) {
    const at = 12 + i * 16;
    if (view.getUint32(at) === 0x636d6170) { cmap = view.getUint32(at + 8); break; }
  }
  if (!cmap) throw new Error('Packaged font has no Unicode cmap');
  let at4 = 0, at12 = 0;
  for (let i = 0; i < view.getUint16(cmap + 2); i++) {
    const at = cmap + 4 + i * 8, platform = view.getUint16(at), encoding = view.getUint16(at + 2);
    if (!(platform === 0 || platform === 3 && [1, 10].includes(encoding))) continue;
    const table = cmap + view.getUint32(at + 4), format = view.getUint16(table);
    if (format === 12) at12 = table;
    if (format === 4) at4 = table;
  }
  const has = (code: number): boolean => {
    if (at12) {
      let lo = 0, hi = view.getUint32(at12 + 12) - 1;
      while (lo <= hi) {
        const mid = (lo + hi) >>> 1, at = at12 + 16 + mid * 12;
        const start = view.getUint32(at), end = view.getUint32(at + 4);
        if (code < start) hi = mid - 1;
        else if (code > end) lo = mid + 1;
        else return view.getUint32(at + 8) + code - start !== 0;
      }
      return false;
    }
    if (!at4 || code > 0xffff) return false;
    const count = view.getUint16(at4 + 6) / 2;
    let lo = 0, hi = count - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >>> 1, end = view.getUint16(at4 + 14 + mid * 2);
      if (code > end) lo = mid + 1;
      else hi = mid - 1;
    }
    if (lo >= count || code < view.getUint16(at4 + 16 + count * 2 + lo * 2)) return false;
    const delta = view.getInt16(at4 + 16 + count * 4 + lo * 2);
    const offsetAt = at4 + 16 + count * 6 + lo * 2, offset = view.getUint16(offsetAt);
    if (!offset) return (code + delta & 0xffff) !== 0;
    const start = view.getUint16(at4 + 16 + count * 2 + lo * 2);
    const glyph = view.getUint16(offsetAt + offset + (code - start) * 2);
    return glyph !== 0 && (glyph + delta & 0xffff) !== 0;
  };
  cmaps.set(bytes, has);
  return has;
}

const decode = (s: string) => s.replace(/&(#x[\da-f]+|#\d+|amp|lt|gt|quot|apos);/gi, (_, entity: string) => {
  if (entity.startsWith('#')) return String.fromCodePoint(parseInt(entity.slice(entity[1].toLowerCase() === 'x' ? 2 : 1), entity[1].toLowerCase() === 'x' ? 16 : 10));
  return ({ amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" } as Record<string, string>)[entity.toLowerCase()];
});
const escape = (s: string) => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c]!);
interface FontAttrs { family: string; weight: number; italic: boolean }
function attributes(tag: string, parent: FontAttrs): FontAttrs {
  const family = /\bfont-family="([^"]*)"/.exec(tag)?.[1];
  const weight = /\bfont-weight="([^"]*)"/.exec(tag)?.[1];
  const style = /\bfont-style="([^"]*)"/.exec(tag)?.[1];
  return { family: family === undefined ? parent.family : decode(family),
    weight: weight === undefined ? parent.weight : weight === 'bold' ? 700 : Number(weight) || 400,
    italic: style === undefined ? parent.italic : style === 'italic' || style === 'oblique' };
}
function faceTag(tag: string, face: FontFace): string {
  const attrs = { 'font-family': `'${face.family}'`, 'font-weight': String(face.weight), 'font-style': face.italic ? 'italic' : 'normal' };
  for (const [key, value] of Object.entries(attrs)) {
    const re = new RegExp(`\\b${key}="[^"]*"`);
    tag = re.test(tag) ? tag.replace(re, `${key}="${escape(value)}"`) : tag.replace(/>$/, ` ${key}="${escape(value)}">`);
  }
  return tag;
}

/** Load nativeFonts(svg) into bytes first. Only the returned display SVG changes. */
export function portableTextFonts(input: string, bytes: FontBytes): string {
  const svg = portableFontFamilies(input), available = nativeFonts(svg);
  const has = new Map(available.map(face => {
    const data = bytes.get(fontKey(face));
    if (!data) throw new Error(`Missing portable font data: ${face.family} ${face.weight}`);
    return [face, fontHasGlyph(data)] as const;
  }));
  const resolve = (content: string, attrs: FontAttrs) => {
    const base = familiesIn(attrs.family).map(name => matchingFace(name, attrs.weight, attrs.italic)).find(Boolean)
      ?? matchingFace('Noto Sans SC', attrs.weight, attrs.italic)!;
    const primary = available.find(face => fontKey(face) === fontKey(base))!;
    const chars = [...decode(content)], codes = chars.map(c => c.codePointAt(0)!);
    const selected = codes.map(() => primary), used = new Set([primary]);
    // Native fallback first shapes the whole run; only if that still has absent
    // glyphs does it copy individual replacements into the original run.
    while (true) {
      const missing = codes.findIndex((code, i) => !has.get(selected[i])!(code));
      if (missing < 0) break;
      const fallback = available.find(face => !used.has(face) && has.get(face)!(codes[missing]));
      if (!fallback) break;
      if (codes.every(has.get(fallback)!)) return { face: fallback, content };
      for (let i = 0; i < codes.length; i++) if (!has.get(selected[i])!(codes[i]) && has.get(fallback)!(codes[i])) selected[i] = fallback;
      used.add(fallback);
    }
    if (selected.every(face => face === primary)) return { face: primary, content };
    // Keep this as one shaping run: introducing tspan boundaries changes the
    // old native shaper's cluster positions. CSS uses the same per-glyph choice
    // when the exact resolved fallback faces are placed after the primary face.
    const fallbacks = [...new Set(selected.filter(face => face !== primary))];
    return { face: primary, content, family: [primary.family, ...fallbacks.map(faceAlias)].join(', ') };
  };
  const stack: { name: string; start: number; end: number; tag: string; attrs: FontAttrs; child: boolean }[] = [];
  const edits: { start: number; end: number; value: string }[] = [];
  for (const match of svg.matchAll(/<\/?([\w:-]+)\b[^>]*>/g)) {
    const tag = match[0], start = match.index!, name = match[1];
    if (tag.startsWith('</')) {
      const node = stack.pop();
      if (node && !node.child && (name === 'text' || name === 'tspan')) {
        const normalized = resolve(svg.slice(node.end, start), node.attrs);
        edits.push({ start: node.start, end: start, value: faceTag(node.tag, normalized.face).replace(/font-family="[^"]*"/, `font-family="${escape(familiesIn(normalized.family ?? normalized.face.family).map(family => `'${family}'`).join(', '))}"`) + normalized.content });
      }
      continue;
    }
    if (stack.length) stack.at(-1)!.child = true;
    if (!tag.endsWith('/>')) stack.push({ name, start, end: start + tag.length, tag,
      attrs: attributes(tag, stack.at(-1)?.attrs ?? { family: 'Noto Sans SC', weight: 400, italic: false }), child: false });
  }
  let out = svg;
  for (const edit of edits.reverse()) out = out.slice(0, edit.start) + edit.value + out.slice(edit.end);
  // Browser synthesis would otherwise embolden a 400-only fallback when the
  // primary run is bold. Native usvg always uses the actual selected face.
  return out.replace(/<svg\b([^>]*)>/, (tag, attrs: string) => /\bstyle="/.test(attrs)
    ? tag.replace(/\bstyle="/, 'style="font-synthesis:none;text-rendering:geometricPrecision;') : tag.replace(/>$/, ' style="font-synthesis:none;text-rendering:geometricPrecision">'));
}
