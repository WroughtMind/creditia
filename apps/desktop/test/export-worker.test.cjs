const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const source = name => fs.readFileSync(require.resolve('../src/' + name), 'utf8').replace(/^import .*;\n/gm, '').replace(/^export /gm, '');
const load = (code, ctx) => vm.runInContext(ts.transpile(code, { target: ts.ScriptTarget.ES2022 }), ctx);
const art = { svg: '<svg/>', w: 1200, h: 630, title: 'Project A', hits: [], bg: '#111' };
const tick = () => new Promise(setImmediate);

function client(postError) {
  const workers = [];
  class Worker {
    constructor() { this.stopped = false; workers.push(this); }
    postMessage(job) { if (postError) throw postError; this.job = structuredClone(job); }
    terminate() { this.stopped = true; }
  }
  const ctx = vm.createContext({ Worker, URL, Error });
  load(source('engine.ts').replace('import.meta.url', JSON.stringify('file:///engine.ts')), ctx);
  return { ctx, workers };
}

test('a trapped export preserves its error and the next export gets a fresh runtime', async () => {
  const { ctx, workers } = client();
  const first = ctx.exportArt('png', null, art, 'native');
  const rejected = assert.rejects(first, e => e.message === 'memory access out of bounds' && e.stack === 'original WASM stack');
  workers[0].onmessage({ data: { error: { message: 'memory access out of bounds', stack: 'original WASM stack' } } });
  await rejected;
  assert(workers[0].stopped);
  const next = ctx.exportArt('png', null, art, 'native');
  assert.equal(workers.length, 2);
  const result = { bytes: Uint8Array.of(1, 2), ext: 'png', mime: 'image/png' };
  workers[1].onmessage({ data: { result } });
  assert.equal(await next, result);
  assert(workers[1].stopped);
});

for (const event of ['onerror', 'onmessageerror']) test(`${event} stops its export without affecting another in progress`, async () => {
  const { ctx, workers } = client();
  const first = ctx.exportArt('png', null, art, 'native');
  const second = ctx.exportArt('svg', null, art, 'native');
  const rejected = assert.rejects(first, /renderer failed|Unable to read/);
  workers[0][event]({ message: 'Export renderer failed' });
  await rejected;
  assert(workers[0].stopped); assert(!workers[1].stopped);
  workers[1].onmessage({ data: { result: 'second result' } });
  assert.equal(await second, 'second result');
});

test('an uncloneable export request closes its worker and reports the original error', async () => {
  const error = new Error('cannot clone request');
  const { ctx, workers } = client(error);
  await assert.rejects(ctx.exportArt('png', null, art, 'native'), e => e === error);
  assert(workers[0].stopped);
});

test('only GIF generates frames and only font data crosses the worker boundary', async () => {
  const { ctx, workers } = client();
  const style = { f: { body: { family: 'Inter', weight: 400 } }, draw() {} };
  let calls = 0;
  const frames = async () => { calls++; return [art, { ...art, svg: '<svg id="last"/>' }]; };
  for (const format of ['png', 'gif']) {
    const pending = ctx.exportArt(format, style, art, 'square', frames);
    await tick();
    const worker = workers.at(-1);
    assert.deepEqual(worker.job.fontStyle, { f: style.f });
    assert.equal(worker.job.size, 'square');
    assert.equal(worker.job.frames?.length, format === 'gif' ? 2 : undefined);
    worker.onmessage({ data: { result: {} } }); await pending;
  }
  assert.equal(calls, 1);
});

function renderer(failAt, cleanupFails = false) {
  const primary = new Error('original ' + failAt), counts = { images: 0, trees: 0, peakImages: 0, rendered: 0 };
  const fail = stage => { if (failAt === stage) throw primary; };
  class Resvg {
    constructor() { fail('constructor'); counts.trees++; }
    render() {
      fail('render'); counts.images++; counts.rendered++;
      counts.peakImages = Math.max(counts.peakImages, counts.images);
      return {
        width: 2, height: 1,
        asPng() { fail('png'); return Uint8Array.of(1, 2, 3); },
        get pixels() { fail('rgba'); return Uint8Array.of(1, 2, 3, 255, 4, 5, 6, 255); },
        free() { counts.images--; if (cleanupFails) throw new Error('image cleanup failed'); },
      };
    }
    toString() { fail('flat'); return '<svg/>'; }
    free() { counts.trees--; if (cleanupFails) throw new Error('tree cleanup failed'); }
  }
  const messages = [];
  const ctx = vm.createContext({ Resvg, Error, TextEncoder, Uint8Array,
    initWasm: async () => {}, fetch: () => {}, wasmUrl: 'test.wasm', fontBytes: async () => [],
    SIZES: [], self: { postMessage: (data, transfer) => messages.push({ data, transfer }) },
    makeExporter: r => ({
      png: a => r.png(a.svg, a.w * 2),
      pdf: async a => r.rgba(a.svg, a.w).data,
      gif: frames => { for (const a of frames) r.rgba(a.svg, 800); return Uint8Array.of(71, 73, 70); },
      svg: a => r.flat(a.svg), html: arts => r.flat(arts[0].svg),
    }),
  });
  load(source('export-worker.ts'), ctx);
  return { ctx, counts, primary, messages };
}

for (const format of ['png', 'pdf', 'gif', 'svg', 'html']) test(`${format} frees each WASM image and tree before returning bytes`, async () => {
  const { ctx, counts } = renderer();
  const out = await ctx.run({ fmt: format, fontStyle: null, art, size: 'native', frames: Array(24).fill(art) });
  assert.equal(out.ext, format); assert(out.bytes.length > 0);
  assert.equal(counts.images, 0); assert.equal(counts.trees, 0);
  assert.equal(counts.peakImages, ['svg', 'html'].includes(format) ? 0 : 1);
  if (format === 'gif') assert.equal(counts.rendered, 24);
});

for (const [stage, format] of [['constructor', 'png'], ['render', 'png'], ['png', 'png'], ['rgba', 'pdf'], ['flat', 'svg']]) test(`${stage} failure releases owned objects and is not replaced by a cleanup error`, async () => {
  const { ctx, counts, primary } = renderer(stage, true);
  await assert.rejects(ctx.run({ fmt: format, fontStyle: null, art, size: 'native' }), e => e === primary);
  assert.equal(counts.images, 0); assert.equal(counts.trees, 0);
});

test('a cleanup error on a successful render still fails the export', async () => {
  const { ctx, counts } = renderer(null, true);
  await assert.rejects(ctx.run({ fmt: 'png', fontStyle: null, art, size: 'native' }), /image cleanup failed/);
  assert.equal(counts.images, 0); assert.equal(counts.trees, 0);
});

test('worker transfers successful bytes and sends the original failure without fake output', async () => {
  const success = renderer();
  await success.ctx.self.onmessage({ data: { fmt: 'png', fontStyle: null, art, size: 'native' } });
  const sent = success.messages[0];
  assert.equal(sent.transfer[0], sent.data.result.bytes.buffer);
  const failure = renderer('render', true);
  await failure.ctx.self.onmessage({ data: { fmt: 'png', fontStyle: null, art, size: 'native' } });
  assert.equal(failure.messages[0].data.error.message, failure.primary.message);
  assert.equal(failure.messages[0].data.error.stack, failure.primary.stack);
  assert(!failure.messages[0].data.result);
});
