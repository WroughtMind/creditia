// Independent decoders for the concrete portable formats emitted by Creditia.
// Unknown encodings fail explicitly; they are never treated as valid by magic alone.
import { inflateSync } from 'node:zlib';
import { pixelDiff } from './common.mjs';
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
function crc32(bytes) { let crc = 0xffffffff; for (const value of bytes) { crc ^= value; for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); } return (crc ^ 0xffffffff) >>> 0; }
const paeth = (a, b, c) => { const p = a + b - c, x = Math.abs(p - a), y = Math.abs(p - b), z = Math.abs(p - c); return x <= y && x <= z ? a : y <= z ? b : c; };
export function decodePng(input) {
  const bytes = Buffer.from(input);
  if (!bytes.subarray(0, 8).equals(pngSignature)) throw Error('Invalid PNG signature');
  let at = 8, header, ended = false; const compressed = [];
  while (at < bytes.length) {
    if (at + 12 > bytes.length) throw Error('Truncated PNG chunk');
    const length = bytes.readUInt32BE(at), type = bytes.toString('ascii', at + 4, at + 8), end = at + 12 + length;
    if (end > bytes.length || crc32(bytes.subarray(at + 4, end - 4)) !== bytes.readUInt32BE(end - 4)) throw Error('Invalid PNG chunk size or CRC');
    const data = bytes.subarray(at + 8, end - 4);
    if (type === 'IHDR') header = data;
    if (type === 'IDAT') compressed.push(data);
    if (type === 'IEND') { ended = true; if (end !== bytes.length) throw Error('Unexpected data after PNG end'); }
    at = end;
  }
  if (!ended || header?.length !== 13 || !compressed.length) throw Error('Incomplete PNG structure');
  const w = header.readUInt32BE(0), h = header.readUInt32BE(4), channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[header[9]];
  if (!channels || header[8] !== 8 || header[10] || header[11] || header[12]) throw Error('Expected non-interlaced 8-bit PNG');
  if (!w || !h || w * h > 32_000_000) throw Error('PNG dimensions exceed acceptance budget');
  const stride = w * channels, raw = inflateSync(Buffer.concat(compressed), { maxOutputLength: (stride + 1) * h });
  if (raw.length !== (stride + 1) * h) throw Error('PNG decoded byte count does not match dimensions');
  const image = new Uint8Array(w * h * channels), rgba = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    const filter = raw[y * (stride + 1)];
    if (filter > 4) throw Error('Unknown PNG filter');
    for (let x = 0; x < stride; x++) {
      const i = y * stride + x, a = x >= channels ? image[i - channels] : 0, b = y ? image[i - stride] : 0, c = y && x >= channels ? image[i - stride - channels] : 0;
      image[i] = (raw[y * (stride + 1) + x + 1] + [0, a, b, Math.floor((a + b) / 2), paeth(a, b, c)][filter]) & 255;
    }
  }
  for (let p = 0; p < w * h; p++) {
    const i = p * channels, j = p * 4, gray = channels < 3;
    rgba[j] = image[i]; rgba[j + 1] = image[i + (gray ? 0 : 1)]; rgba[j + 2] = image[i + (gray ? 0 : 2)];
    rgba[j + 3] = channels === 4 ? image[i + 3] : channels === 2 ? image[i + 1] : 255;
  }
  return { w, h, data: rgba };
}

