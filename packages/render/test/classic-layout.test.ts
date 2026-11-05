import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { classicArt, classicFrames } from '../src/classic/art.ts';
import { buildModel } from '../src/classic/data.ts';
import { themeById } from '../src/classic/themes.ts';

test('interactive classic frames reuse the fitted still layout without repeating measurement passes', async () => {
  const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
  const model = buildModel(raw, { director: false, lang: 'en', group: 'auto', named: true });
  const theme = themeById('film')!, ctx = { lang: 'en' as const, variant: 'original' };
  const trees: string[] = [];
  const satori = async (tree: unknown) => { trees.push(JSON.stringify(tree)); return '<svg/>'; };
  await classicArt('tier', model, theme, ctx, satori, []);
  const fitted = trees.at(-1), measurements = trees.length;
  await classicFrames('tier', model, theme, ctx, satori, [], 24, [.25]);
  assert.equal(trees.length, measurements + 1, 'one new SVG draw, with no layout pass');
  assert.notEqual(trees.at(-1), fitted, 'the sampled frame has its own motion state');
  await classicFrames('tier', model, theme, ctx, satori, [], 24, [1]);
  assert.equal(trees.length, measurements + 2);
  assert.equal(trees.at(-1), fitted, 'sampling never mutates the cached resting tree');
  await classicFrames('tier', model, theme, { ...ctx, transparent: true }, satori, [], 24, [1]);
  assert.ok(trees.length > measurements + 3, 'changed canvas preferences get a fresh layout');
});
