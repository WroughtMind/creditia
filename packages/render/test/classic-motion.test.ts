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
  const expected = { donut: 'radial', area: 'area', versions: 'bar-y', clock: 'intensity', ribbon: 'cell', mosaic: 'cell', lanes: 'bar-x', settle: 'bar-x' };
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
  assert.ok(bars.every((node) => node.props.style.opacity === 0));
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


test('terminal areas print complete original time columns without rescaling stacked glyph heights', () => {
  const theme = themeById('terminal')!;
  const original = FORMS.area.render(m, theme, { lang: 'en' })!;
  const decode = (tree: any) => Buffer.from(cues(tree).find(n => n.motionCue.kind === 'area').props.src.split(',')[1], 'base64').toString();
  const columns = (svg: string) => {
    const result = new Map<string, string[]>();
    for (const match of svg.matchAll(/<rect x="([\d.-]+)" y="[\d.-]+" width="[\d.-]+" height="[\d.-]+" fill="url\(#tg\d+\)"\/>/g)) result.set(match[1], [...(result.get(match[1]) ?? []), match[0]]);
    return result;
  };
  const expected = columns(decode(original.tree));
  for (const time of [.05, .15, .3]) {
    const tree = structuredClone(original.tree); animate(tree, time, theme);
    const shown = columns(decode(tree));
    assert.ok(shown.size > 0 && shown.size < expected.size, `${time}: time columns must actually print in sequence`);
    for (const [x, rects] of shown) assert.deepEqual(rects, expected.get(x), `${time}/${x}: every original layer keeps its exact position and height`);
    const visible = [...shown.keys()].map(Number).sort((a, b) => a - b);
    assert.deepEqual(visible, [...expected.keys()].map(Number).sort((a, b) => a - b).slice(0, visible.length));
  }
});