export function decodePdf(input) {
  const bytes = Buffer.from(input), text = bytes.toString('latin1');
  if (!text.startsWith('%PDF-1.4\n') || !text.endsWith('%%EOF\n')) throw Error('Incomplete PDF envelope');
  const xref = /startxref\n(\d+)\n%%EOF\n$/.exec(text);
  if (!xref || text.slice(+xref[1], +xref[1] + 5) !== 'xref\n') throw Error('Invalid PDF cross-reference position');
  const table = /^xref\n0 (\d+)\n([\s\S]*?)trailer\n/.exec(text.slice(+xref[1]));
  if (!table) throw Error('Invalid PDF cross-reference table');
  const offsets = table[2].trimEnd().split('\n');
  if (offsets.length !== +table[1]) throw Error('PDF object count differs from cross-reference table');
  for (let i = 1; i < offsets.length; i++) if (!text.startsWith(`${i} 0 obj\n`, Number(offsets[i].slice(0, 10)))) throw Error(`Invalid PDF object offset ${i}`);
  const media = /\/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]/.exec(text);
  if (!media || !text.includes('/Count 1') || !text.includes('/Im0 4 0 R') || !text.includes('/Root 1 0 R')) throw Error('Expected one correctly linked PDF page');
  function object(id) {
    const start = Number(offsets[id]?.slice(0, 10)), stream = text.indexOf('stream\n', start), dictionary = text.slice(start, stream);
    const length = /\/Length (\d+)/.exec(dictionary), width = /\/Width (\d+)/.exec(dictionary), height = /\/Height (\d+)/.exec(dictionary);
    if (!length || !width || !height || !dictionary.includes('/FlateDecode') || !dictionary.includes('/BitsPerComponent 8')) throw Error('Invalid PDF image dictionary');
    const end = stream + 7 + Number(length[1]);
    if (!text.startsWith('\nendstream\nendobj', end)) throw Error('PDF stream length does not match bytes');
    const w = Number(width[1]), h = Number(height[1]);
    if (!w || !h || w * h > 32_000_000) throw Error('PDF image dimensions exceed acceptance budget');
    return { w, h, dictionary, data: inflateSync(bytes.subarray(stream + 7, end), { maxOutputLength: w * h * 4 }) };
  }
  const rgb = object(4);
  if (!rgb.dictionary.includes('/DeviceRGB') || rgb.data.length !== rgb.w * rgb.h * 3) throw Error('Invalid PDF RGB image');
  const maskId = /\/SMask (\d+) 0 R/.exec(rgb.dictionary), alpha = maskId ? object(+maskId[1]) : null;
  if (alpha && (alpha.w !== rgb.w || alpha.h !== rgb.h || alpha.data.length !== rgb.w * rgb.h || !alpha.dictionary.includes('/DeviceGray'))) throw Error('Invalid PDF alpha mask');
  const data = new Uint8Array(rgb.w * rgb.h * 4);
  for (let i = 0; i < rgb.w * rgb.h; i++) { data.set(rgb.data.subarray(i * 3, i * 3 + 3), i * 4); data[i * 4 + 3] = alpha ? alpha.data[i] : 255; }
  return { w: rgb.w, h: rgb.h, data, page_width_points: +media[1], page_height_points: +media[2], objects: offsets.length - 1 };
}

