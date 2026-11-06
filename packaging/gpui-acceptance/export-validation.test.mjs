import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { deflateSync } from 'node:zlib';
import { sha256 } from './common.mjs';
import { decodePng, decodePdf, decodeGif, firstSvg, quantizedDiff } from './export-validation.mjs';

test('PNG decoder reconstructs color and alpha and rejects corrupted chunks', () => {
  const bytes = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAGklEQVR4nGP4z8Dwn+E/QwMDA8N/Bkkj7xMAODkF2+hyZ00AAAAASUVORK5CYII=', 'base64');
  const image = decodePng(bytes);
  assert.deepEqual([image.w, image.h], [2, 2]);
  assert.deepEqual([...image.data], [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 25, 50, 75, 200]);
  bytes[50] ^= 1;
  assert.throws(() => decodePng(bytes), /CRC/);
});

test('GIF decoder agrees with an independent Pillow raster across dictionary width changes', () => {
  const fixture = JSON.parse(fs.readFileSync(new URL('./decoder-fixture.json', import.meta.url)));
  const bytes = Buffer.from(fixture.gif_base64, 'base64'), gif = decodeGif(bytes);
  assert.deepEqual([gif.w, gif.h, gif.frames.length, gif.loop, gif.frames[0].delay_ms], [64, 64, 1, 0, 100]);
  assert.equal(sha256(gif.frames[0].data), fixture.rgba_sha256);
  assert.throws(() => decodeGif(bytes.subarray(0, -1)), /trailer/);
});

function tinyPdf() {
  const data = deflateSync(Buffer.from([255, 0, 0, 0, 255, 0]));
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 1.50 0.75] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>',
    Buffer.concat([Buffer.from(`<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length ${data.length} >>\nstream\n`), data, Buffer.from('\nendstream')]),
    '<< /Length 0 >>\nstream\n\nendstream', '<< /Title (test) /Producer (creditia) >>',
  ];
  let content = Buffer.from('%PDF-1.4\n'); const offsets = [0];
  objects.forEach((object, i) => { offsets.push(content.length); content = Buffer.concat([content, Buffer.from(`${i + 1} 0 obj\n`), Buffer.from(object), Buffer.from('\nendobj\n')]); });
  const xref = content.length;
  return Buffer.concat([content, Buffer.from(`xref\n0 7\n0000000000 65535 f \n${offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size 7 /Root 1 0 R /Info 6 0 R >>\nstartxref\n${xref}\n%%EOF\n`)]);
}
test('PDF validates xrefs, page geometry and decoded image content', () => {
  const bytes = tinyPdf(), pdf = decodePdf(bytes);
  assert.deepEqual([pdf.w, pdf.h, pdf.page_width_points, pdf.page_height_points], [2, 1, 1.5, .75]);
  assert.deepEqual([...pdf.data], [255, 0, 0, 255, 0, 255, 0, 255]);
  const invalid = Buffer.from(bytes.toString('latin1').replace('/Count 1', '/Count 9'), 'latin1');
  assert.throws(() => decodePdf(invalid), /one correctly linked/);
});
test('HTML extracts a complete nested SVG and GIF budget cannot hide alpha changes', () => {
  const svg = '<svg width="2"><g><svg width="1"><path/></svg></g></svg>';
  assert.equal(firstSvg('<body><div>' + svg + '</div><script>const x="svg"</script></body>'), svg);
  assert.throws(() => firstSvg('<body><svg><svg></svg>'), /Unclosed/);
  const expected = { w: 1, h: 1, data: new Uint8Array([50, 60, 70, 0]) };
  assert.equal(quantizedDiff(expected, { ...expected, data: new Uint8Array([0, 0, 0, 0]) }).equal, true);
  assert.equal(quantizedDiff(expected, { ...expected, data: new Uint8Array([50, 60, 70, 255]) }).equal, false);
});
