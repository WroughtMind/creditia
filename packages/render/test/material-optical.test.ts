import { test } from 'node:test';
import assert from 'node:assert/strict';
import satori from 'satori';
import { Scene, rect } from '../src/svg.ts';
import { raster } from '../src/index.ts';
import { materialEnter } from '../src/material-motion.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';
import { themeById } from '../src/classic/themes.ts';
import { animate, motion } from '../src/classic/motion.ts';

const b = { x: 20, y: 20, w: 180, h: 50 };
const style = (id: string) => bridgeStyle(themeById(id)!);

test('film projection preserves complete text and uses no decorative line across its letters', () => {
  for (const k of [.1, .35, .65, .95]) {
    const s = new Scene(240, 100);
    materialEnter(s, style('film'), k, b, () => s.text('Claude Code 48.3%', 20, 50, { font: style('film').f.body, size: 18 }));
    const svg = s.toString();
    assert.equal([...svg.matchAll(/<text\b/g)].length, 1);
    assert.match(svg, />Claude Code 48.3%<\/text>/);
    assert.doesNotMatch(svg, /<clipPath|<mask|<line\b/);
    assert.match(svg, /<feGaussianBlur/);
  }
});

test('dark emblem marks follow their real silhouette while ordinary rectangular labels retain the baseline', () => {
  for (const role of ['mark', 'content'] as const) {
    const s = new Scene(240, 100);
    materialEnter(s, style('minimal-dark'), .4, b, () => s.text('CATNAP', 20, 50, { font: style('minimal-dark').f.body, size: 18 }), role);
    if (role === 'mark') {
      assert.doesNotMatch(s.toString(), /<line\b/);
      assert.match(s.toString(), /SourceAlpha/);
    } else assert.match(s.toString(), /<line\b/);
  }
});

test('film data exposure keeps a continuous full-size base and reaches the original pixels before mask removal', () => {
  const sample = (k: number) => {
    const s = new Scene(240, 100);
    materialEnter(s, style('film'), k, b, () => s.add(rect(20, 20, 180, 50, { fill: '#ffffff' })), 'data');
    return raster.rgba(s.toString(), 240);
  };
  for (const k of [.1, .3, .6]) {
    const frame = sample(k);
    for (let y = 21; y < 69; y++) for (let x = 21; x < 199; x++)
      assert.ok(frame.data[(y * 240 + x) * 4 + 3] > 0, `${k}: no false holes at ${x},${y}`);
  }
  assert.deepEqual(sample(.999).data, sample(1).data);
});

test('native film tiles retain the complete geometric cell while fixed grain develops on its surface', async () => {
  const tile = motion({ type: 'div', props: { style: { position: 'absolute', left: 40, top: 30, width: 80, height: 40, backgroundColor: '#ffffff' }, children: [] } }, 'cell');
  const original = structuredClone(tile);
  const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 200, height: 120 }, children: [tile] } };
  animate(tree, .12, themeById('film')!);
  const frame = raster.rgba(await satori(tree as any, { width: 200, height: 120, fonts: [] }), 200);
  for (let y = 30; y < 70; y++) for (let x = 40; x < 120; x++)
    assert.ok(frame.data[(y * 200 + x) * 4 + 3] > 0, `the real tile must remain connected at ${x},${y}`);
  let painted = 0; for (let i = 3; i < frame.data.length; i += 4) painted += frame.data[i] > 0 ? 1 : 0;
  assert.equal(painted, 80 * 40, 'material must preserve the exact full painted cell support');
  assert.equal(tile.props.style.width, 80); assert.equal(tile.props.style.height, 40);
  const final = structuredClone(original); animate(final, 1, themeById('film')!);
  assert.deepEqual(final, original);
});

test('native film cards focus without changing the original paper color', () => {
  const card = motion({ type: 'div', props: { style: { width: 180, height: 100, backgroundColor: '#fffbed' }, children: 'Claude Code 48.3%' } }, 'card');
  animate(card, .12, themeById('film')!);
  assert.equal(card.props.style.backgroundColor, '#fffbed');
  assert.match(card.props.style.filter!, /^blur\(/);
  assert.doesNotMatch(card.props.style.filter!, /brightness/);
  assert.equal(card.props.children, 'Claude Code 48.3%');
});
