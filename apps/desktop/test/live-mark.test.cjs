const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { createRequire } = require('node:module');
const ts = require('typescript');

function scene(reducedMotion) {
  const file = require.resolve('../src/live-mark.ts');
  const svg = { innerHTML: '', style: {} };
  const ctx = vm.createContext({
    exports: {}, require: createRequire(file),
    matchMedia: () => ({ matches: reducedMotion }), performance: { now: () => 0 },
    requestAnimationFrame: () => 1,
    document: { createElement: () => ({ firstElementChild: svg, classList: { toggle() {}, add() {}, remove() {} } }) },
  });
  vm.runInContext(ts.transpile(fs.readFileSync(file, 'utf8'), { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }), ctx);
  return { mark: new ctx.exports.LiveMark(), svg, brand: ctx.exports.BRAND };
}
const petals = Array.from({ length: 12 }, (_, i) => ({ color: i % 2 ? '#10A37F' : '#D97757', len: i === 0 ? 1 : 0.46 }));
const count = (svg) => (svg.innerHTML.match(/<path /g) ?? []).length;

test('live mark shows every tool and restores them after progress and error states', () => {
  const { mark, svg, brand } = scene(true);
  mark.set(petals); mark.now(); assert.equal(count(svg), 12);
  for (const mode of ['progress', 'error']) {
    mark.setMode(mode); mark.now(); assert.equal(count(svg), 5);
    mark.setMode('idle'); mark.now(); assert.equal(count(svg), 12);
  }
  mark.set(brand); mark.now(); assert.equal(count(svg), 5);
  mark.set(petals); mark.now(); assert.equal(count(svg), 12);
  mark.dispose();
});

test('growing and shrinking between five and twelve petals settles without missing or invalid paths', () => {
  const { mark, svg, brand } = scene(false);
  for (const shape of [petals, brand, petals]) {
    mark.set(shape, { grow: true });
    for (let i = 0; i < 300; i++) {
      mark.tick(1 / 60, i / 60);
      assert.doesNotMatch(svg.innerHTML, /NaN|Infinity/);
    }
    assert.equal(count(svg), shape.length);
  }
  mark.dispose();
});

test('loading uses the current petal widths after tools from the previous project fade out', () => {
  const previous = scene(false), fresh = scene(false);
  previous.mark.set(petals); previous.mark.now();
  for (const { mark, brand } of [previous, fresh]) { mark.set(brand); mark.setMode('loading'); }
  for (let i = 0; i < 300; i++) for (const { mark } of [previous, fresh]) mark.tick(1 / 60, i / 60);
  assert.equal(previous.svg.innerHTML, fresh.svg.innerHTML);
  previous.mark.dispose(); fresh.mark.dispose();
});
