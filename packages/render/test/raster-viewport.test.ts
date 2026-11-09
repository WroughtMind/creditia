import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { nativeBackend, renderRequest } from '../src/native-protocol.ts';
import { raster } from '../src/index.ts';
import { sized } from '../src/export.ts';
import { rasterExtent, rasterViewport } from '../src/raster-viewport.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const unwrap = (response: any) => { assert.equal(response.ok, true, response.error?.message); return response.result; };

test('scroll viewport overscan preserves native pixels, dimensions and transparency', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 20 40 20" data-creditia-raster="-20 60"><rect x="10" y="24" width="10" height="8" fill="#cc8833" opacity=".5"/></svg>';
  const reference = svg.replace(' data-creditia-raster="-20 60"', '');
  for (const width of [40, 80, 113]) {
    const v = rasterViewport(svg, width)!;
    assert.equal(v.width, width);
    const expected = raster.rgba(reference, width), actual = raster.rgba(svg, width);
    assert.equal(actual.w, expected.w); assert.equal(actual.h, expected.h);
    assert.deepEqual(actual.data, expected.data);
    assert.deepEqual(raster.png(svg, width), raster.png(reference, width), `PNG crop ${width}`);
  }
  const art = { svg, w: 40, h: 20, title: 'Viewport', bg: 'transparent', hits: [] };
  const fitted = sized(art, 320, 240), extent = rasterExtent(fitted.svg)!;
  assert.ok(extent[0] < 0 && extent[1] > 240);
  assert.ok(rasterExtent(nativeBackend.raster.flat(fitted.svg)));
  assert.throws(() => rasterViewport(svg, 8192), /32 million/);
});

test('small sized scroll GIFs preserve material filters without native bounds aborts', { timeout: 120_000 }, async () => {
  // These five style/presentation cases aborted the complete export matrix in
  // resvg0.35 (layer intersection or displacement-map extent assertions).
  for (const [style, group, named] of [
    ['cinema', 'model', false], ['woodblock', 'model', false], ['cyber', 'model', false],
    ['meadow', 'auto', true], ['grimoire', 'auto', true],
  ] as const) {
    const selected = { version: 1, id: style, raw, renderer: 'svg', style, piece: 'roll', variant: 'scroll', lang: 'zh', group, named };
    const result = unwrap(await renderRequest({ ...selected, op: 'export', format: 'gif', size: '320x240', width: 160, frameCount: 4 }));
    const gif = Buffer.from(result.bytes, 'base64');
    assert.equal(gif.subarray(0, 3).toString(), 'GIF');
    assert.equal(gif.readUInt16LE(6), 160); assert.equal(gif.readUInt16LE(8), 120);
    const frames = unwrap(await renderRequest({ ...selected, op: 'frames', frameCount: 4 }));
    for (const frame of frames.frames) {
      const fitted = sized(frame, 320, 240), image = nativeBackend.raster.rgba(fitted.svg, 160);
      assert.equal(image.w, 160); assert.equal(image.h, 120); assert.equal(image.data.length, 160 * 120 * 4);
      // HTML contains outlined SVG frames. Re-reading those must retain the
      // scrolling window and avoid the same native clipped-layer abort.
      const outlined = nativeBackend.raster.flat(fitted.svg);
      assert.match(outlined, /data-creditia-window=/);
      assert.match(outlined, /data-creditia-bg=/);
      const decoded = nativeBackend.raster.rgba(outlined, 160);
      assert.equal(decoded.w, 160); assert.equal(decoded.h, 120);
    }
    const native = nativeBackend.raster.rgba(frames.frames.at(-1).svg, 1080);
    assert.equal(native.w, 1080); assert.equal(native.h, 608);
  }
});
