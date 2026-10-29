import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, buildModel, STYLES } from '../src/index.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const shapes = (svg: string) => [...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<path\b[^>]*\bd="([^"]+)"/g)].map((m) => m[1]);

test('比例环和时间河流揭示真实图形，中间帧不移动扇区或改变历史波峰', () => {
  for (const style of STYLES) for (const [id, variant] of [['donut', 'ring'], ['donut', 'nested'], ['stream', undefined]]) {
    if (style.cells) continue;
    const full = art(id!, model, style, { variant })!;
    for (const t of [0, 0.2, 0.6]) {
      const frame = art(id!, model, style, { variant, t })!;
      assert.deepEqual(shapes(frame.svg), shapes(full.svg), `${style.id}/${id}/${variant}/${t}`);
      assert.equal(frame.w, full.w);
      assert.equal(frame.h, full.h);
      assert.deepEqual(frame.hits, full.hits);
    }
  }
});

test('日历中已出现的日期沿用最终活动强度，不把淡入程度当成活跃量', () => {
  const cells = (svg: string) => new Map([...svg.matchAll(/<rect x="([^"]+)" y="([^"]+)"[^>]*opacity="([^"]+)"[^>]*\/>/g)].map((m) => [`${m[1]},${m[2]}`, m[3]]));
  for (const style of STYLES) {
    const full = cells(art('calendar', model, style)!.svg);
    const halfway = cells(art('calendar', model, style, { t: 0.5 })!.svg);
    assert.ok(halfway.size < full.size, style.id);
    for (const [position, opacity] of halfway) assert.equal(opacity, full.get(position), `${style.id}/${position}`);
  }
});

test('泳道的所有工具共用时间扫描边界，时间段和热区保持完整', () => {
  const style = STYLES[0];
  const full = art('lanes', model, style)!;
  const halfway = art('lanes', model, style, { t: 0.5 })!;
  assert.deepEqual(halfway.hits, full.hits);
  const widths = [...halfway.svg.matchAll(/<clipPath[^>]*>\s*<rect[^>]*width="([^"]+)"/g)].map((m) => m[1]);
  assert.ok(widths.length > 1);
  assert.equal(new Set(widths).size, 1);
  const segments = (svg: string) => [...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect\b[^>]*stroke-width="1\.4"[^>]*>/g)].map((m) => m[0]);
  assert.deepEqual(segments(halfway.svg), segments(full.svg));
});
