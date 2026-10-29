import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildModel, frames, STYLES } from '../src/index.ts';
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
  assert.equal(new Set(scroll.map((a) => a.frameDelay)).size, 1);
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
