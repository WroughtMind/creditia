import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { Scene, rect } from '../src/svg.ts';
import { STYLES } from '../src/styles/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';
import { raster } from '../src/index.ts';
import { MATERIAL_MOTIONS, materialEnter, materialReveal, materialOrder } from '../src/material-motion.ts';

const modern = STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id));
const styles = [...THEMES.map(bridgeStyle), ...modern];
const b = { x: 20, y: 20, w: 180, h: 50 };
const plain = (s: Scene) => { s.add(rect(b.x, b.y, b.w, b.h, { fill: '#292322' })); s.hit(b.x, b.y, b.w, b.h, { t: 'data' }); };
const normalize = (s: string) => s.replace(/\bc[0-9a-z]+-/g, 'scene-');

test('数据轮廓从初现起保留每个归属色，不把份额统染为强调色', () => {
  for (const id of ['neon', 'mono', 'grimoire']) {
    const st = styles.find(s => s.id === id)!;
    for (const k of [.15, .25, .5]) {
      const s = new Scene(240, 100);
      materialEnter(s, st, k, b, () => {
        s.add(rect(20, 20, 86, 50, { fill: '#ff0000' }), rect(114, 20, 86, 50, { fill: '#00ff00' }));
      }, 'data');
      const { data } = raster.rgba(s.toString(), 240);
      let red = 0, green = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i + 3] < 12) continue;
        if (data[i] > data[i + 1] * 3 && data[i] > 8) red++;
        if (data[i + 1] > data[i] * 3 && data[i + 1] > 8) green++;
      }
      assert.ok(red > 20 && green > 20, `${id}/${k}: both real ownership colors must remain visible`);
    }
  }
});

test('每个可选风格有明确的材质动作，终点直接回到原始作品', () => {
  assert.deepEqual(styles.map(s => s.id).sort(), Object.keys(MATERIAL_MOTIONS).sort());
  for (const st of styles) {
    const original = new Scene(240, 100); plain(original);
    for (const fn of ['enter', 'reveal']) {
      const s = new Scene(240, 100);
      if (fn === 'enter') materialEnter(s, st, 1, b, () => plain(s));
      else materialReveal(s, st, 1, b, 'right', () => plain(s));
      assert.equal(normalize(s.toString()), normalize(original.toString()), `${st.id}/${fn}`);
      assert.deepEqual(s.hits, original.hits);
    }
  }
});

test('隐藏内容仍完成排版回调，文字进入不使用裁切，数据进入不移动命中区', () => {
  for (const st of styles) {
    const hidden = new Scene(240, 100); let called = false;
    materialEnter(hidden, st, 0, b, () => { called = true; plain(hidden); });
    assert.ok(called, st.id); assert.equal(hidden.hits.length, 0, st.id);
    const s = new Scene(240, 100);
    materialEnter(s, st, 0.45, b, () => { s.text('Claude Code 48.3%', b.x, 50, { font: st.f.body, size: 18 }); });
    assert.doesNotMatch(s.toString(), /<clipPath|clip-path=/, st.id);
    const data = new Scene(240, 100);
    materialEnter(data, st, 0.45, b, () => plain(data), 'data');
    assert.deepEqual(data.hits, [{ ...b, t: 'data' }], st.id);
  }
});

test('不同材质的实际中间像素有区别，全部风格可被导出器绘制', () => {
  const hashes = new Map<string, string>();
  for (const st of styles) {
    const samples: Buffer[] = [];
    for (const k of [0, 0.2, 0.5, 0.8, 1]) {
      // 同底色同形状，差别必须来自动作，而不是风格配色或 scene id。
      const s = new Scene(240, 150); s.add(rect(0, 0, 240, 150, { fill: '#f8f4e9' }));
      const identicalPalette = { ...st, c: { ...st.c, ink: '#292322', accent: '#956d3b' } };
      materialEnter(s, identicalPalette, k, b, () => plain(s));
      materialReveal(s, identicalPalette, k, { ...b, y: 90, h: 30 }, 'right', () => s.add(rect(20, 90, 180, 30, { fill: '#292322' })));
      const png = raster.png(s.toString(), 240);
      assert.deepEqual([...png.slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10], st.id);
      samples.push(png);
    }
    hashes.set(st.id, createHash('sha256').update(Buffer.concat(samples)).digest('hex'));
    for (const i of [0, 1, 4, 7, 12]) assert.ok(materialOrder(st, i, 13, 5) >= 0 && materialOrder(st, i, 13, 5) <= 1, st.id);
  }
  for (const family of [['terminal', 'crt', 'arcade', 'neon'], ['ink', 'ukiyoe', 'woodblock', 'cutout'], ['film', 'cinema', 'playbill', 'gala'], ['minimal-light', 'minimal-dark', 'mono', 'editorial', 'broadsheet']]) {
    assert.equal(new Set(family.map(id => hashes.get(id))).size, family.length, family.join('/'));
  }
});

test('纹理在缩小画幅仍可见，接近终帧不残留整片孔洞', () => {
  for (const id of ['film', 'broadsheet', 'woodblock', 'grimoire']) {
    const st = styles.find(s => s.id === id)!;
    const sample = (k: number, width: number) => {
      const s = new Scene(240, 100);
      materialReveal(s, st, k, b, 'right', () => plain(s));
      return raster.rgba(s.toString(), width);
    };
    for (const width of [60, 240]) {
      const mid = sample(0.45, width), near = sample(0.999, width), end = sample(1, width), scale = width / 240;
      let visible = 0;
      for (let y = Math.ceil((b.y + 4) * scale); y < Math.floor((b.y + b.h - 4) * scale); y++) {
        for (let x = Math.ceil((b.x + 4) * scale); x < Math.floor((b.x + b.w - 4) * scale); x++) {
          const alpha = (y * width + x) * 4 + 3;
          visible += mid.data[alpha] > 0 ? 1 : 0;
          assert.ok(end.data[alpha] - near.data[alpha] <= 1, `${id}/${width} retained a hole at ${x},${y}`);
        }
      }
      assert.ok(visible > 0, `${id}/${width} disappeared before its final frame`);
    }
  }
});
