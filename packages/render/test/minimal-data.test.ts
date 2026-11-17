import { test } from 'node:test';
import assert from 'node:assert/strict';
import satori from 'satori';
import { animate, motion } from '../src/classic/motion.ts';
import { themeById } from '../src/classic/themes.ts';
import { raster } from '../src/index.ts';

test('极简数据网格的已落定块与新出现块共用同一坐标面，完整面积不互相覆盖', async () => {
  for (const id of ['minimal-light', 'minimal-dark']) {
    const tile = (x: number, order: number) => {
      const n = motion({ type: 'div', props: { style: { position: 'absolute', left: x, top: 30, width: 40, height: 40, backgroundColor: '#ff0000' }, children: [] } }, 'cell', order, 2);
      n.motionCue.plane = 'area'; return n;
    };
    // order=.5 exercises an entering cell while the first cell has already completed.
    const cells = [tile(40, 0), tile(80, .5)];
    if (id === 'minimal-dark') cells[0].motionCue.total = 1;
    const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 180, height: 120 }, children: cells } };
    animate(tree, .5, themeById(id)!);
    assert.equal(cells[0].props.style.transform, cells[1].props.style.transform);
    assert.equal(cells[1].props.style.opacity, 1, 'new true area is already complete, not interpolated alpha');
    const im = raster.rgba(await satori(tree as any, { width: 180, height: 120, fonts: [] }), 180);
    let painted = 0;
    for (let i = 3; i < im.data.length; i += 4) if (im.data[i]) { assert.equal(im.data[i], 255); painted++; }
    assert.equal(painted, 80 * 40, 'no overlapping cells, holes, partial area or outside halo');
    const left = id === 'minimal-dark' ? 31 : 40, top = id === 'minimal-light' ? 39 : 30;
    for (let y = top; y < top + 40; y++) for (let x = left; x < left + 80; x++) assert.equal(im.data[(y * 180 + x) * 4 + 3], 255);
  }
});

test('极简网格语义不误用到徽章标志或日期织带，静态端点保持原树', () => {
  for (const id of ['minimal-light', 'minimal-dark']) {
    const plain = motion({ type: 'div', props: { style: { width: 40, height: 40, backgroundColor: '#ff0000' }, children: [] } }, 'cell');
    animate(plain, .5, themeById(id)!);
    assert.equal(plain.props.style.transform, undefined);
    const data = motion({ type: 'div', props: { style: { width: 40, height: 40, backgroundColor: '#ff0000' }, children: [] } }, 'cell');
    data.motionCue.plane = 'count';
    const original = structuredClone(data); animate(data, 1, themeById(id)!); assert.deepEqual(data, original);
  }
});

test('极简日期织带沿非值轴成形，日期横坐标与宽度保持真实且终态不留临时光层', async () => {
  // 单列的局部进入窗口为前 18%；在窗口内检查，不能把已完成列误当中间态。
  for (const id of ['minimal-light', 'minimal-dark']) for (const time of [.02, .06, .1]) {
    const day = () => {
      const n = motion({ type: 'div', props: { style: { position: 'absolute', left: 12, top: 8, width: 12, height: 80, backgroundColor: '#336699' }, children: [] } }, 'cell');
      n.motionCue.plane = 'time'; return n;
    };
    const cell = day(), original = structuredClone(cell);
    const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 40, height: 100 }, children: [cell] } };
    animate(tree, time, themeById(id)!);
    assert.equal(cell.props.style.left, 12); assert.equal(cell.props.style.width, 12);
    assert.equal(cell.props.style.transform, undefined); assert.equal(cell.props.style.opacity, 1);
    const im = raster.rgba(await satori(tree as any, { width: 40, height: 100, fonts: [] }), 40);
    let rows = 0; const colors = new Set<string>();
    for (let y = 0; y < 100; y++) {
      const xs: number[] = [];
      for (let x = 0; x < 40; x++) if (im.data[(y * 40 + x) * 4 + 3]) xs.push(x);
      if (!xs.length) continue;
      rows++; assert.deepEqual(xs, Array.from({ length: 12 }, (_, x) => x + 12), 'each visible date cross-section retains its full true width');
      colors.add(Array.from(im.data.slice((y * 40 + 18) * 4, (y * 40 + 18) * 4 + 3)).join(','));
    }
    assert.ok(rows > 0);
    if (id === 'minimal-dark') { assert.equal(rows, 80, 'light travels on a complete date face'); assert.ok(colors.size > 2, 'the actual surface light is spatial, not a whole-column alpha change'); }
    else assert.ok(rows < 80, 'only non-value thickness is forming');
    const endpoint = day(); animate(endpoint, 1, themeById(id)!); assert.deepEqual(endpoint, original);
  }
});
