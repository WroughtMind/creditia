import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deflateSync, inflateSync } from 'node:zlib';
import { raster } from '../src/index.ts';
import { raster as classicRaster } from '../src/classic/index.ts';
import { makeExporter } from '../src/export.ts';

const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="3" height="1"><rect width="1" height="1" fill="#c86814" opacity="0.5"/><rect x="1" width="1" height="1" fill="#40c4e0" opacity="0.37"/></svg>';
const expected = [199, 104, 20, 128, 65, 198, 225, 94, 0, 0, 0, 0];
test('native SVG and classic raster adapters expose straight alpha, matching PNG edge colors', () => {
  assert.deepEqual([...raster.rgba(svg).data], expected);
  assert.deepEqual([...classicRaster.rgba(svg).data], expected);
});

test('PDF stores straight RGB and a separate alpha mask without darkening edges twice', async () => {
  const art = { svg, w: 3, h: 1, title: 'Alpha edges', bg: 'transparent', hits: [] };
  const pdf = Buffer.from(await makeExporter(raster, bytes => new Uint8Array(deflateSync(bytes))).pdf(art, 1));
  const body = pdf.toString('latin1');
  const stream = (number: number) => {
    const start = body.indexOf(`${number} 0 obj\n`), headerEnd = body.indexOf('stream\n', start);
    const length = Number(/\/Length (\d+)/.exec(body.slice(start, headerEnd))![1]);
    return [...inflateSync(pdf.subarray(headerEnd + 7, headerEnd + 7 + length))];
  };
  assert.deepEqual(stream(4), [199, 104, 20, 65, 198, 225, 0, 0, 0]);
  assert.deepEqual(stream(6), [128, 94, 0]);
});

test('demultiply uses the PNG encoder rounding order at half-integer boundaries', async () => {
  const { straightRgba } = await import('../src/pixels.ts');
  assert.deepEqual([...straightRgba(new Uint8Array([11, 33, 55, 66, 7, 35, 49, 70]))], [42, 127, 212, 66, 25, 127, 178, 70]);
});
