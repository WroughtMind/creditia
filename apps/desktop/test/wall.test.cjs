const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

// Exercise the shipped layout with wide, tall, square and small artwork.
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const code = ts.transpile(main.slice(main.indexOf('function layout('), main.indexOf('function wireTile(')), { target: ts.ScriptTarget.ES2022 });
function wall() {
  const tiles = [
    [1200, 630, 2], [960, 1300], [900, 1800], [460, 1000], [800, 800],
    [1200, 500, 2], [900, 480], [1000, 320, 2], [320, 80, 2], [220, 60],
  ].map(([w, h, span = '']) => ({
    dataset: { w, h, span }, style: {}, classList: { contains: () => true },
  }));
  const board = { style: {}, querySelectorAll: () => tiles };
  const ctx = vm.createContext({ $: () => board, innerWidth: 1360 });
  vm.runInContext(code, ctx);
  return (width) => {
    ctx.innerWidth = width;
    ctx.layout();
    return {
      width: parseFloat(board.style.width), height: parseFloat(board.style.height),
      tiles: tiles.map((t) => ({
        x: parseFloat(t.style.left), y: parseFloat(t.style.top),
        w: parseFloat(t.style.width), h: parseFloat(t.style.width) * t.dataset.h / t.dataset.w,
      })),
    };
  };
}

test('the default window shows another artwork beside the share preview', () => {
  const scene = wall()(1360), share = scene.tiles[0];
  assert.ok(scene.tiles.slice(1).some((tile) => tile.y === share.y), 'the share preview occupies the whole first row');
});

test('shrinking the window never enlarges any preview, including across column changes', () => {
  const layout = wall();
  let previous = layout(1920);
  for (let width = 1919; width >= 390; width--) {
    const scene = layout(width);
    scene.tiles.forEach((tile, i) => assert.ok(tile.w <= previous.tiles[i].w + 1e-6,
      `artwork ${i} grew from ${previous.tiles[i].w} to ${tile.w} at window width ${width}`));
    previous = scene;
  }
});

test('reflow keeps artwork inside the board without overlaps and returns to the same layout', () => {
  const layout = wall(), original = layout(1360);
  for (const width of [1920, 1512, 1400, 1360, 1100, 1099, 960, 760, 390]) {
    const scene = layout(width);
    assert.ok(scene.width > 0 && scene.width <= width);
    for (const [i, a] of scene.tiles.entries()) {
      assert.ok(a.x >= 0 && a.x + a.w <= scene.width + 1e-6);
      assert.ok(a.y >= 0 && a.y + a.h <= scene.height);
      for (const b of scene.tiles.slice(i + 1)) {
        assert.ok(a.x + a.w <= b.x + 1e-6 || b.x + b.w <= a.x + 1e-6 || a.y + a.h <= b.y + 1e-6 || b.y + b.h <= a.y + 1e-6,
          `artwork overlaps at window width ${width}`);
      }
    }
  }
  assert.deepEqual(layout(1360), original);
});
