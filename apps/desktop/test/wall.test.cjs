const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

// Exercise the shipped layout with wide, tall, square and small artwork.
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const code = ts.transpile(main.slice(main.indexOf('function layout('), main.indexOf('function wireTile(')), { target: ts.ScriptTarget.ES2022 });
function wall(artworks = [
    [1200, 630, 2], [960, 1300], [900, 1800], [460, 1000], [800, 800],
    [1200, 500, 2], [900, 480], [1000, 320, 2], [320, 80, 2], [220, 60],
  ]) {
  const tiles = artworks.map(([w, h, span = '']) => ({
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


test('later small artwork fills a wide-card gap without changing preview sizes', () => {
  // Actual catnap minimal artwork dimensions. The old eight-item lookahead
  // forced a spanning card after six deferrals, leaving a 223px internal hole.
  const dimensions = [
    [1200,630,2], [800,1016], [1080,522], [1080,1350], [1080,1350],
    [1200,630], [1080,980], [1200,500,2], [1200,680], [1200,860],
    [1200,740], [1200,700], [1400,870,2], [1200,670], [1200,934],
    [1200,694], [1200,570], [1200,620], [1200,560], [780,1124],
    [1200,724], [1280,710,2], [900,560,2], [1200,120,2],
    [380,20,2], [260,260], [420,76,2],
  ];
  const arrange = wall(dimensions);
  for (const width of [900, 1440]) {
    const scene = arrange(width);
    assert.equal(scene.tiles.length, dimensions.length);
    const columns = width === 900 ? 2 : 3, cw = width === 900 ? 300 : 360;
    for (let column = 0; column < columns; column++) {
      const x = column * (cw + 40) + cw / 2;
      const stack = scene.tiles.filter(t => t.x <= x && t.x + t.w >= x).sort((a, b) => a.y - b.y);
      stack.slice(1).forEach((tile, i) => {
        const gap = tile.y - stack[i].y - stack[i].h;
        assert.ok(gap >= 62 - 1e-6 && gap <= 80, `internal gap ${gap}px at ${width}px`);
      });
    }
    scene.tiles.forEach((tile, i) => {
      const [w, h, span = 0] = dimensions[i];
      const slots = columns >= 3 && w >= 500 ? span || (w / h > 1.7 ? 2 : 1) : 1;
      assert.equal(tile.w, Math.min(cw * slots + 40 * (slots - 1), w * (w < 500 ? 1.15 : 1)));
    });
  }
});
