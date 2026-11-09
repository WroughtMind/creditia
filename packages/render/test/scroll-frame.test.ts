import { test } from 'node:test';
import assert from 'node:assert/strict';
import { frame } from '../src/registry.ts';
import { STYLES } from '../src/styles/index.ts';
import { Resvg } from '@resvg/resvg-js';
import type { Art } from '../src/export.ts';

test('native single-frame scroll sampling uses authored stops and complete material groups', () => {
  const paint = '<rect x="10" y="10" width="30" height="12" fill="#c03020"/>';
  const full: Art = { w: 100, h: 200, svg: `<svg xmlns="http://www.w3.org/2000/svg" width="100" height="200"><rect width="100" height="200" fill="#ffffff"/>${paint}</svg>`, hits: [], bg: '#ffffff', title: 'scroll',
    motion: { kind: 'scroll', viewW: 100, viewH: 80, secs: 14, keys: [{ offset: 0, y: 0 }, { offset: .4, y: 0 }, { offset: 1, y: 120 }], material: { style: 'cutout', layers: [{ x: 10, y: 10, w: 30, h: 12, role: 'line', chapter: 0, paint }] } } };
  const style = STYLES.find(style => style.id === 'cutout')!;
  const early = frame('roll', {} as never, style, {}, .02, full)!;
  const later = frame('roll', {} as never, style, {}, .04, full)!;
  assert.match(early.svg, /viewBox="0 0 100 80"/);
  assert.notEqual(early.svg.replace(/\bc[0-9a-z]+-/g, 'scene-'), later.svg.replace(/\bc[0-9a-z]+-/g, 'scene-'));
  assert.equal(early.crop?.progress, .02);
  assert.ok(!/NaN|Infinity/.test(early.svg));
  const end = frame('roll', {} as never, style, {}, 1, full)!;
  assert.match(end.svg, /viewBox="0 120 100 80"/);
  const actual = new Resvg(end.svg).render();
  assert.equal(actual.width, 100); assert.equal(actual.height, 80);
  assert.deepEqual([...actual.pixels.slice(0, 4)], [255, 255, 255, 255]);
});
