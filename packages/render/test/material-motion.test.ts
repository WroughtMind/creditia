import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildModel } from '../src/data.ts';
import { art, frame } from '../src/registry.ts';
import { STYLES } from '../src/styles/index.ts';
import { ORDERED_THEMES } from '../src/classic/catalog.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const model = buildModel(raw, { lang: 'zh', group: 'model', named: false, sign: '' });
const normalized = (svg: string) => svg.replace(/(\bid="|url\(#|\bhref="#)c[0-9a-z]+(?=-)/g, '$1scene');

test('every desktop asterisk material has finite changing samples and exact resting geometry', () => {
  const hidden = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
  const styles = [...ORDERED_THEMES.map(bridgeStyle), ...STYLES.filter(style => !hidden.has(style.id))];
  assert.equal(styles.length, 26);
  for (const style of styles) {
    const resting = art('asterisk', model, style)!;
    const samples = [0, .3, .7, 1].map(t => frame('asterisk', model, style, {}, t, resting)!);
    for (const sample of samples) {
      assert.ok(!/NaN|Infinity/.test(sample.svg), style.id);
      assert.equal(sample.w, resting.w, style.id);
      assert.equal(sample.h, resting.h, style.id);
    }
    assert.notEqual(normalized(samples[0].svg), normalized(samples[1].svg), style.id);
    assert.notEqual(normalized(samples[1].svg), normalized(samples[2].svg), style.id);
    assert.equal(normalized(samples[3].svg), normalized(resting.svg), style.id);
    assert.deepEqual(samples[3].hits, resting.hits, style.id);
  }
});

test('brush materials unfold the foreground with a stable texture seed and remove the mask at rest', () => {
  for (const id of ['ink', 'ukiyoe', 'woodblock', 'manuscript']) {
    const style = STYLES.find(style => style.id === id)!;
    const resting = art('tier', model, style, { variant: 'grouped' })!;
    const first = frame('tier', model, style, { variant: 'grouped' }, .2, resting)!;
    const later = frame('tier', model, style, { variant: 'grouped' }, .7, resting)!;
    assert.ok(first.svg.includes('material-unfold'), id);
    assert.ok(first.svg.includes('feDisplacementMap'), id);
    assert.equal(/seed="(\d+)"/.exec(first.svg)?.[1], /seed="(\d+)"/.exec(later.svg)?.[1], id);
    assert.ok(!resting.svg.includes('material-unfold'), id);
  }
});
