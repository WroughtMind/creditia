import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Readable, Writable } from 'node:stream';
import { dispatchRender, renderCatalog, MAX_REQUEST_BYTES } from '../src/protocol.ts';
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

test('native catalog preserves all 26 desktop styles and registry variants', async () => {
  const result = unwrap(await renderRequest({ version: 1, id: 'catalog', op: 'catalog' }));
  assert.equal(result.styles.length, 26);
  assert.equal(new Set(result.styles.map(s => s.id)).size, 26);
  assert.equal(result.styles.filter(s => s.renderer === 'classic').length, 15);
  assert.deepEqual(result.formats.map(f => f.id), ['png', 'svg', 'pdf', 'gif', 'html']);
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
    for (let i = 0; i < exported.length; i++) assert.deepEqual(normalizedArt(frame(piece, model, style, { variant }, i / (exported.length - 1))), normalizedArt(exported[i]));
  }
});

test('every catalog style/piece/layout keeps the shared renderer data and geometry', async () => {
  let count = 0;
  for (const style of renderCatalog().styles) {
    const theme = C.themeById(style.id);
    for (const piece of style.pieces) {
      if (piece.requires.length) continue; // profile/blame require separate CLI result data
      for (const variant of [undefined, ...piece.variants.map(v => v.id)]) {
        const group = count % 2 ? 'model' : 'tool', named = count % 3 === 0;
        const options = { lang: 'en' as const, group: group as 'tool' | 'model', named };
        const actual = unwrap(await renderRequest({ ...request, ...options, style: style.id, piece: piece.id, variant })).art;
        const expected = theme
          ? await C.art(piece.id, C.buildModel(raw, { director: false, ...options }), theme, { lang: 'en', variant })
          : art(piece.id, buildModel(raw, { ...options, sign: '' }), STYLES.find(s => s.id === style.id)!, { variant });
        assert.deepEqual(normalizedArt(actual), normalizedArt(expected), `${style.id}/${piece.id}/${variant ?? 'default'}`);
        count++;
      }
    }
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
  assert.ok(samples.every(x => !x.crop));
  const pixels = samples.map(x => raster.rgba(x.svg));
  assert.ok(pixels.every(x => x.w === 200 && x.h === 200));
  const center = (img: typeof pixels[number]) => [...img.data.subarray((100 * img.w + 100) * 4, (100 * img.w + 100) * 4 + 4)];
  assert.deepEqual(center(pixels[0]), [255, 0, 0, 255]);
  assert.deepEqual(center(pixels[1]), [0, 0, 255, 255]);
  const seen: string[] = [];
  const exporter = makeExporter({ ...raster, rgba(svg, width) { seen.push(svg); return raster.rgba(svg, width); } });
  const gif = exporter.gif(samples, { width: 200 });
  assert.deepEqual(seen, samples.map(x => x.svg));
  // GIF logical screen width/height (little endian, after GIF89a).
  assert.equal(gif[6] + (gif[7] << 8), 200);
  assert.equal(gif[8] + (gif[9] << 8), 200);
});

test('HTML honors language and static canvas size, and rejects resized scrolling HTML', async () => {
  const result = unwrap(await renderRequest({ ...request, op: 'export', format: 'html', size: 'square' }));
  const html = Buffer.from(result.bytes, 'base64').toString();
  assert.ok(html.includes('<html lang="en">'));
  assert.ok(html.includes('"w":1080,"h":1080'));
  const scrolling = await renderRequest({ ...request, op: 'export', style: 'ink', piece: 'roll', variant: 'scroll', format: 'html', size: 'square' });
  assert.equal(scrolling.ok, false);
  if (!scrolling.ok) assert.equal(scrolling.error.code, 'unsupported_size');
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
    [{ op: 'export', format: 'gif', piece: 'badge' }, 'not_animated'],
  ] as const) {
    const result = await dispatchRender({ ...request, ...patch }, blocked);
    assert.equal(result.ok, false);
    if (!result.ok) assert.equal(result.error.code, code, JSON.stringify(patch));
  }
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
});
