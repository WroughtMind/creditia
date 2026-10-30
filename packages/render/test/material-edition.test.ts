import { test } from 'node:test';
import assert from 'node:assert/strict';
import satori from 'satori';
import { Scene, rect } from '../src/svg.ts';
import { themeById } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';
import { animate, motion, progress } from '../src/classic/motion.ts';
import { raster } from '../src/index.ts';
import { valueBar } from '../src/pieces/value-bar.ts';

const editions = ['editorial', 'mono', 'cinema', 'playbill'];

test('细泳道的原底线在500px缩图仍有真实读时前沿，不被1px背景tile吞掉', async () => {
  for (const id of editions) {
    const guide = motion({ type: 'div', props: { style: { display: 'flex', position: 'absolute', left: 100, top: 20, width: 800, height: 1 }, children: [] } }, 'lane-rule');
    const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 1000, height: 80 }, children: [guide] } };
    animate(tree, .4, themeById(id)!);
    const frame = raster.rgba(await satori(tree as any, { width: 1000, height: 80, fonts: [] }), 500);
    let lit = 0;
    for (let y = 0; y < frame.h; y++) for (let x = 0; x < frame.w; x++) {
      if (frame.data[(y * frame.w + x) * 4 + 3] > 12) {
        lit++; assert.ok(y === 10, `${id}: paint remains on the original one-pixel baseline`);
      }
    }
    assert.ok(lit > 35, `${id}: the temporal guide must survive actual 500px rasterization`);
  }
});

test('印版与剧场数据面保留全部真值，两轴的每个份额接受相同密度', () => {
  for (const id of editions) for (const axis of ['x', 'y'] as const) for (const k of [.04, .2, .5, .8, .999]) {
    const s = new Scene(160, 160), b = { x: 20, y: 20, w: 120, h: 120 };
    valueBar(s, bridgeStyle(themeById(id)!), k, b, axis, () => {
      for (let i = 0; i < 3; i++) s.add(rect(20 + (axis === 'x' ? i * 40 : 0), 20 + (axis === 'y' ? i * 40 : 0), axis === 'x' ? 40 : 120, axis === 'y' ? 40 : 120, { fill: ['#f00', '#0f0', '#00f'][i] }));
    });
    const frame = raster.rgba(s.toString(), 160), pixel = (x: number, y: number) => frame.data.subarray((y * 160 + x) * 4, (y * 160 + x) * 4 + 4);
    for (let cross = 21; cross < 139; cross++) {
      const reference = pixel(axis === 'x' ? 30 : cross, axis === 'x' ? cross : 30)[3];
      assert.ok(reference > 0, `${id}/${axis}/${k}: complete connected face`);
      for (let i = 0; i < 3; i++) {
        const p = pixel(axis === 'x' ? 30 + i * 40 : cross, axis === 'x' ? cross : 30 + i * 40);
        assert.equal(p[3], reference, `${id}/${axis}/${k}: same density across actual value axis`);
        assert.ok(p[i] >= p[3] - 2 && p[(i + 1) % 3] < 2, `${id}: original ownership color in premultiplied pixels`);
      }
    }
  }
});

test('单色文字压印不把灰底轨道或渐变滤成黑色，节目单行不横移出边框', async () => {
  for (const id of ['mono', 'playbill']) for (const time of [.06, .15, .3]) {
    const tile = motion({ type: 'div', props: { style: { display: 'flex', position: 'absolute', left: 20, top: 30, width: 160, height: 30, backgroundColor: '#cccccc' }, children: [] } }, 'row');
    const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 200, height: 100 }, children: [tile] } };
    animate(tree, time, themeById(id)!);
    const frame = raster.rgba(await satori(tree as any, { width: 200, height: 100, fonts: [] }), 200);
    const center = (45 * 200 + 100) * 4;
    assert.ok(Math.abs(frame.data[center] * 255 / frame.data[center + 3] - 204) < 2, `${id}/${time}: gray carrier stays gray after alpha is removed`);
    if (id === 'playbill') {
      const xs = [];
      for (let y = 0; y < 100; y++) for (let x = 0; x < 200; x++) if (frame.data[(y * 200 + x) * 4 + 3]) xs.push(x);
      assert.equal(Math.min(...xs), 20); assert.equal(Math.max(...xs), 179);
    }
  }
  const cue = { kind: 'bar-y' as const, total: 4 };
  assert.ok(progress(.15, { ...cue, order: 0 }, themeById('playbill')!) > progress(.15, { ...cue, order: 1 }, themeById('playbill')!), 'versions follow their chronological axis');
});

test('单色版本柱和面积格始终连通，排线不能造出双柱或镂空的面积单位', async () => {
  for (const kind of ['bar-y', 'cell'] as const) for (const time of [.03, .08, .18]) {
    const tree = motion({ type: 'div', props: { style: { display: 'flex', width: 96, height: 80, backgroundColor: '#000' }, children: [] } }, kind);
    animate(tree, time, themeById('mono')!);
    const frame = raster.rgba(await satori(tree as any, { width: 96, height: 80, fonts: [] }), 96);
    for (let y = 1; y < 79; y++) for (let x = 1; x < 95; x++) assert.ok(frame.data[(y * 96 + x) * 4 + 3] >= 180, `${kind}/${time}: whole true face under the ink texture`);
  }
});

test('排版校样只绘制一条底线，不把透明文字行描成矩形框', async () => {
  const tile = motion({ type: 'div', props: { style: { display: 'flex', position: 'absolute', left: 20, top: 20, width: 160, height: 30 }, children: [] } }, 'row');
  const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 200, height: 80 }, children: [tile] } };
  animate(tree, .2, themeById('editorial')!);
  const frame = raster.rgba(await satori(tree as any, { width: 200, height: 80, fonts: [] }), 200), ys = new Set<number>();
  let pixels = 0;
  for (let y = 0; y < 80; y++) for (let x = 0; x < 200; x++) if (frame.data[(y * 200 + x) * 4 + 3]) { ys.add(y); pixels++; }
  assert.ok(pixels >= 150, 'the baseline is actually painted');
  assert.ok(ys.size <= 2, 'a proof baseline must not create top or vertical edges');
});
