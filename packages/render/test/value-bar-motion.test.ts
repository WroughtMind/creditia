import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Resvg } from '@resvg/resvg-js';
import { Scene, rect } from '../src/svg.ts';
import { STYLES } from '../src/styles/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';
import { valueBar } from '../src/pieces/value-bar.ts';

const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id))];
const lengths = [60, 40, 20];
function sample(st: typeof styles[number], axis: 'x' | 'y', k: number) {
  const s = new Scene(160, 160), b = { x: 20, y: 20, w: axis === 'x' ? 120 : 12, h: axis === 'x' ? 12 : 120 };
  valueBar(s, st, k, b, axis, () => {
    let at = 20;
    lengths.forEach((n, i) => {
      const box = axis === 'x' ? { x: at, y: 20, w: n, h: 12 } : { x: 20, y: at, w: 12, h: n };
      s.add(rect(box.x, box.y, box.w, box.h, { fill: ['#ff0000', '#00cc00', '#0000ff'][i] }));
      s.hit(box.x, box.y, box.w, box.h, { t: String(i) }); at += n;
    });
  });
  return s;
}

test('all material value bars preserve each visible segment along the measured axis', () => {
  assert.equal(styles.length, 26);
  for (const st of styles) for (const axis of ['x', 'y'] as const) for (const k of [.15, .4, .75, 1]) {
    const middle = sample(st, axis, k), final = sample(st, axis, 1);
    assert.equal(middle.hits.length, 3, `${st.id}/${axis}/${k}: every real segment is present`);
    for (let i = 0; i < 3; i++) {
      assert.equal(middle.hits[i][axis], final.hits[i][axis], `${st.id}/${axis}/${k}/${i}: real starting position`);
      assert.equal(middle.hits[i][axis === 'x' ? 'w' : 'h'], lengths[i], `${st.id}/${axis}/${k}/${i}: real measured extent`);
    }
  }
});

test('rasterized value bars retain all three true extents instead of a moving value-axis cut', () => {
  // These use moving windows or whole units; print textures have intentional internal ink holes.
  const ids = ['minimal-light', 'minimal-dark', 'glass', 'cinema', 'editorial', 'playbill', 'gala', 'terminal', 'arcade', 'cyber', 'ink', 'ukiyoe', 'pastel', 'manga', 'meadow'];
  for (const st of styles.filter(s => ids.includes(s.id))) for (const axis of ['x', 'y'] as const) for (const k of [.25, .5, .8]) {
    const pixels = new Resvg(sample(st, axis, k).toString()).render().pixels;
    let start = 20;
    lengths.forEach((n, segment) => {
      const visible: number[] = [];
      for (let p = start; p < start + n; p++) {
        let alpha = 0;
        for (let cross = 20; cross < 32; cross++) {
          const x = axis === 'x' ? p : cross, y = axis === 'x' ? cross : p;
          alpha = Math.max(alpha, pixels[(y * 160 + x) * 4 + 3]);
        }
        if (alpha > 2) visible.push(p);
      }
      assert.equal(visible[0], start, `${st.id}/${axis}/${k}/${segment}: start survives real raster mask`);
      assert.equal(visible.at(-1), start + n - 1, `${st.id}/${axis}/${k}/${segment}: end survives real raster mask`);
      start += n;
    });
  }
});
