import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deflateSync } from 'node:zlib';
import { exportArtwork } from '../src/export-job.ts';
import type { Art, Raster } from '../src/export.ts';

test('PDF resolution follows the fitted viewport and A4 uses physical paper dimensions', async () => {
  const widths: number[] = [];
  const raster: Raster = {
    png() { throw new Error('unexpected PNG'); }, flat(svg) { return svg; },
    rgba(_svg, width) { widths.push(width!); return { w: 1, h: 1, data: Uint8Array.of(20, 40, 60, 255) }; },
  };
  const art: Art = { svg: '<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="10000"/>', w: 1000, h: 10000, title: 'Long roll', bg: '#fff', hits: [] };
  const deflate = (bytes: Uint8Array) => new Uint8Array(deflateSync(bytes));
  await exportArtwork(art, { format: 'pdf', size: '320x240' }, raster, deflate);
  assert.equal(widths.at(-1), 960, 'a small fitted page retains full export resolution');
  const a4 = await exportArtwork(art, { format: 'pdf', size: 'a4' }, raster, deflate);
  assert.equal(widths.at(-1), 2480);
  assert.ok(new TextDecoder().decode(a4.bytes).includes('/MediaBox [0 0 595.28 841.89]'));
});
