import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, buildModel, raster, styleById } from '../src/index.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));

test('narrow terminal bylines keep code rain within safe native raster bounds', () => {
  for (const lang of ['zh', 'en'] as const) for (const variant of ['line', 'models']) {
    const model = buildModel(raw, { lang, group: 'model', named: true, sign: '' });
    const a = art('byline', model, styleById('terminal'), { variant })!;
    assert.ok(a.svg.includes('feGaussianBlur'), 'title glow remains');
    assert.ok(a.svg.includes('DotGothic16, JetBrains Mono'), 'visible rain remains');
    for (const width of [320, a.w, a.w * 2]) {
      const img = raster.rgba(a.svg, width);
      assert.equal(img.w, width);
      assert.equal(img.h, Math.round(width * a.h / a.w));
      assert.equal(img.data.length, img.w * img.h * 4);
    }
  }
});