test('film exposure, gala light and arcade time columns preserve every layer of an area cross-section', () => {
  const source = '<svg xmlns="http://www.w3.org/2000/svg" width="300" height="120"><g data-creditia-area="true"><path d="M0,0H300V24H0Z" fill="#ff0000"/><path d="M0,24H300V60H0Z" fill="#00ff00"/><path d="M0,60H300V120H0Z" fill="#0000ff"/></g></svg>';
  for (const id of ['film', 'gala', 'arcade']) for (const time of [.1, .25, .4]) {
    const node = motion({ type: 'img', props: { width: 300, height: 120, src: 'data:image/svg+xml;base64,' + Buffer.from(source).toString('base64') } }, 'area');
    animate(node, time, themeById(id)!);
    const svg = Buffer.from(node.props.src.split(',')[1], 'base64').toString(), frame = raster.rgba(svg, 300);
    const alpha = (x: number, y: number) => frame.data[(y * frame.w + x) * 4 + 3];
    assert.ok(alpha(id === 'gala' ? 150 : 0, 70) > 0, `${id}@${time} must visibly expose a complete time column`);
    for (const x of [0, 10, 100, 150, 200, 299]) for (const y of [0, 12, 23, 24, 40, 59, 60, 90, 119]) {
      assert.ok(Math.abs(alpha(x, y) - alpha(x, 70)) <= 1, `${id}@${time}/${x},${y}: all stacked layers at the same time need equal visibility`);
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
      assert.equal(Number(rect[3]) % 12, 0); assert.equal(Number(rect[4]), 12);
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

test('clock reveals complete cells at their true intensity and terminal bars never cut text', () => {
  for (const theme of THEMES) {
    const out = FORMS.clock.render(m, theme, { lang: 'en' })!;
    const cells = cues(out.tree).filter(n => n.motionCue.kind === 'intensity');
    const colors = cells.map(n => n.props.style.backgroundColor);
    animate(out.tree, .25, theme);
    assert.deepEqual(cells.map(n => n.props.style.backgroundColor), colors);
    assert.ok(cells.every(n => [0, 1, undefined].includes(n.props.style.opacity)), theme.id);
    assert.ok(cells.every(n => !n.props.style.filter && !n.props.style.maskImage && !n.props.style.clipPath));
    assert.ok(cells.every(n => !n.props.style.transform || /^translate\([-\d.]+px, [-\d.]+px\)$/.test(n.props.style.transform)), 'whole cells may settle by a small translation, never scale or distort their encoded area');
  }
  const terminal = themeById('terminal')!;
  const bar = motion({ type: 'div', props: { style: {}, children: '■■■■■' } }, 'bar-x');
  animate(bar, .13, terminal);
  assert.equal(bar.props.children, '■■■■■');
  assert.equal(bar.props.style.clipPath, undefined);
  assert.equal(bar.props.style.opacity, 1);
  const versions = FORMS.versions.render(m, terminal, { lang: 'en' })!;
  animate(versions.tree, .26, terminal);
  for (const node of cues(versions.tree)) if (node.motionCue.kind === 'bar-y' && node.props.style.clipPath) {
    const right = Number(/inset\(0 ([\d.]+)px 0 0\)/.exec(node.props.style.clipPath)![1]);
    assert.equal((96 - right) % 12, 0, 'terminal reveals complete glyph columns with every data layer');
  }
});

test('material cells preserve their true painted area instead of scaling percentage and treemap units', async () => {
  const { default: satori } = await import('satori');
  for (const id of ['glass', 'gala', 'minimal-dark']) for (const time of [.03, .07, .12]) {
    const tile = motion({ type: 'div', props: { style: { position: 'absolute', left: 64, top: 50, width: 80, height: 40, backgroundColor: '#ff0000' }, children: [] } }, 'cell');
    const tree = { type: 'div', props: { style: { display: 'flex', position: 'relative', width: 240, height: 160 }, children: [tile] } };
    animate(tree, time, themeById(id)!);
    const frame = raster.rgba(await satori(tree as any, { width: 240, height: 160, fonts: [] }), 240);
    let alpha = 0; for (let i = 3; i < frame.data.length; i += 4) alpha += frame.data[i];
    const paintedArea = alpha / 255 / (tile.props.style as any).opacity;
    assert.ok(Math.abs(paintedArea - 80 * 40) < 80 * 40 * .01, `${id}@${time}: material may expose or rigidly move a tile, but its full geometric area must remain 3200`);
    assert.equal(tile.props.style.width, 80); assert.equal(tile.props.style.height, 40);
  }
});

test('native stacked columns reveal their full true height and every color share in all material styles', async () => {
  const { default: satori } = await import('satori');
  for (const theme of THEMES) for (const time of [.08, .23, .4]) {
    const tree = motion({ type: 'div', props: { style: { display: 'flex', flexDirection: 'column', width: 96, height: 300 }, children: [
      { type: 'div', props: { style: { width: 96, height: 60, backgroundColor: '#ff0000' }, children: [] } },
      { type: 'div', props: { style: { width: 96, height: 90, backgroundColor: '#00ff00' }, children: [] } },
      { type: 'div', props: { style: { width: 96, height: 150, backgroundColor: '#0000ff' }, children: [] } },
    ] } }, 'bar-y');
    tree.motionCue.grid = 12; tree.motionCue.travel = 96;
    animate(tree, time, theme);
    const frame = raster.rgba(await satori(tree as any, { width: 96, height: 300, fonts: [] }), 96);
    const rowAlpha = (y: number) => {
      let alpha = 0; for (let x = 0; x < frame.w; x++) alpha += frame.data[(y * frame.w + x) * 4 + 3];
      return alpha;
    };
    const reference = rowAlpha(299);
    assert.ok(reference > 0, `${theme.id}@${time}: the true baseline must be visible`);
    for (const y of [0, 30, 59, 60, 100, 149, 150, 225, 299]) {
      assert.ok(Math.abs(rowAlpha(y) - reference) <= 1, `${theme.id}@${time}: row ${y} must receive the same reveal as every other data layer`);
    }
    assert.equal(tree.props.style.height, 300);
    assert.equal(tree.props.style.transform, undefined);
  }
});

test('native horizontal shares and time intervals retain both true endpoints throughout material formation', async () => {
  const { default: satori } = await import('satori');
  for (const theme of THEMES) for (const time of [.08, .23, .4]) {
    const tree = motion({ type: 'div', props: { style: { display: 'flex', width: 300, height: 48 }, children: [
      { type: 'div', props: { style: { width: 60, height: 48, backgroundColor: '#ff0000' }, children: [] } },
      { type: 'div', props: { style: { width: 90, height: 48, backgroundColor: '#00ff00' }, children: [] } },
      { type: 'div', props: { style: { width: 150, height: 48, backgroundColor: '#0000ff' }, children: [] } },
    ] } }, 'bar-x');
    animate(tree, time, theme);
    const frame = raster.rgba(await satori(tree as any, { width: 300, height: 48, fonts: [] }), 300);
    const columnAlpha = (x: number) => {
      let alpha = 0; for (let y = 0; y < frame.h; y++) alpha += frame.data[(y * frame.w + x) * 4 + 3];
      return alpha;
    };
    const reference = columnAlpha(299);
    assert.ok(reference > 0, `${theme.id}@${time}: the true right endpoint must already be visible`);
    for (const x of [0, 30, 59, 60, 100, 149, 150, 225, 299]) {
      assert.ok(Math.abs(columnAlpha(x) - reference) <= 1, `${theme.id}@${time}: all data positions must receive the same reveal`);
    }
    assert.equal(tree.props.style.width, 300);
    assert.equal(tree.props.style.transform, undefined);
  }
});

test('horizontal interval surfaces retain their original alpha when material formation completes', () => {
  for (const theme of THEMES) for (const time of [.2, .559999, .639999, 1]) {
    const bar = motion({ type: 'div', props: { style: { width: 120, height: 20, opacity: .9 }, children: [] } }, 'bar-x');
    animate(bar, time, theme);
    assert.ok(bar.props.style.opacity <= .9, `${theme.id}@${time} must not overwrite the original interval alpha`);
    if (time === 1) assert.equal(bar.props.style.opacity, .9);
  }
});

test('glass, monochrome and newspaper rings reveal existing arc surfaces, while role cards print complete people independently', () => {
  for (const id of ['glass', 'mono', 'broadsheet']) {
    const theme = themeById(id)!;
    const out = FORMS.donut.render(m, theme, { lang: 'en' })!;
    const ring = cues(out.tree).find(n => n.motionCue.kind === 'radial');
    const decode = () => Buffer.from(ring.props.src.split(',')[1], 'base64').toString();
    const before = decode();
    const blankTree = structuredClone(out.tree); animate(blankTree, 0, theme);
    const blankRing = cues(blankTree).find(n => n.motionCue.kind === 'radial');
    const blank = Buffer.from(blankRing.props.src.split(',')[1], 'base64').toString();
    animate(out.tree, .22, theme); const after = decode();
    const arcs = (s: string) => [...s.matchAll(/<circle[^>]*stroke-dasharray="([^"]+)"[^>]*stroke-dashoffset="([^"]+)"/g)].map(x => [x[1], x[2]]);
    assert.deepEqual(arcs(after), arcs(before), id + ' preserves every final angular share');
    assert.match(after, /mask="url\(#cm-ring\)"/);
    assert.notDeepEqual(raster.png(after, 210), raster.png(blank, 210), id + ' must visibly print before its final frame');
  }
  const cards = FORMS.roll.render(m, theme, { lang: 'en', variant: 'cards' })!;
  assert.ok(cues(cards.tree).filter(n => n.motionCue.kind === 'card').every(n => n.motionCue.columns === 2));
  const paired = (order: number) => progress(.2, { kind: 'card', order, total: 4, columns: 2 }, themeById('playbill')!);
  assert.equal(paired(0), paired(1)); assert.equal(paired(2), paired(3)); assert.ok(paired(0) > paired(2));
  assert.ok(cues(cards.tree).some(n => n.motionCue.kind === 'print'));
});

test('native ring material never shortens an angular share and terminal prints complete ownership groups', () => {
  const geometry = (svg: string) => [...svg.matchAll(/<circle\b[^>]*stroke-dasharray="[^"]+"[^>]*\/>/g)].map(([circle]) =>
    ['cx', 'cy', 'r', 'stroke-width', 'stroke-dasharray', 'stroke-dashoffset', 'transform'].map(attr => new RegExp(`\\b${attr}="([^"]+)"`).exec(circle)?.[1]));
  const glyphGroups = (svg: string) => {
    const groups = new Map<string, string[]>();
    for (const match of svg.matchAll(/<rect x="[\d.-]+" y="[\d.-]+" width="[\d.-]+" height="[\d.-]+" fill="url\(#(tg\d+)\)"\/>/g)) groups.set(match[1], [...(groups.get(match[1]) ?? []), match[0]]);
    return groups;
  };
  for (const theme of THEMES) {
    const original = FORMS.donut.render(m, theme, { lang: 'en' })!;
    const source = Buffer.from(cues(original.tree).find(n => n.motionCue.kind === 'radial').props.src.split(',')[1], 'base64').toString();
    for (const time of [.03, .12, .3, .5]) {
      const tree = structuredClone(original.tree); animate(tree, time, theme);
      const sampled = Buffer.from(cues(tree).find(n => n.motionCue.kind === 'radial').props.src.split(',')[1], 'base64').toString();
      if (theme.family === 'terminal') {
        const expected = glyphGroups(source), visible = glyphGroups(sampled);
        assert.ok(visible.size, `${theme.id}@${time} must print an ownership group`);
        for (const [id, rects] of visible) assert.deepEqual(rects, expected.get(id), `${id} must print its complete original arc, never an angle slice`);
        if (time === .03) assert.ok(visible.size < expected.size, 'terminal groups retain a visible print sequence');
      } else assert.deepEqual(geometry(sampled), geometry(source), `${theme.id}@${time} changed a data angle or radius`);
    }
  }
});

test('continuous native ring surfaces fill their antialiased edges before temporary masks are removed', () => {
  for (const theme of THEMES.filter(t => !['terminal', 'crt', 'arcade', 'editorial', 'playbill', 'poster'].includes(t.family))) {
    const out = FORMS.donut.render(m, theme, { lang: 'en' })!;
    const plot = cues(out.tree).find(n => n.motionCue.kind === 'radial');
    const decode = () => Buffer.from(plot.props.src.split(',')[1], 'base64').toString();
    const end = raster.rgba(decode(), 420);
    const span = ['film', 'cinema', 'gala'].includes(theme.family) ? .64 : .56;
    animate(out.tree, span - .00001, theme);
    const almost = raster.rgba(decode(), 420);
    let changed = 0;
    for (let i = 0; i < end.data.length; i += 4) if ([0, 1, 2, 3].some(j => Math.abs(end.data[i + j] - almost.data[i + j]) > 8)) changed++;
    assert.ok(changed < 420 * 420 * .001, `${theme.id}: antialiased ring edges must not suddenly fill when the mask disappears`);
  }
});

test('native print area surfaces remain visible when reduced and optical masks reach their actual endpoint continuously', async () => {
  const sample = (id: string, time: number) => {
    const theme = themeById(id)!;
    const out = FORMS.area.render(m, theme, { lang: 'en' })!;
    animate(out.tree, time, theme);
    const plot = cues(out.tree).find(n => n.motionCue.kind === 'area');
    return Buffer.from(plot.props.src.split(',')[1], 'base64').toString().replace(/<line\b[^>]*\/>/g, '');
  };
  const alpha = (frame: ReturnType<typeof raster.rgba>) => {
    let sum = 0; for (let i = 3; i < frame.data.length; i += 4) sum += frame.data[i]; return sum;
  };
  for (const id of ['broadsheet', 'mono']) for (const width of [130, 260]) {
    const middle = raster.rgba(sample(id, .2), width), end = raster.rgba(sample(id, 1), width);
    assert.ok(alpha(middle) > alpha(end) * .12, `${id}/${width}: actual ink must remain visible at reduced resolution`);
    assert.ok(alpha(middle) < alpha(end) * .9, `${id}/${width}: print formation must still be intermediate`);
    const almost = raster.rgba(sample(id, .55999), width);
    assert.ok(alpha(almost) > alpha(end) * .995, `${id}/${width}: no subpixel holes may survive until the mask is removed`);
  }
  const almost = raster.rgba(sample('glass', .55), 520), end = raster.rgba(sample('glass', .56), 520);
  let changed = 0;
  for (let i = 3; i < end.data.length; i += 4) if (Math.abs(end.data[i] - almost.data[i]) > 5) changed++;
  assert.ok(changed < end.w * end.h * .002, 'the last optical sliver must not leave a large faded corner that snaps solid');

  const { default: satori } = await import('satori');
  const bar = async (time: number) => {
    const tree = motion({ type: 'div', props: { style: { display: 'flex', width: 400, height: 80, backgroundColor: '#000' }, children: [] } }, 'bar-x');
    animate(tree, time, themeById('glass')!);
    return raster.rgba(await satori(tree as any, { width: 400, height: 80, fonts: [] }), 200);
  };
  assert.ok(alpha(await bar(.55)) > alpha(await bar(.56)) * .995, 'a glass quantity bar must complete its soft edge before the mask disappears');
});

test('editorial native area prints complete weekly cross-sections without floating caps', () => {
  const theme = themeById('editorial')!;
  const sample = (time: number) => {
    const out = FORMS.area.render(m, theme, { lang: 'en' })!;
    animate(out.tree, time, theme);
    const plot = cues(out.tree).find(n => n.motionCue.kind === 'area');
    const svg = Buffer.from(plot.props.src.split(',')[1], 'base64').toString().replace(/<line\b[^>]*\/>/g, '');
    return raster.rgba(svg, 520);
  };
  const end = sample(1);
  for (const time of [.03, .07, .11, .15, .2]) {
    const frame = sample(time); let seen = 0;
    for (let x = 0; x < frame.w; x++) {
      const base = ((frame.h - 1) * frame.w + x) * 4 + 3;
      if (end.data[base] <= 128) continue;
      let visible = false;
      for (let y = 0; y < frame.h; y++) if (frame.data[(y * frame.w + x) * 4 + 3] > 128) visible = true;
      if (visible) { seen++; assert.ok(frame.data[base] > 128, `time ${time}, column ${x}: a revealed weekly cross-section must reach its real baseline`); }
    }
    assert.ok(seen > 0, `time ${time}: actual weekly ink must be present`);
  }
});

test('clock uses real day, period and activity-run units while every visible cell retains its true intensity', () => {
  const cellsAt = (id: string, at: number) => {
    const theme = themeById(id)!, out = FORMS.clock.render(m, theme, { lang: 'en' })!;
    const cells = cues(out.tree).filter(n => n.motionCue.kind === 'intensity');
    const original = cells.map(n => n.props.style.backgroundColor);
    animate(out.tree, at, theme);
    cells.forEach((n, i) => {
      assert.equal(n.props.style.backgroundColor, original[i]);
      assert.ok([0, 1, undefined].includes(n.props.style.opacity));
      assert.ok(!n.props.style.filter && !n.props.style.maskImage && !n.props.style.clipPath);
      if (n.props.style.transform) for (const v of n.props.style.transform.matchAll(/(-?[\d.]+)px/g)) assert.ok(Math.abs(Number(v[1])) <= 6, 'a cell must stay within its own grid gap while settling');
    });
    return cells;
  };
  for (const theme of THEMES) for (const at of [.04, .16, .35, .6, .85]) cellsAt(theme.id, at);
  const assertSame = (cells: any[], key: (n: any) => number) => {
    const groups = new Map<number, any[]>();
    for (const n of cells) { const k = key(n); if (!groups.has(k)) groups.set(k, []); groups.get(k)!.push(n.props.style.opacity ?? 1); }
    for (const values of groups.values()) assert.equal(new Set(values).size, 1, 'a semantic block must enter as a complete unit');
  };
  assertSame(cellsAt('film', .16), n => n.motionCue.row);
  assertSame(cellsAt('terminal', .16), n => n.motionCue.row);
  assertSame(cellsAt('glass', .16), n => Math.floor(n.motionCue.column / 6));
  assertSame(cellsAt('poster', .16), () => 0);
  assertSame(cellsAt('neon', .35).filter(n => n.motionCue.level > 0), n => n.motionCue.run);
  const arcade = cellsAt('arcade', .16);
  assertSame(arcade, n => n.motionCue.row * 4 + Math.floor(n.motionCue.column / 6));
});
