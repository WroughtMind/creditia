import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Resvg } from '@resvg/resvg-js';
import { scrollAtlas, createScrollCompositor } from '../src/scroll-compositor.ts';
import { scrollLayerTimings, type ScrollMotion } from '../src/scroll-motion.ts';

test('越过原画幅的装饰不能污染字组透明平面', () => {
  const paint = '<rect x="3" y="3" width="4" height="4" fill="#0000ff"/>';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0H40V10H0Z" fill="#ff0000"/>${paint}</svg>`;
  const motion: ScrollMotion = { kind: 'scroll', viewW: 10, viewH: 10, secs: 2, keys: [{ offset: 0, y: 0 }, { offset: 1, y: 0 }], material: { style: 'manga', layers: [{ x: 2, y: 2, w: 6, h: 6, role: 'line', chapter: 0, paint }] } };
  const atlas = scrollAtlas(svg, 10, 10, motion), image = new Resvg(atlas.svg).render();
  const rgba = image.pixels;
  for (let y = 0; y < 10; y++) for (let x = 0; x < 10; x++) {
    const pixel = [...rgba.subarray((y * 30 + 20 + x) * 4, (y * 30 + 20 + x + 1) * 4)];
    assert.deepEqual(pixel, x >= 3 && x < 7 && y >= 3 && y < 7 ? [0, 0, 255, 255] : [0, 0, 0, 0], `foreground ${x},${y} must only contain its declared blue ink`);
  }
});

test('章节标签和首个姓名同进场，后续姓名不能因跨阅读停驻先于首行', () => {
  const motion: ScrollMotion = { kind: 'scroll', viewW: 100, viewH: 100, secs: 10, keys: [{ offset: 0, y: 0 }, { offset: .2, y: 50 }, { offset: .6, y: 50 }, { offset: 1, y: 150 }], material: { style: 'cyber', layers: [
    { x: 0, y: 40, w: 20, h: 80, paint: 'role', role: 'role', chapter: 0 },
    { x: 30, y: 40, w: 40, h: 10, paint: 'first', role: 'line', chapter: 0 },
    { x: 30, y: 55, w: 40, h: 10, paint: 'second', role: 'line', chapter: 0 },
  ] } };
  const [role, first, second] = scrollLayerTimings(motion);
  assert.equal(role.start, first.start);
  assert.ok(second.start! >= first.start!, 'later names must not become visible before the first name');
  assert.deepEqual(motion.keys, [{ offset: 0, y: 0 }, { offset: .2, y: 50 }, { offset: .6, y: 50 }, { offset: 1, y: 150 }]);
});


test('透明纸面使用straight RGBA，半透明墨色叠加和转开不会二次变暗', () => {
  const w = 20, h = 20, data = new Uint8Array(w * 3 * h * 4);
  const motion: ScrollMotion = { kind: 'scroll', viewW: w, viewH: h, secs: 2, keys: [{ offset: 0, y: 0 }, { offset: 1, y: 0 }], material: { style: 'ink', layers: [] } };
  const layer = { x: 1, y: 1, w: 18, h: 18, paint: '', role: 'line' as const, chapter: 0, start: 0, duration: 2 };
  for (let y = 1; y < 19; y++) for (let x = 1; x < 19; x++) {
    data.set([0, 0, 200, 128], (y * w * 3 + w + x) * 4);
    data.set([200, 0, 0, 128], (y * w * 3 + 2 * w + x) * 4);
  }
  const pose = () => ({ dx: 0, dy: 0, opacity: 1 });
  const atlas = { originalW: w, originalH: h, layers: [layer] };
  const composited = createScrollCompositor({ w: w * 3, h, data }, atlas, motion, () => 0, pose).render(.5);
  assert.deepEqual([...composited.data.subarray((10 * w + 10) * 4, (10 * w + 10) * 4 + 4)], [134, 0, 66, 192], 'straight source-over of half-red onto half-blue');
  // A rotated translucent red word remains red even at bilinearly sampled edges.
  for (let y = 0; y < h; y++) data.fill(0, (y * w * 3 + w) * 4, (y * w * 3 + 2 * w) * 4);
  motion.material!.style = 'meadow';
  const rotated = createScrollCompositor({ w: w * 3, h, data }, atlas, motion, () => 0, pose).render(.25);
  let seen = 0;
  for (let p = 0; p < rotated.data.length; p += 4) if (rotated.data[p + 3] > 0) { seen++; assert.deepEqual([...rotated.data.subarray(p, p + 3)], [200, 0, 0]); }
  assert.ok(seen > 0);
});