function lzw(bytes, minimum, length) {
  if (minimum < 2 || minimum > 8) throw Error('Unsupported GIF code size');
  const clear = 1 << minimum, end = clear + 1; let size, next, table, old = null, bit = 0, out = [];
  const reset = () => { table = Array.from({ length: clear }, (_, i) => [i]); size = minimum + 1; next = end + 1; old = null; };
  reset(); let ended = false;
  while (bit + size <= bytes.length * 8) {
    let code = 0; for (let i = 0; i < size; i++, bit++) code |= ((bytes[bit >> 3] >> (bit & 7)) & 1) << i;
    if (code === clear) { reset(); continue; }
    if (code === end) { ended = true; break; }
    const value = table[code] ?? (code === next && old ? [...old, old[0]] : null);
    if (!value) throw Error('Invalid GIF dictionary code');
    out.push(...value); if (out.length > length) throw Error('GIF pixels exceed frame dimensions');
    if (old && next < 4096) { table[next++] = [...old, value[0]]; if (next === 1 << size && size < 12) size++; }
    old = value;
  }
  if (!ended || out.length !== length) throw Error('Incomplete GIF image stream');
  return out;
}
export function decodeGif(input) {
  const bytes = Buffer.from(input); let at = 0;
  const take = count => { if (at + count > bytes.length) throw Error('Truncated GIF'); const part = bytes.subarray(at, at + count); at += count; return part; };
  if (!['GIF87a', 'GIF89a'].includes(take(6).toString())) throw Error('Invalid GIF signature');
  const screen = take(7), w = screen.readUInt16LE(0), h = screen.readUInt16LE(2);
  if (!w || !h || w * h > 32_000_000) throw Error('GIF dimensions exceed acceptance budget');
  const global = screen[4] & 128 ? take(3 * (2 << (screen[4] & 7))) : null;
  const blocks = () => { const data = []; while (true) { const size = take(1)[0]; if (!size) break; data.push(take(size)); } return Buffer.concat(data); };
  const frames = []; let gce = null, loop = null, ended = false;
  while (at < bytes.length) {
    const marker = take(1)[0];
    if (marker === 0x3b) { ended = true; break; }
    if (marker === 0x21) {
      const kind = take(1)[0];
      if (kind === 0xf9) { if (take(1)[0] !== 4) throw Error('Invalid GIF graphic control'); gce = take(4); if (take(1)[0] !== 0) throw Error('Invalid GIF extension terminator'); }
      else if (kind === 0xff) { const name = take(take(1)[0]).toString(); const data = blocks(); if (name === 'NETSCAPE2.0' && data[0] === 1) loop = data.readUInt16LE(1); }
      else blocks();
      continue;
    }
    if (marker !== 0x2c) throw Error('Unknown GIF block');
    const descriptor = take(9), x = descriptor.readUInt16LE(0), y = descriptor.readUInt16LE(2), fw = descriptor.readUInt16LE(4), fh = descriptor.readUInt16LE(6);
    if (x || y || fw !== w || fh !== h || descriptor[8] & 64) throw Error('Expected full-canvas non-interlaced Creditia GIF frames');
    const palette = descriptor[8] & 128 ? take(3 * (2 << (descriptor[8] & 7))) : global;
    if (!palette) throw Error('GIF frame has no color table');
    const minimum = take(1)[0], pixels = lzw(blocks(), minimum, w * h), data = new Uint8Array(w * h * 4);
    const transparent = gce && gce[0] & 1 ? gce[3] : -1;
    for (let i = 0; i < pixels.length; i++) { const color = pixels[i]; if (color * 3 + 3 > palette.length) throw Error('GIF palette index out of bounds'); data.set(palette.subarray(color * 3, color * 3 + 3), i * 4); data[i * 4 + 3] = color === transparent ? 0 : 255; }
    frames.push({ w, h, data, delay_ms: gce ? gce.readUInt16LE(1) * 10 : 0 }); gce = null;
  }
  if (!ended || at !== bytes.length || !frames.length) throw Error('Incomplete GIF trailer');
  return { w, h, frames, loop };
}
export function quantizedDiff(expected, actual) {
  if (expected.w !== actual.w || expected.h !== actual.h) return { equal: false, dimensions_equal: false };
  let alphaErrors = 0, absolute = 0, channels = 0, maximum = 0;
  for (let i = 0; i < expected.data.length; i += 4) {
    const alpha = expected.data[i + 3] < 128 ? 0 : 255;
    if (alpha !== actual.data[i + 3]) alphaErrors++;
    if (alpha) for (let c = 0; c < 3; c++) { const delta = Math.abs(expected.data[i + c] - actual.data[i + c]); absolute += delta; maximum = Math.max(maximum, delta); channels++; }
  }
  const mean = channels ? absolute / channels : 0;
  return { equal: alphaErrors === 0 && mean <= 12, dimensions_equal: true, alpha_errors: alphaErrors, mean_absolute_rgb_error: mean, max_rgb_error: maximum, maximum_allowed_mean_rgb_error: 12 };
}
export function firstSvg(html) {
  const body = html.indexOf('<body'), start = html.indexOf('<svg', body);
  if (body < 0 || start < 0) throw Error('HTML has no inline artwork');
  let depth = 0;
  for (const match of html.slice(start).matchAll(/<svg\b[^>]*>|<\/svg>/g)) {
    depth += match[0].startsWith('</') ? -1 : match[0].endsWith('/>') ? 0 : 1;
    if (!depth) return html.slice(start, start + match.index + match[0].length);
  }
  throw Error('Unclosed HTML artwork SVG');
}
export { pixelDiff };
