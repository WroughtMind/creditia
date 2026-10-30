import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Resvg } from '@resvg/resvg-js';
import { makeExporter, sized, type Art } from '../src/export.ts';
import type { ScrollMotion } from '../src/scroll-motion.ts';

const body = '<rect width="10" height="10" fill="#ff0000"/><rect y="10" width="10" height="10" fill="#0000ff"/>';
const reel = `<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20">${body}</svg>`;
const motion: ScrollMotion = { kind: 'scroll', viewW: 10, viewH: 10, secs: 2, keys: [{ offset: 0, y: 0 }, { offset: 1, y: 10 }], material: { style: 'cutout', layers: [] } };
function sample(y: number, progress?: number): Art {
  return { w: 10, h: 10, svg: `<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 ${y} 10 10">${body}</svg>`, hits: [], bg: '#ffffff', title: 'Crop', crop: { svg: reel, w: 10, h: 20, y, motion, ...(progress === undefined ? {} : { progress }) } };
}
function exporter() {
  const calls: { width: number; height: number }[] = [];
  return { calls, ex: makeExporter({ png: () => new Uint8Array(), flat: s => s, rgba: (svg, width) => {
    const image = new Resvg(svg, { fitTo: { mode: 'width', value: width! } }).render();
    calls.push({ width: image.width, height: image.height });
    return { w: image.width, h: image.height, data: new Uint8Array(image.pixels) };
  } }) };
}

test('材质长卷保留显式 crop.y，progress 缺省或与阅读轴不同都不改裁位', () => {
  for (const progress of [undefined, 1]) {
    const frames = [0, 3, 7, 10].map(y => sample(y, progress));
    const { ex, calls } = exporter();
    const actual = ex.gif(frames, { width: 10 });
    assert.deepEqual(calls, [{ width: 30, height: 20 }], 'one full atlas rasterization');
    const expected = ex.gif(frames.map(({ crop, ...a }) => a), { width: 10 });
    assert.deepEqual(actual, expected, 'complete GIF pixels equal independent SVG viewport rendering');
  }
});

test('材质长卷与普通帧混用仍保留 CSS 留白颜色，每卷只栅格一次', () => {
  for (const color of ['#ff0000', 'rgb(255, 0, 0)', 'red', '#f00']) {
    const frames = [0, 3, 7, 10].map(y => sized({ ...sample(y), bg: color }, 100, 100, .1));
    for (const mixed of [false, true]) {
      const input = mixed ? [{ ...frames[0], crop: undefined }, ...frames] : frames;
      const { ex, calls } = exporter();
      const actual = ex.gif(input, { width: 100 });
      assert.equal(calls.filter(c => c.width === 240 && c.height === 160).length, 1, 'one full atlas even in a mixed sequence');
      assert.equal(calls.filter(c => c.width === 1 && c.height === 1).length, color.length === 7 && color[0] === '#' ? 0 : 1, 'nonhex CSS uses only one cached 1px swatch');
      const expected = ex.gif(input.map(({ crop, ...a }) => a), { width: 100 });
      assert.deepEqual(actual, expected, `${color}: final encoded pixels and padding match the original SVG renderer`);
    }
  }
});
