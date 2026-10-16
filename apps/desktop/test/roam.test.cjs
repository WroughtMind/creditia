const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

// Run the shipped placement/growth functions with cheap SVGs, without booting Tauri.
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const code = ts.transpile(main.slice(main.indexOf('type RItem ='), main.indexOf('/** 墙上那件作品借用')), { target: ts.ScriptTarget.ES2022 });
function scene(missing) {
  const calls = [];
  const node = () => ({ style: {}, querySelector: node, appendChild() {}, remove() {} });
  const plane = node();
  let ctx;
  const host = { console, innerWidth: 1000, innerHeight: 700,
    store: { get: () => 'roam' }, tiles: () => Array.from({ length: 25 }, (_, i) => ({ id: String(i) })),
    keyOf: (b) => b.id, curVariant: () => undefined,
    state: { style: { variants: () => [], pieceName: (id) => id } },
    drawArt: async (id) => {
      calls.push(id);
      // The old bug never yields to a timer; stop it deterministically to report failure.
      if (calls.length === 500) vm.runInContext('roam.gen++; roam.ready = false', ctx);
      return missing(id) ? null : { w: 500, h: 400, svg: '<svg/>', hits: [] };
    },
    $: () => plane, document: { createElement: node }, esc: String, L: (s) => s, mountHits() {},
  };
  ctx = vm.createContext(host);
  vm.runInContext(code + '\nglobalThis.roam = roam; globalThis.grow = grow;', ctx);
  ctx.roam.ready = true;
  return { ctx, calls };
}
const settled = () => new Promise(setImmediate);

test('an entirely empty column yields instead of retrying forever', async () => {
  const { ctx, calls } = scene(() => true);
  await ctx.grow(8, -700, 1400);
  await settled();
  assert.ok(calls.length < 500, 'empty artwork repeatedly restarted the growth loop');
  assert.equal(ctx.roam.gen, 0);
  const before = calls.length;
  await settled();
  assert.equal(calls.length, before);
});

test('missing first artwork is skipped and later artwork fills the viewport', async () => {
  const { ctx, calls } = scene((id) => id === '8');
  await ctx.grow(8, -700, 1400);
  await settled();
  assert.ok(calls.length < 500);
  const items = ctx.roam.cols.get(8).items;
  assert.ok(items.length > 0);
  assert.equal(items.find((x) => x.r >= 0).r, 1);
  assert.equal(new Set(items.map((x) => x.r)).size, items.length);
  assert.ok(items[0].y - 30 <= -700);
  assert.ok(items.at(-1).y + items.at(-1).h >= 1400);
});

test('skipping artwork in either direction preserves existing tile identities', async () => {
  const { ctx } = scene((id) => id === '8');
  await ctx.grow(3, -700, 1400);
  await settled();
  const col = ctx.roam.cols.get(3);
  for (const item of col.items) assert.equal(String(((3 + item.r * 5) % 25 + 25) % 25), item.id);
  const before = col.items.map((x) => [x, x.r]);
  await ctx.grow(3, -4000, 5000);
  await settled();
  for (const [item, row] of before) assert.equal(item.r, row);
  assert.equal(new Set(col.items.map((x) => x.r)).size, col.items.length);
});
