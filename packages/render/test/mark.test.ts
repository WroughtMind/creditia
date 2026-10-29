import { test } from 'node:test';
import assert from 'node:assert/strict';
import { markPaths, sharePetals } from '../src/mark.ts';

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

test('every contributing tool keeps its own visible petal, including tiny shares after the sixth tool', () => {
  const tools = Array.from({ length: 12 }, (_, i) => ({ tool: `tool-${i}`, lines: i === 0 ? 1e9 : 1, color: `#${(i + 1).toString(16).padStart(6, '0')}` }));
  const petals = sharePetals([...tools, { tool: 'unused', lines: 0 }, { ...tools[0], lines: 1 }]);
  assert.equal(petals.length, tools.length);
  for (const options of [{}, { optical: true }]) {
    const svg = markPaths({ ...options, petals });
    assert.equal((svg.match(/<path /g) ?? []).length, tools.length);
    for (const tool of tools) assert.ok(svg.includes(`fill="${tool.color}"`), tool.tool);
    assert.doesNotMatch(svg, /NaN|Infinity/);
  }
});

test('petals stay inside their angular sectors as the number of tools grows', () => {
  for (const n of [5, 7, 12, 24]) for (const optical of [false, true]) {
    const petals = sharePetals(Array.from({ length: n }, (_, i) => ({ tool: `tool-${i}`, lines: i === 0 ? 1e9 : 1 })));
    const paths = [...markPaths({ petals, optical }).matchAll(/d="([^"]+)"/g)];
    assert.equal(paths.length, n);
    for (const [i, path] of paths.entries()) {
      const angle = -Math.PI / 2 + i * 2 * Math.PI / n;
      for (const point of path[1].slice(1, -1).split('L')) {
        const [x, y] = point.split(' ').map(Number), dx = x - 12, dy = y - 12;
        const along = dx * Math.cos(angle) + dy * Math.sin(angle);
        const across = Math.abs(-dx * Math.sin(angle) + dy * Math.cos(angle));
        assert.ok(along > 0 && across <= along * Math.tan(Math.PI / n) + 0.002, `${n} tools, petal ${i} overlaps its neighbor`);
      }
    }
  }
});
