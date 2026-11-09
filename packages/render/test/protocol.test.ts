import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, symlinkSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Readable, Writable } from 'node:stream';
import { dispatchRender, renderCatalog, createRenderSession, MAX_REQUEST_BYTES } from '../src/protocol.ts';
import { nativeBackend, renderRequest } from '../src/native-protocol.ts';
import { buildModel, art, frame, frames, STYLES, raster } from '../src/index.ts';
import { makeExporter, sized, type Art } from '../src/export.ts';
import * as C from '../src/classic/index.ts';
import { serve } from '../tools/bridge.mjs';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const request = { version: 1, id: 1, op: 'render', style: 'minimal-light', piece: 'asterisk', raw, lang: 'en', group: 'auto', named: true };
// Only per-render namespace prefixes are allowed to differ; keep definition
// names, geometry, text, colors and every hit-region field exact.
const normalized = (s: string) => s
  .replace(/(\bid="|url\(#|\bhref="#)c[0-9a-z]+(?=-)/g, '$1scene')
  .replace(/(\bid="|url\(#|\bhref="#)s[0-9a-z]+(?=_)/g, '$1satori');
const normalizedArt = (a: Art | null) => a && ({ ...a, svg: normalized(a.svg), ...(a.crop ? { crop: { ...a.crop, svg: normalized(a.crop.svg) } } : {}) });
const unwrap = (response: Awaited<ReturnType<typeof renderRequest>>): any => {
  assert.equal(response.ok, true, response.ok ? '' : JSON.stringify(response.error));
  return response.ok ? response.result : null;
};

test('the installed bridge serves requests through a directory alias', () => {
  const directory = mkdtempSync(join(tmpdir(), 'creditia-bridge-'));
  try {
    const alias = join(directory, 'installed renderer');
    symlinkSync(fileURLToPath(new URL('..', import.meta.url)), alias, 'junction');
    const output = execFileSync(process.execPath, [join(alias, 'tools/bridge.mjs')], {
      cwd: directory, encoding: 'utf8', timeout: 30_000,
      input: JSON.stringify({ version: 1, id: 'alias', op: 'catalog' }) + '\n',
    });
    const response = JSON.parse(output);
    assert.equal(response.id, 'alias');
    assert.equal(response.result.styles.length, 26);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test('native catalog preserves all 26 desktop styles and registry variants', async () => {
  const result = unwrap(await renderRequest({ version: 1, id: 'catalog', op: 'catalog' }));
  assert.equal(result.styles.length, 26);
  assert.equal(new Set(result.styles.map(s => s.id)).size, 26);
  assert.equal(result.styles.filter(s => s.renderer === 'classic').length, 15);
  assert.deepEqual(result.formats.map(f => f.id), ['png', 'svg', 'pdf', 'gif', 'html', 'md', 'embed', 'txt', 'json']);
  assert.equal(result.sharedStyles.length, 16);
  for (const style of result.styles) assert.ok(style.board.length >= 20);
  for (const style of result.styles) {
    assert.ok(style.pieces.some(p => p.id === 'asterisk'));
    for (const p of style.pieces) if (p.defaultVariant !== null) assert.ok(p.variants.some(v => v.id === p.defaultVariant), `${style.id}/${p.id}`);
    const rendered = unwrap(await renderRequest({ ...request, style: style.id }));
    assert.ok(rendered.art?.svg.startsWith('<svg'), style.id);
    assert.ok(rendered.art.hits.length > 0, style.id);
  }
});

test('SVG protocol preserves archive preferences, geometry and hit regions', async () => {
  const style = STYLES.find(s => s.id === 'ink')!;
  const model = buildModel(raw, { lang: 'en', group: 'model', named: false, sign: '' });
  const expected = art('tier', model, style, { variant: 'grouped' });
  const result = unwrap(await renderRequest({ ...request, style: 'ink', piece: 'tier', variant: 'grouped', group: 'model', named: false }));
  assert.deepEqual(normalizedArt(result.art), normalizedArt(expected));
  assert.ok(!JSON.stringify(result.art).includes('Mara Okafor'));
});

test('native classic output and sampled animation use existing layout and motion', async () => {
  const theme = C.themeById('minimal-light')!;
  const model = C.buildModel(raw, { director: false, lang: 'en', group: 'auto', named: true });
  const expected = await C.art('tier', model, theme, { lang: 'en', variant: 'original' });
  const actual = unwrap(await renderRequest({ ...request, piece: 'tier', variant: 'original' })).art;
  assert.equal(normalized(actual.svg), normalized(expected!.svg));
  assert.deepEqual(actual.hits, expected!.hits);
  for (const [piece, variant] of [['tier', 'original'], ['roll', 'crawl'], ['roll', 'scroll'], ['roll', 'troupe']]) {
    const animation = await C.frames(piece, model, theme, { lang: 'en', variant }, 2);
    const sampled = unwrap(await renderRequest({ ...request, piece, variant, t: 0.5 })).art;
    assert.equal(normalized(sampled.svg), normalized(animation[1].svg), `${piece}/${variant}`);
  }
});

test('flatten preserves already outlined classic artwork and outlines SVG text', async () => {
  let flattened = 0;
  const backend = { ...nativeBackend, raster: { ...nativeBackend.raster, flat(svg: string) { flattened++; return nativeBackend.raster.flat(svg); } } };
  const classic = unwrap(await dispatchRender({ ...request, piece: 'tier', variant: 'original', flatten: true }, backend)).art;
  assert.equal(flattened, 0);
  assert.ok(!classic.svg.includes('<text'));
  const svg = unwrap(await dispatchRender({ ...request, flatten: true }, backend)).art;
  assert.equal(flattened, 1);
  assert.ok(!svg.svg.includes('<text'));
});

test('SVG time sampling matches exported frames, including scrolling crops', () => {
  const model = buildModel(raw, { lang: 'en' });
  const style = STYLES.find(s => s.id === 'ink')!;
  for (const [piece, variant] of [['asterisk', undefined], ['roll', 'scroll']] as const) {
    const exported = frames(piece, model, style, { variant }, 4);
    for (let i = 0; i < exported.length; i++) {
      const { frameDelay, ...image } = exported[i];
      const time = image.crop?.progress ?? i / (exported.length - 1);
      assert.deepEqual(normalizedArt(frame(piece, model, style, { variant }, time)), normalizedArt(image));
      assert.ok(i === exported.length - 1 ? frameDelay! >= 0 : frameDelay! > 0, 'movement samples retain their authored hold; the encoder owns final rest');
    }
  }
});

test('every catalog style/piece/layout keeps the shared renderer data and geometry', async t => {
  let count = 0;
  for (const style of renderCatalog().styles) {
    const theme = C.themeById(style.id);
    for (const piece of style.pieces) {
      if (piece.requires.length) continue; // profile/blame require separate CLI result data
      for (const variant of [undefined, ...piece.variants.map(v => v.id)]) {
        const group = count % 2 ? 'model' : 'tool', named = count % 3 === 0;
        const options = { lang: 'en' as const, group: group as 'tool' | 'model', named };
        const row = `${count}: ${style.id}/${piece.id}/${variant ?? 'default'} group=${group} named=${named}`;
        try {
          const actual = unwrap(await renderRequest({ ...request, ...options, style: style.id, piece: piece.id, variant })).art;
          const expected = theme
            ? await C.art(piece.id, C.buildModel(raw, { director: false, ...options }), theme, { lang: 'en', variant })
            : art(piece.id, buildModel(raw, { ...options, sign: '' }), STYLES.find(s => s.id === style.id)!, { variant });
          assert.deepEqual(normalizedArt(actual), normalizedArt(expected), row);
        } catch (error) {
          throw new Error(`Matrix row ${row}; memory=${JSON.stringify(process.memoryUsage())}`, { cause: error });
        }
        count++;
      }
    }
    t.diagnostic(`${style.id}: ${count} cumulative rows; memory=${JSON.stringify(process.memoryUsage())}`);
  }
  assert.ok(count > 1482, `Expected complete desktop matrix plus default layouts; compared ${count}`);
});

test('sized scrolling GIF keeps both its viewport and chosen canvas', () => {
  // Two colored sections make loss of the scroll origin visible at a known pixel.
  const reel = '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="200" viewBox="0 0 100 200"><rect width="100" height="100" fill="#ff0000"/><rect y="100" width="100" height="100" fill="#0000ff"/></svg>';
  const at = (y: number): Art => ({
    w: 100, h: 100, bg: '#ffffff', hits: [], title: 'Scroll',
    svg: reel.replace('height="200" viewBox="0 0 100 200"', `height="100" viewBox="0 ${y} 100 100"`),
    crop: { svg: reel, w: 100, h: 200, y },
  });
  const samples = [sized(at(0), 200, 200), sized(at(100), 200, 200)];
  for (const sample of samples) assert.deepEqual(sample.crop?.viewport, { x: 12, y: 12, w: 176, h: 176 });
  const pixels = samples.map(x => raster.rgba(x.svg));
  assert.ok(pixels.every(x => x.w === 200 && x.h === 200));
  const center = (img: typeof pixels[number]) => [...img.data.subarray((100 * img.w + 100) * 4, (100 * img.w + 100) * 4 + 4)];
  assert.deepEqual(center(pixels[0]), [255, 0, 0, 255]);
  assert.deepEqual(center(pixels[1]), [0, 0, 255, 255]);
  const seen: string[] = [];
  const exporter = makeExporter({ ...raster, rgba(svg, width) { seen.push(svg); return raster.rgba(svg, width); } });
  const gif = exporter.gif(samples, { width: 200 });
  assert.deepEqual(seen, [reel], 'sizing reuses one reel raster instead of rendering each viewport');
  // GIF logical screen width/height (little endian, after GIF89a).
  assert.equal(gif[6] + (gif[7] << 8), 200);
  assert.equal(gif[8] + (gif[9] << 8), 200);
});

test('HTML honors language and canvas size for static and scrolling artwork', async () => {
  const result = unwrap(await renderRequest({ ...request, op: 'export', format: 'html', size: 'square', frameCount: 2 }));
  const html = Buffer.from(result.bytes, 'base64').toString();
  assert.ok(html.includes('<html lang="en">'));
  assert.ok(html.includes('"w":1080,"h":1080'));
  const scrolling = await renderRequest({ ...request, op: 'export', style: 'ink', piece: 'roll', variant: 'scroll', format: 'html', size: 'square', frameCount: 2 });
  const animated = Buffer.from(unwrap(scrolling).bytes, 'base64').toString();
  assert.ok(animated.includes('htmlPlayer'));
  assert.ok(animated.includes('\"w\":1080,\"h\":1080'));
  assert.ok(!animated.includes('class=\"reel\"'));
});

test('native export returns the five real formats with embedded outlined SVG fonts', async () => {
  for (const [format, magic] of [['png', '\x89PNG'], ['svg', '<svg'], ['pdf', '%PDF'], ['gif', 'GIF8'], ['html', '<!doctype html>']]) {
    const result = unwrap(await renderRequest({ ...request, op: 'export', format, frameCount: 2 }));
    assert.equal(result.encoding, 'base64');
    const bytes = Buffer.from(result.bytes, 'base64');
    assert.equal(bytes.subarray(0, magic.length).toString('latin1'), magic, format);
    if (format === 'svg') assert.ok(!bytes.toString().includes('<text'));
    assert.equal(result.ext, format);
  }
});

test('invalid requests fail explicitly and do not invoke the renderer', async () => {
  const blocked = new Proxy({}, { get() { throw new Error('Backend should not be touched'); } }) as any;
  for (const [patch, code] of [
    [{ version: 2 }, 'unsupported_version'], [{ style: '__proto__' }, 'unknown_style'],
    [{ piece: 'constructor' }, 'unknown_piece'], [{ variant: 'no-such-layout' }, 'unknown_variant'],
    [{ raw: null }, 'invalid_request'], [{ lang: 'fr' }, 'invalid_request'],
    [{ t: 1.1 }, 'invalid_request'], [{ named: 'false' }, 'invalid_request'],
    [{ output: '/tmp/file' }, 'invalid_request'], [{ piece: 'profile' }, 'missing_data'],
    [{ op: 'export', format: 'exe' }, 'invalid_request'],
  ] as const) {
    const result = await dispatchRender({ ...request, ...patch }, blocked);
    assert.equal(result.ok, false);
    if (!result.ok) assert.equal(result.error.code, code, JSON.stringify(patch));
  }
});

test('native compact badges now expose real animation frames and GIF output', async () => {
  const animation = unwrap(await renderRequest({ ...request, op: 'frames', piece: 'badge', frameCount: 2 }));
  assert.equal(animation.frames.length, 3);
  assert.notEqual(normalized(animation.frames[0].svg), normalized(animation.frames[1].svg));
  const output = unwrap(await renderRequest({ ...request, op: 'export', piece: 'badge', format: 'gif', frameCount: 2, width: 160 }));
  assert.equal(Buffer.from(output.bytes, 'base64').subarray(0, 6).toString(), 'GIF89a');
});

test('JSONL transport recovers after malformed and oversized input, then exits on EOF', async () => {
  const good = JSON.stringify({ version: 1, id: 'last', op: 'catalog' });
  const chunks = [Buffer.from('{invalid}\n'), Buffer.alloc(MAX_REQUEST_BYTES + 1, 120), Buffer.from('\n' + good)];
  let output = '';
  const sink = new Writable({ write(chunk, _encoding, cb) { output += chunk.toString(); cb(); } });
  await serve(Readable.from(chunks), sink, nativeBackend);
  const responses = output.trim().split('\n').map(s => JSON.parse(s));
  assert.equal(responses.length, 3);
  assert.equal(responses[0].error.code, 'invalid_json');
  assert.equal(responses[1].error.code, 'request_too_large');
  assert.equal(responses[2].id, 'last');
  assert.equal(responses[2].ok, true);
  assert.deepEqual(responses[2].result, renderCatalog());
  assert.ok(responses.every(response => Number.isSafeInteger(response.runtime.rssBytes) && response.runtime.rssBytes > 0));
});

test('persistent session retains prepared models and artwork, replaces archives and evicts bounded state', async () => {
  let renders = 0;
  const backend = { ...nativeBackend, classicArt: async (...args: Parameters<typeof nativeBackend.classicArt>) => { renders++; return nativeBackend.classicArt(...args); } };
  const session = createRenderSession(backend);
  const call = (patch: object) => session.dispatch({ version: 1, id: 'session', ...patch });
  unwrap(await call({ op: 'archive', archiveId: 'one', raw }));
  const options = { op: 'render', archiveId: 'one', style: 'minimal-light', piece: 'tier', variant: 'original' };
  const first = unwrap(await call(options)).art;
  first.svg = 'caller mutation';
  const second = unwrap(await call(options)).art;
  assert.notEqual(second.svg, 'caller mutation');
  assert.equal(renders, 1);
  assert.equal(session.stats().models, 2);
  assert.equal(session.stats().artworks, 1);
  unwrap(await call({ op: 'archive', archiveId: 'one', raw: { ...raw, project: { ...raw.project, name: 'Replacement' } } }));
  assert.ok(unwrap(await call(options)).art.title.includes('Replacement'));
  assert.equal(renders, 2);
  for (let i = 0; i < 9; i++) unwrap(await call({ op: 'archive', archiveId: `project-${i}`, raw }));
  const evicted = await call(options);
  assert.equal(evicted.ok, false);
  if (!evicted.ok) assert.equal(evicted.error.code, 'unknown_archive');
  assert.equal(session.stats().archives, 8);
  assert.ok(session.stats().archiveBytes <= 24 * 1024 * 1024);
  unwrap(await call({ op: 'release', archiveId: 'project-8' }));
  assert.equal(session.stats().archives, 7);
});

test('animation samples have one shared duration, exact final geometry and reduced-motion final state', async () => {
  const options = { ...request, op: 'frames', style: 'ink', piece: 'asterisk', frameCount: 4 };
  const result = unwrap(await renderRequest(options));
  assert.deepEqual(result.times, [0, .25, .5, .75, 1]);
  assert.equal(result.frames.length, 5);
  const reference = frames('asterisk', buildModel(raw, { lang: 'en' }), STYLES.find(style => style.id === 'ink')!, {}, 2);
  assert.equal(result.timeline.durationMs, reference[0].frameDelay! * 2, 'native and standalone export use the same authored duration');
  const { frameCount: _count, ...renderOptions } = options;
  const still = unwrap(await renderRequest({ ...renderOptions, op: 'render' }));
  assert.deepEqual(normalizedArt(result.frames.at(-1)), normalizedArt(still.art));
  const reduced = unwrap(await renderRequest({ ...options, reducedMotion: true }));
  assert.equal(reduced.frames.length, 1);
  assert.deepEqual(reduced.times, [1]);
  assert.equal(reduced.timeline.loop, false);
  assert.equal(reduced.timeline.durationMs, 0);
  assert.deepEqual(normalizedArt(reduced.frames[0]), normalizedArt(still.art));
});

test('hidden shared SVG styles remain addressable independently from desktop classic styles', async () => {
  for (const style of ['minimal', 'noir', 'glass', 'cinema', 'terminal']) {
    const result = unwrap(await renderRequest({ ...request, renderer: 'svg', style })).art;
    const expected = art('asterisk', buildModel(raw, { lang: 'en', named: true, group: 'auto', sign: '' }), STYLES.find(s => s.id === style)!);
    assert.deepEqual(normalizedArt(result), normalizedArt(expected), style);
  }
});

test('text, archive, embed and cross-project output use the shared data model', async () => {
  for (const format of ['md', 'txt', 'json', 'embed']) {
    const result = unwrap(await renderRequest({ ...request, op: 'export', format }));
    const text = Buffer.from(result.bytes, 'base64').toString();
    assert.ok(text.length > 20);
    if (format === 'json') assert.deepEqual(JSON.parse(text), raw);
    if (format === 'embed') {
      assert.ok(text.includes('<creditia-card'));
      assert.ok(text.includes('renderer="classic"'));
      assert.ok(text.includes('lang="en"'));
      assert.ok(text.includes('group="auto"'));
      assert.ok(text.includes('named="true"'));
    }
  }
  const merge = unwrap(await renderRequest({ version: 1, id: 'merge', op: 'merge', projects: [raw, raw], lang: 'en' }));
  assert.equal(merge.raw.totals.linesAdded, raw.totals.linesAdded * 2);
  const malicious = await renderRequest({ ...request, op: 'export', format: 'embed', piece: '"><script>' });
  assert.equal(malicious.ok, false);
});

test('transparent PNG removes the canvas and SVG font embedding includes actual bytes and selectable text', async () => {
  for (const style of ['minimal-light', 'ink']) {
    const result = unwrap(await renderRequest({ ...request, style, transparent: true })).art;
    assert.equal(result.bg, 'transparent');
    const image = raster.rgba(result.svg, 120);
    assert.equal(image.data[3], 0, style);
  }
  const result = unwrap(await renderRequest({ ...request, piece: 'tier', variant: 'original', op: 'export', format: 'svg', fontMode: 'embed' }));
  const svg = Buffer.from(result.bytes, 'base64').toString();
  assert.ok(svg.includes('<text'));
  assert.ok(svg.includes('@font-face'));
  assert.ok(svg.includes('data:font/ttf;base64,'));
  assert.ok(!svg.includes('file://'));
  const badge = unwrap(await renderRequest({ ...request, piece: 'badge', variant: 'original', op: 'export', format: 'svg', fontMode: 'embed' }));
  const badgeSvg = Buffer.from(badge.bytes, 'base64').toString();
  const root = /^<svg\b[^>]*>/.exec(badgeSvg)![0];
  assert.equal((root.match(/\brole=/g) ?? []).length, 1);
  assert.equal((root.match(/\baria-label=/g) ?? []).length, 1);
  assert.ok(badgeSvg.includes('font-family="&apos;Noto Sans SC&apos;"'));
});

test('standalone HTML includes hover placement and cannot terminate its own script with an archive title', async () => {
  const injected = { ...raw, project: { ...raw.project, name: '</script><script>alert(1)</script>' } };
  const result = unwrap(await renderRequest({ ...request, raw: injected, piece: 'badge', op: 'export', format: 'html' }));
  const html = Buffer.from(result.bytes, 'base64').toString();
  assert.ok(html.includes('function placeTip('));
  assert.ok(!html.includes('</script><script>alert(1)</script>'));
});

test('GIF preserves transparent pixels and explicit reduced motion produces a still HTML document', async () => {
  const gif = unwrap(await renderRequest({ ...request, op: 'export', format: 'gif', transparent: true, frameCount: 2, width: 80 }));
  const bytes = Buffer.from(gif.bytes, 'base64');
  const controls: number[] = [];
  for (let i = 0; i + 7 < bytes.length; i++) if (bytes[i] === 0x21 && bytes[i + 1] === 0xf9 && bytes[i + 2] === 4 && bytes[i + 7] === 0) controls.push(bytes[i + 3]);
  assert.equal(controls.length, 3);
  assert.ok(controls.every(flags => (flags & 1) !== 0), 'every frame declares a transparent palette entry');
  const result = unwrap(await renderRequest({ ...request, op: 'export', format: 'html', reducedMotion: true }));
  const html = Buffer.from(result.bytes, 'base64').toString();
  assert.ok(!html.includes('htmlPlayer('));
  assert.ok(html.includes('.piece,.screen .reel{animation:none!important}'));
});

test('transparent Satori layouts render and animate without undefined CSS properties', async () => {
  for (const [style, piece, variant] of [
    ['minimal-light', 'receipt', 'themed'], ['film', 'tier', 'original'],
    ['glass', 'roll', 'scroll'], ['arcade', 'roll', 'crew'],
  ]) {
    const selection = { ...request, style, piece, variant, transparent: true };
    const result = unwrap(await renderRequest(selection)).art;
    assert.equal(result.bg, 'transparent');
    assert.ok(raster.rgba(result.svg, 120).data[3] < 255, `${style}/${piece}/${variant}: no opaque canvas`);
    const frames = unwrap(await renderRequest({ ...selection, op: 'frames', frameCount: 2 })).frames;
    assert.equal(frames.length, 3);
    assert.ok(frames.every(frame => frame.bg === 'transparent'));
  }
});

test('registered project context is cloned, inherited and reused across sampled frames', async () => {
  const contexts: unknown[] = [];
  const backend = { ...nativeBackend,
    classicArt: async (...args: Parameters<typeof nativeBackend.classicArt>) => { contexts.push(args[3].projects); return nativeBackend.classicArt(...args); },
    classicFrames: async (...args: Parameters<typeof nativeBackend.classicFrames>) => { contexts.push(args[3].projects); return nativeBackend.classicFrames(...args); },
  };
  const session = createRenderSession(backend);
  const call = (patch: object) => session.dispatch({ version: 1, id: 'projects', ...patch });
  const projects = [structuredClone(raw), { ...structuredClone(raw), project: { ...raw.project, name: 'Second project' } }];
  unwrap(await call({ op: 'archive', archiveId: 'all', raw, projects }));
  projects[0].project.name = 'Caller mutation';
  const options = { op: 'render', archiveId: 'all', style: 'minimal-light', piece: 'profile', lang: 'en' };
  const first = unwrap(await call(options)).art;
  assert.ok(first && !first.svg.includes('Caller mutation'));
  unwrap(await call({ ...options, op: 'frames', frameCount: 2 }));
  assert.ok(contexts.length >= 2);
  assert.equal(contexts[0], contexts[1], 'whole prepared context array is reused');
  assert.equal((contexts[0] as any[])[0].project, raw.project.name);
  const overridden = await call({ ...options, projects: [] });
  assert.equal(overridden.ok, false);
  if (!overridden.ok) assert.equal(overridden.error.code, 'missing_data');
  assert.ok(session.stats().archiveBytes > JSON.stringify(raw).length);
});
