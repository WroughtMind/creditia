import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildModel } from '../src/classic/data.ts';
import { FORMS } from '../src/classic/registry.ts';
import { THEMES, themeById } from '../src/classic/themes.ts';
import { animate, motion, progress } from '../src/classic/motion.ts';
import { art, frames } from '../src/classic/index.ts';
import { motionDuration } from '../src/motion.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const m = buildModel(raw, { director: false, lang: 'en' });
const theme = themeById('minimal-light')!;
const nodes = (tree: any): any[] => {
  if (!tree || typeof tree !== 'object' || !tree.props) return [];
  return [tree, ...[tree.props.children].flat().flatMap(nodes)];
};
const cues = (tree: any) => nodes(tree).filter((node) => node.motionCue);
// A motion frame may change drawing, but never the laid-out content or geometry.
const layout = (tree: any) => JSON.stringify(tree, (key, value) => ['opacity', 'transform', 'clipPath'].includes(key) ? undefined : value);

test('every classic form and layout declares content groups; sampling leaves layout and the static end untouched', () => {
  for (const t of THEMES) for (const [id, form] of Object.entries(FORMS)) for (const variant of form.variants ?? [undefined]) {
    const out = form.render(m, t, { lang: 'en', variant, projects: [m], blame: { file: 'entry.ts', line: 1 } });
    assert.ok(out, `${id}/${t.id}/${variant}`);
    const label = `${id}/${t.id}/${variant ?? 'default'}`;
    assert.ok(cues(out.tree).length, `${label} has no content motion`);
    const still = structuredClone(out.tree);
    animate(still, 1, t);
    assert.deepEqual(still, out.tree, `${label} altered the static end`);
    const middle = structuredClone(out.tree);
    animate(middle, 0.25, t);
    assert.equal(layout(middle), layout(out.tree), `${label} changed layout or text`);
  }
});

test('chart motion follows the represented quantity and leaves headings and decoration static', () => {
  const expected = { donut: 'radial', area: 'scan-x', versions: 'bar-y', clock: 'cell', ribbon: 'cell', mosaic: 'cell', lanes: 'bar-x', settle: 'bar-x' };
  for (const [id, kind] of Object.entries(expected)) {
    const out = FORMS[id].render(m, theme, { lang: 'en' })!;
    assert.ok(cues(out.tree).some((node) => node.motionCue.kind === kind), id);
    const unmarked = nodes(out.tree).filter((node) => !node.motionCue);
    const before = unmarked.map((node) => JSON.stringify(node.props.style));
    animate(out.tree, 0.2, theme);
    assert.deepEqual(unmarked.map((node) => JSON.stringify(node.props.style)), before, id);
  }
  const settle = FORMS.settle.render(m, theme, { lang: 'en' })!;
  const bars = cues(settle.tree);
  assert.equal(bars.length, 2);
  assert.ok(bars.some((node) => node.props.style.width === '100%'));
  animate(settle.tree, 0, theme);
  assert.ok(bars.every((node) => node.props.style.clipPath === 'inset(0 100% 0 0)'));
});

test('terminal output steps by whole rows, cinema settles, and print styles reveal without displacement', () => {
  const source = { type: 'div', props: { style: { width: 100, height: 20 }, children: 'Tool 42%' } };
  const terminal = motion(structuredClone(source), 'row');
  animate(terminal, 0.15, themeById('terminal')!);
  assert.equal(terminal.props.style.transform, undefined);
  assert.equal(terminal.props.children, 'Tool 42%');
  const cinema = motion(structuredClone(source), 'row');
  animate(cinema, 0.15, themeById('cinema')!);
  assert.match(cinema.props.style.transform, /translate/);
  assert.ok(cinema.props.style.opacity > 0 && cinema.props.style.opacity < 1);
  const print = motion(structuredClone(source), 'row');
  animate(print, 0.15, themeById('broadsheet')!);
  assert.match(print.props.style.clipPath, /inset/);
  assert.equal(print.props.style.transform, undefined);
  const p = progress(0.15, { kind: 'bar-x' }, themeById('terminal')!);
  assert.equal(p * 16, Math.floor(p * 16));
});

test('export frames render the same static end across different drawing techniques and fitted layouts', async () => {
  const strip = (svg: string) => svg.replace(/s[0-9a-z]+_/g, '');
  for (const [id, tid, variant] of [
    ['tier', 'minimal-light'], ['donut', 'neon'], ['area', 'terminal'],
    ['versions', 'minimal-light'], ['receipt', 'minimal-light', 'thermal'],
    ['roll', 'cinema', 'crawl'], ['roll', 'cinema', 'slate'], ['app', 'mono', 'ios'],
  ]) {
    const t = themeById(tid)!;
    const ctx = { lang: 'en' as const, variant };
    const result = await frames(id, m, t, ctx, 4);
    const final = (await art(id, m, t, ctx))!;
    assert.equal(result.length, 5);
    assert.ok(result.every((frame) => frame.frameDelay === motionDuration(id, variant ?? FORMS[id]?.def?.(t), tid) / 4));
    assert.equal(strip(result.at(-1)!.svg), strip(final.svg), `${id}/${tid}/${variant}`);
    assert.ok(result.every((frame) => frame.w === final.w && frame.h === final.h));
    assert.notEqual(strip(result[0].svg), strip(result.at(-1)!.svg), id);
  }
});
