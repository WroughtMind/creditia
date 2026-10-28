import { test } from 'node:test';
import assert from 'node:assert/strict';
import { markPaths } from '../src/mark.ts';

test('growing and overshooting petals always produce finite SVG coordinates', () => {
  for (const options of [{}, { optical: true }, { ink: '#000000' }]) {
    for (let i = 0; i <= 1400; i++) {
      const len = i / 1000;
      const paths = markPaths({ ...options, petals: [{ color: '#D97757', len, at: 0.37 }] });
      assert.doesNotMatch(paths, /NaN|Infinity/, `length ${len}, ${JSON.stringify(options)}`);
      if (len >= 0.12) assert.match(paths, /<path /);
    }
  }
});

test('zero and negative petal lengths produce no geometry', () => {
  for (const len of [0, -0.1]) {
    assert.equal(markPaths({ petals: [{ color: '#D97757', len }] }), '');
  }
});
