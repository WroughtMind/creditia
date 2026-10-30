import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, buildModel, styleById } from '../src/index.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));

test('能力雷达的填色和轮廓一起展开，中间帧保持闭合图形', () => {
  for (const t of [0.13, 0.33, 0.63, 0.88, 1]) {
    const frame = art('tier', model, styleById('manga'), { variant: 'stats', t })!;
    const polygons = [...frame.svg.matchAll(/<path\b[^>]*fill-opacity="0\.55"[^>]*\/>/g)].map((m) => m[0]);
    assert.equal(polygons.length, 4);
    for (const polygon of polygons) {
      assert.match(polygon, /d="M[^"Z]*Z"/);
      assert.match(polygon, /stroke-width="2\.4"/);
      assert.doesNotMatch(polygon, /stroke-dasharray|stroke-dashoffset|clip-path/);
    }
    assert.doesNotMatch(frame.svg, /<clipPath/);
  }
});

test('竖排榜单按完整列显现，姓名和占比不经过裁剪边界', () => {
  const final = art('tier', model, styleById('ink'), { variant: 'vertical' })!;
  const text = (svg: string) => [...svg.matchAll(/<text\b[^>]*>[\s\S]*?<\/text>/g)].map((m) => m[0]);
  for (const t of [0, 0.13, 0.33, 0.5, 0.88]) {
    const frame = art('tier', model, styleById('ink'), { variant: 'vertical', t })!;
    assert.deepEqual(text(frame.svg), text(final.svg));
    const clipped: boolean[] = [];
    for (const [tag] of frame.svg.matchAll(/<\/?g\b[^>]*>|<text\b[^>]*>/g)) {
      if (tag.startsWith('</g')) clipped.pop();
      else if (tag.startsWith('<g')) { if (!tag.endsWith('/>')) clipped.push(/clip-path=/.test(tag)); }
      else assert.ok(!clipped.some(Boolean), 'vertical name must not be clipped');
    }
    for (const hit of frame.hits) assert.deepEqual(hit, final.hits.find((h) => h.k === hit.k && h.t === hit.t));
  }
});

test('领奖台材质展开，完整名次和姓名不侵入下方的补充名单', () => {
  const st = styleById('baroque');
  const final = art('tier', model, st, { variant: 'podium' })!;
  for (const t of [0.15, 0.33, 0.5, 0.8]) {
    const frame = art('tier', model, st, { variant: 'podium', t })!;
    const stack: boolean[] = [];
    for (const [tag] of frame.svg.matchAll(/<\/?g\b[^>]*>|<text\b[^>]*>/g)) {
      if (tag.startsWith('</g')) stack.pop();
      else if (tag.startsWith('<g')) { if (!tag.endsWith('/>')) stack.push(/clip-path=/.test(tag)); }
      else assert.ok(!stack.some(Boolean), 'podium numbers and names must be complete');
    }
    for (const hit of frame.hits) {
      const end = final.hits.find((h) => h.k === hit.k && h.t === hit.t)!;
      assert.equal(hit.h, end.h);
      assert.ok(hit.y + hit.h <= end.y + end.h, hit.t);
    }
  }
});
