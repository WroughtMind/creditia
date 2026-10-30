import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, buildModel, STYLES } from '../src/index.ts';
import { ROLL_VARIANTS } from '../src/pieces/roll.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const normalized = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');
const text = (svg: string) => [...svg.matchAll(/<text\b[^>]*>(.*?)<\/text>/gs)].map(m => m[1]);

test('全部字幕版式按完整内容组进入，文字不被动画遮罩切断', () => {
  for (const variant of ROLL_VARIANTS) for (const style of STYLES) {
    const final = art('roll', model, style, { variant, t: 1 })!;
    const finalText = new Set(text(final.svg));
    const states: string[] = [];
    for (const t of [0, 0.18, 0.47, 0.83, 1]) {
      const frame = art('roll', model, style, { variant, t })!;
      assert.equal(frame.w, final.w); assert.equal(frame.h, final.h);
      assert.doesNotMatch(frame.svg, /NaN|Infinity/);
      for (const value of text(frame.svg)) assert.ok(finalText.has(value), `${variant}/${style.id}: changed text`);
      const stack: boolean[] = [];
      for (const [tag] of frame.svg.matchAll(/<\/?g\b[^>]*>|<text\b[^>]*>/g)) {
        if (tag.startsWith('</g')) stack.pop();
        else if (tag.startsWith('<g')) { if (!tag.endsWith('/>')) stack.push(/clip-path="url\(#[^"]*-(?:motion|material)-/.test(tag)); }
        else assert.ok(!stack.some(Boolean), `${variant}/${style.id}: clipped text`);
      }
      states.push(normalized(frame.svg));
    }
    if (variant === 'scroll') assert.equal(new Set(states).size, 1, `${style.id}: scroll must reuse static long image`);
    else assert.ok(new Set(states).size >= 3, `${variant}/${style.id}: no staged entrance`);
  }
});

test('银幕滚动首屏直接可读，长图不逐帧改变文字', () => {
  for (const style of STYLES) {
    const first = art('roll', model, style, { variant: 'scroll', t: 0 })!;
    const final = art('roll', model, style, { variant: 'scroll', t: 1 })!;
    const viewH = Math.round(first.w * 9 / 16);
    const initialHits = final.hits.filter(h => h.y + 34 < viewH);
    assert.ok(initialHits.length > 0, style.id);
    for (const hit of initialHits) assert.ok(first.hits.some(h => h.k === hit.k && h.t === hit.t && h.y === hit.y), `${style.id}: first screen hidden`);
    assert.deepEqual(first.hits, final.hits);
    assert.equal(normalized(first.svg), normalized(final.svg));
    assert.doesNotMatch(final.svg, /material-|visibility="hidden"/, style.id);
  }
});
