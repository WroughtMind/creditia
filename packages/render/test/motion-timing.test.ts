import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, buildModel, frames, STYLES } from '../src/index.ts';
import { makeExporter, type Art } from '../src/export.ts';
import { motionDuration } from '../src/motion.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));

test('作品的采样数量不会改变原定播放时长，滚动字幕遵循自己的阅读时间', () => {
  const style = STYLES.find((s) => s.id === 'ink')!;
  for (const n of [4, 8]) {
    const result = frames('asterisk', model, style, {}, n);
    assert.equal(result.slice(0, -1).reduce((ms, frame) => ms + frame.frameDelay!, 0), motionDuration('asterisk', undefined, 'ink'));
  }
  const scroll = frames('roll', model, style, { variant: 'scroll' }, 4);
  assert.ok(scroll[0].frameDelay! > 0);
  assert.ok(scroll[0].crop);
  const still = art('roll', model, style, { variant: 'scroll' })!;
  assert.ok(Math.abs(scroll.slice(0, -1).reduce((ms, frame) => ms + frame.frameDelay!, 0) - still.motion!.secs * 1000) < 1e-7);
});

test('GIF 使用作品节奏，明确传入 fps 时仍尊重调用方的选择', () => {
  const exporter = makeExporter({
    rgba: () => ({ w: 1, h: 1, data: new Uint8Array([20, 40, 60, 255]) }),
    flat: (svg) => svg,
    png: () => new Uint8Array(),
  });
  const art: Art = { w: 1, h: 1, svg: '', hits: [], bg: '#fff', title: 'Timing', frameDelay: 160 };
  const delay = (bytes: Uint8Array) => {
    const at = bytes.findIndex((value, i) => value === 0x21 && bytes[i + 1] === 0xf9 && bytes[i + 2] === 4);
    assert.ok(at >= 0, 'GIF Graphic Control Extension');
    return (bytes[at + 4] + bytes[at + 5] * 256) * 10;
  };
  assert.equal(delay(exporter.gif([art, art])), 160);
  assert.equal(delay(exporter.gif([art, art], { fps: 25 })), 40);
});

test('同卷只栅格化一次，不同长卷和普通帧的混合仍能导出', () => {
  const sampled: string[] = [];
  const exporter = makeExporter({
    rgba: (svg) => { sampled.push(svg); return { w: 2, h: 4, data: new Uint8Array(Array.from({ length: 8 }, (_, i) => [i * 20, 40, 60, 255]).flat()) }; },
    flat: svg => svg, png: () => new Uint8Array(),
  });
  const sample: Art = { w: 2, h: 2, svg: 'viewport A', hits: [], bg: '#fff', title: 'Scroll', crop: { svg: 'reel A', w: 2, h: 4, y: 0 } };
  exporter.gif([sample, { ...sample, svg: 'viewport B', crop: { svg: 'reel B', w: 2, h: 4, y: 2 } }]);
  assert.deepEqual(sampled, ['viewport A', 'viewport B']);
  sampled.length = 0;
  exporter.gif([{ ...sample, crop: undefined }, sample]);
  assert.deepEqual(sampled, ['viewport A', 'viewport A']);
  sampled.length = 0;
  exporter.gif([sample, { ...sample, crop: { ...sample.crop!, y: 2 } }]);
  assert.deepEqual(sampled, ['reel A']);
});

test('GIF 实际编码裁窗像素，不重复长卷首屏；调整画幅保留留白且只栅格化一次', async () => {
  const { Resvg } = await import('@resvg/resvg-js');
  const { sized } = await import('../src/export.ts');
  const reel = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="10" fill="#ff0000"/><rect y="10" width="10" height="10" fill="#0000ff"/></svg>';
  const samples: Art[] = [0, 3, 7, 10].map(y => ({ w: 10, h: 10, svg: `<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 ${y} 10 10"><rect width="10" height="10" fill="#ff0000"/><rect y="10" width="10" height="10" fill="#0000ff"/></svg>`, hits: [], bg: '#ffffff', title: 'Crop', crop: { svg: reel, w: 10, h: 20, y } }));
  let calls = 0;
  const ex = makeExporter({ png: () => new Uint8Array(), flat: s => s, rgba: (svg, width) => {
    calls++;
    const r = new Resvg(svg, { fitTo: { mode: 'width', value: width! } }).render();
    return { w: r.width, h: r.height, data: new Uint8Array(r.pixels) };
  } });
  for (const fr of [samples, samples.map(a => sized(a, 100, 100, .1)), samples.map(a => sized(a, 100, 200, .1))]) {
    calls = 0;
    const optimized = ex.gif(fr, { width: fr[0].w });
    assert.equal(calls, 1);
    const direct = ex.gif(fr.map(({ crop, ...a }) => a), { width: fr[0].w });
    assert.deepEqual(optimized, direct, 'decoded viewport colors and letterbox match independent full SVG renders');
  }
});
