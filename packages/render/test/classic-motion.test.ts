import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildModel } from '../src/classic/data.ts';
import { FORMS } from '../src/classic/registry.ts';
import { THEMES, themeById } from '../src/classic/themes.ts';
import { animate, motion, progress } from '../src/classic/motion.ts';
import { art, frames, animatedClassic, raster } from '../src/classic/index.ts';
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
const layout = (tree: any) => JSON.stringify(tree, (key, value) => ['opacity', 'transform', 'clipPath', 'src', 'filter', 'maskImage', 'textShadow', 'boxShadow', 'backgroundColor'].includes(key) ? undefined : value);

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
  const expected = { donut: 'radial', area: 'area', versions: 'bar-y', clock: 'cell', ribbon: 'cell', mosaic: 'cell', lanes: 'bar-x', settle: 'bar-x' };
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
  assert.equal(print.props.style.clipPath, undefined);
  assert.ok(print.props.style.opacity > 0 && print.props.style.opacity < 1);
  assert.equal(print.props.style.transform, undefined);
  const p = progress(0.15, { kind: 'bar-x' }, themeById('terminal')!);
  assert.equal(p * 16, Math.floor(p * 16));
});

test('export frames render the same static end across different drawing techniques and fitted layouts', async () => {
  const strip = (svg: string) => svg.replace(/s[0-9a-z]+_/g, '');
  for (const [id, tid, variant] of [
    ['badge', 'minimal-light'], ['byline', 'editorial'], ['tier', 'minimal-light'], ['tier', 'minimal-dark'], ['tier', 'glass'], ['tier', 'crt'], ['donut', 'neon'], ['area', 'terminal'],
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
    assert.ok(raster.png(result[1].svg, 160).length > 0, `${id}/${tid} must produce a valid painted SVG`);
  }
});


test('all original layouts preserve whole names and values throughout motion', () => {
  const words = (node: any): string => {
    if (typeof node === 'string') return node;
    return node && typeof node === 'object' && node.props ? [node.props.children].flat().map(words).join('') : '';
  };
  for (const t of THEMES) for (const [id, form] of Object.entries(FORMS)) for (const variant of form.variants ?? [undefined]) {
    const out = form.render(m, t, { lang: 'en', variant, projects: [m], blame: { file: 'entry.ts', line: 1 } })!;
    for (const time of [0.1, 0.5, 0.9]) {
      const sampled = structuredClone(out.tree);
      animate(sampled, time, t);
      const originals = nodes(out.tree);
      nodes(sampled).forEach((node, i) => {
        if (!['area', 'radial'].includes(node.motionCue?.kind)) assert.equal(node.props.src, originals[i].props.src, `${id}/${variant}/${t.id} changed an image source`);
      });
      for (const node of cues(sampled)) {
        const text = words(node);
        // Terminal quantity glyphs are graphics; tool names, labels and percentages are not.
        if (text && !/^[■·\s]+$/.test(text)) assert.equal(node.props.style.clipPath, undefined, `${id}/${variant}/${t.id}@${time} clips ${text}`);
      }
    }
  }
});

test('every receipt prints each complete item and every compact artifact has motion', () => {
  for (const variant of FORMS.receipt.variants!) {
    const out = FORMS.receipt.render(m, theme, { lang: 'en', variant })!;
    const rows = cues(out.tree).filter((node) => node.motionCue.kind === 'print');
    const itemCount = [m.spanDays, m.activeDays, m.sessions, m.tokens, m.commits, m.linesAdded, m.linesSurviving, m.director].filter(Boolean).length;
    assert.ok(rows.length >= itemCount, `${variant} should print every actual detail row`);
    assert.ok(cues(out.tree).some((node) => node.motionCue.kind === 'sheet'));
  }
  assert.equal(animatedClassic('badge'), true);
  const byline = FORMS.byline.render(m, theme, { lang: 'en' })!;
  assert.ok(cues(byline.tree).some((node) => node.motionCue.kind === 'signature'));
});

test('area material motion preserves every data path, date, grid and legend; terminal pixels stay square', () => {
  for (const t of THEMES) {
    const out = FORMS.area.render(m, t, { lang: 'en' })!;
    const plot = cues(out.tree).find((node) => node.motionCue.kind === 'area');
    const original = Buffer.from(plot.props.src.split(',')[1], 'base64').toString();
    animate(out.tree, 0.2, t);
    const sampled = Buffer.from(plot.props.src.split(',')[1], 'base64').toString();
    assert.equal(plot.props.style.clipPath, undefined);
    if (t.family === 'terminal') {
      assert.ok(!sampled.includes('scale('), 'terminal pixels must retain their square proportions');
      for (const rect of sampled.matchAll(/<rect x="[\d.]+" y="([\d.]+)" width="[\d.]+" height="([\d.]+)" fill="url\(#tg\d+\)"/g)) {
        assert.equal(Number(rect[1]) % 12, 0);
        assert.equal(Number(rect[2]) % 12, 0);
      }
      assert.equal(sampled.match(/<pattern[\s\S]*<\/pattern>/)?.[0], original.match(/<pattern[\s\S]*<\/pattern>/)?.[0]);
    } else {
      const paths = (svg: string) => [...svg.matchAll(/<path[^>]* d="([^"]+)"/g)].map((m) => m[1]);
      const grid = (svg: string) => [...svg.matchAll(/<line [^>]+\/>/g)].map((m) => m[0]);
      assert.deepEqual(paths(sampled), paths(original), `${t.id} changed final area geometry`);
      assert.deepEqual(grid(sampled), grid(original), `${t.id} animated the axis or grid`);
      assert.notEqual(sampled, original, `${t.id} has no area motion`);
    }
  }
});


test('terminal rings print whole grid cells and neon arcs light without cutting their glow', () => {
  for (const id of ['terminal', 'neon']) {
    const t = themeById(id)!;
    const out = FORMS.donut.render(m, t, { lang: 'en' })!;
    const plot = cues(out.tree).find((node) => node.motionCue.kind === 'radial');
    animate(out.tree, 0.2, t);
    assert.equal(plot.props.style.clipPath, undefined);
    const sampled = Buffer.from(plot.props.src.split(',')[1], 'base64').toString();
    if (id === 'terminal') for (const rect of sampled.matchAll(/<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)" fill="url\(#tg\d+\)"/g)) {
      assert.equal(Number(rect[1]) % 12, 0); assert.equal(Number(rect[2]) % 12, 0);
      assert.equal(Number(rect[3]), 12); assert.equal(Number(rect[4]), 12);
    }
    else assert.match(sampled, /filter="url\(#ng\)"/);
  }
});


test('every native badge preserves complete signatures, bounds and the exact static end', async () => {
  for (const t of THEMES) {
    const result = await frames('badge', m, t, { lang: 'en' }, 4);
    const still = (await art('badge', m, t, { lang: 'en' }))!;
    const signatures = (svg: string) => new Set([...svg.matchAll(/<text[^>]*>([^<]+)<\/text>/g)].map((match) => match[1]));
    for (const frame of result) {
      assert.deepEqual(signatures(frame.svg), signatures(still.svg), `${t.id} changed a name or value`);
      assert.equal(frame.w, still.w); assert.equal(frame.h, still.h);
      assert.doesNotMatch(frame.svg, /data-creditia-badge="text"[^>]*scale/);
    }
    assert.equal(result.at(-1)!.svg, still.svg);
  }
});

test('rasterized neon area lights the tubes along the timeline before filling the area', () => {
  const t = themeById('neon')!;
  const out = FORMS.area.render(m, t, { lang: 'en' })!;
  const plot = cues(out.tree).find((node) => node.motionCue.kind === 'area');
  const decode = () => Buffer.from(plot.props.src.split(',')[1], 'base64').toString();
  const end = raster.rgba(decode(), 520);
  animate(out.tree, 0.03, t);
  const early = raster.rgba(decode(), 520);
  const lit = (frame: typeof early, from: number, to: number) => {
    let count = 0;
    for (let y = 0; y < frame.h; y++) for (let x = from; x < to; x++) if (frame.data[(y * frame.w + x) * 4 + 3] > 128) count++;
    return count;
  };
  assert.ok(lit(early, 0, 200) > 0, 'the first tube section must actually paint');
  assert.equal(lit(early, 300, 500), 0, 'the unlit end must not already show the complete tube');
  assert.ok(lit(end, 300, 500) > 0, 'the final tube must reach the end of the timeline');
});
