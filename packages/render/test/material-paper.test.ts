import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Scene, measure, el, rect, circle } from '../src/svg.ts';
import { STYLES } from '../src/styles/index.ts';
import { materialEnter, activityEnter } from '../src/material-motion.ts';
import { raster } from '../src/index.ts';

const manuscript = STYLES.find(style => style.id === 'manuscript')!;

test('临时文字度量支持嵌套，异常结束不泄露收集器或改变 SVG', () => {
  const scene = new Scene(300, 200), plain = new Scene(300, 200);
  let inner: ReturnType<Scene['captureTextBounds']> = [];
  const outer = scene.captureTextBounds(() => {
    scene.text('Centered', 150, 40, { font: manuscript.f.body, size: 20, anchor: 'middle' });
    inner = scene.captureTextBounds(() => scene.text('Right', 280, 90, { font: manuscript.f.body, size: 20, anchor: 'end' }));
  });
  plain.text('Centered', 150, 40, { font: manuscript.f.body, size: 20, anchor: 'middle' });
  plain.text('Right', 280, 90, { font: manuscript.f.body, size: 20, anchor: 'end' });
  assert.equal(scene.toString(), plain.toString());
  assert.equal(outer.length, 2); assert.equal(inner.length, 1);
  assert.equal(outer[0].x + outer[0].w / 2, 150);
  assert.equal(inner[0].x + inner[0].w, 280);
  assert.throws(() => scene.captureTextBounds(() => { throw new Error('stop'); }));
  scene.text('Outside', 10, 130, { font: manuscript.f.body, size: 20 });
  assert.equal(outer.length, 2); assert.equal(inner.length, 1);
});

test('手稿批注跟随真实居中和右齐字面，不给空容器、条码或弧题字画直线', () => {
  const scene = new Scene(400, 200), k = .5;
  materialEnter(scene, manuscript, k, { x: 0, y: 0, w: 400, h: 180 }, () => {
    scene.text('48.3%', 200, 60, { font: manuscript.f.num, size: 18, anchor: 'middle' });
    scene.text('Mara', 380, 100, { font: manuscript.f.body, size: 20, anchor: 'end' });
    scene.add(el('path', { d: 'M10 120V160M15 120V160', stroke: '#111' }));
    scene.add(el('text', {}, el('textPath', { href: '#ring' }, 'A curved title')));
  });
  const svg = scene.toString(), lines = [...svg.matchAll(/<line\b[^>]+/g)].map(match => match[0]);
  assert.equal(lines.length, 2);
  const width = measure('48.3%', manuscript.f.num, 18), firstX = Number(/x1="([^"]+)"/.exec(lines[0])![1]);
  assert.ok(Math.abs(firstX - (200 - width / 2 - 1.5)) < .02, 'number annotation starts at actual centered text');
  const secondX = Number(/x1="([^"]+)"/.exec(lines[1])![1]);
  assert.ok(Math.abs(secondX - (380 - measure('Mara', manuscript.f.body, 20) - 1.5)) < .02, 'right aligned name keeps its true origin');
  assert.ok(lines.every(line => !line.includes('x1="0"')), 'container width is not a text baseline');
});

test('纸墨活动标记整枚落定，真实浓度和内部颜色不被材质阴影改变', () => {
  for (const id of ['ink', 'ukiyoe', 'woodblock', 'baroque', 'grimoire', 'manuscript']) {
    const style = STYLES.find(item => item.id === id)!;
    for (const round of [false, true]) for (const k of [.2, .5, .8]) {
      const draw = (s: Scene) => s.add(round
        ? circle(30, 30, 10, { fill: '#b32a1f', opacity: .6 })
        : rect(20, 20, 20, 20, { fill: '#b32a1f', opacity: .6 }));
      const scene = new Scene(60, 60), end = new Scene(60, 60);
      activityEnter(scene, style, k, { x: 20, y: 20, w: 20, h: 20 }, () => { draw(scene); }, round);
      draw(end);
      const middle = raster.rgba(scene.toString(), 60), final = raster.rgba(end.toString(), 60);
      for (let y = 27; y < 33; y++) for (let x = 27; x < 33; x++) {
        const at = (y * 60 + x) * 4;
        assert.deepEqual([...middle.data.slice(at, at + 4)], [...final.data.slice(at, at + 4)],
          `${id}/${round}/${k}: material contact stays outside the true activity ink`);
      }
    }
  }
});
