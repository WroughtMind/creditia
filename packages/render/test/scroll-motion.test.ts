import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { art, frames, buildModel, STYLES } from '../src/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { makeExporter } from '../src/export.ts';
import { SCROLL_READING, scrollMotion, scrollTransforms, scrollY } from '../src/scroll-motion.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));

test('全部风格都有材质适合的滚动阅读动作，章节停驻按真实几何计算', () => {
  for (const style of [...STYLES, ...THEMES]) assert.ok(SCROLL_READING[style.id], style.id);
  const sections = [{ y: 100, h: 90 }, { y: 500, h: 180 }];
  const film = scrollMotion('cinema', 100, 200, 1000, sections, []);
  const paper = scrollMotion('ink', 100, 200, 1000, sections, []);
  const terminal = scrollMotion('terminal', 100, 200, 1000, sections, [200, 224, 278, 356, 900]);
  assert.equal(film.keys.length, 2);
  const holds = paper.keys.slice(1).flatMap((key, i) => key.y === paper.keys[i].y ? [(key.offset - paper.keys[i].offset) * paper.secs] : []);
  assert.equal(holds.length, 2);
  assert.ok(Math.abs(holds[1] / holds[0] - 2) < 1e-10, 'twice the visible paragraph height receives twice the reading pause');
  assert.deepEqual(terminal.keys.slice(1, 5).map(key => key.y), [40, 64, 118, 196]);
  for (const motion of [film, paper, terminal]) {
    assert.equal(scrollY(motion.keys, 0), 0);
    assert.equal(scrollY(motion.keys, 1), 800);
    for (let i = 1; i < motion.keys.length; i++) {
      const a = motion.keys[i - 1], b = motion.keys[i], mid = (a.offset + b.offset) / 2;
      assert.equal(scrollY(motion.keys, mid), a.easing ? a.y : (a.y + b.y) / 2);
    }
  }
});

test('GIF 每个滚动作品只栅格化一次，与 App/HTML 取同一时间线', () => {
  for (const id of ['cinema', 'ink', 'terminal']) {
    const style = STYLES.find(style => style.id === id)!;
    const full = art('roll', model, style, { variant: 'scroll' })!;
    const samples = frames('roll', model, style, { variant: 'scroll' });
    assert.equal(new Set(samples.map(frame => frame.crop!.svg)).size, 1);
    assert.equal(samples[0].crop!.y, 0);
    samples.forEach((frame, i) => assert.equal(frame.crop!.y, Math.round(scrollY(full.motion!.keys, i / (samples.length - 1)))));
    let rasterizations = 0;
    const exporter = makeExporter({ png: () => new Uint8Array(), flat: svg => svg, rgba: () => {
      rasterizations++;
      return { w: 2, h: Math.ceil(full.h * 2 / full.w), data: new Uint8Array(2 * Math.ceil(full.h * 2 / full.w) * 4).fill(255) };
    } });
    exporter.gif(samples, { width: 2 });
    assert.equal(rasterizations, 1, id);
    const html = exporter.html([full], { title: 'Scroll' });
    for (const key of scrollTransforms(full.motion!.keys, full.h)) assert.ok(html.includes(`transform:${key.transform};animation-timing-function:${key.easing ?? 'linear'}`));
    assert.match(html, /prefers-reduced-motion:reduce/);
    assert.match(html, /left:0;top:0;animation:/, 'first screen starts with readable content');
    assert.doesNotMatch(html, /linear infinite|top:100%/);
  }
});
