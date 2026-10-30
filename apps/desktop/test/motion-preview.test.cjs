const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

const source = fs.readFileSync(require.resolve('../../../packages/render/tools/motion-preview/worker.ts'), 'utf8').replace(/^import .*;\n/gm, '');
function worker(still, prepare = async () => {}) {
  const messages = [], generated = [];
  const render = () => still;
  const frames = () => { generated.push(true); return [{ svg: 'first' }, { svg: 'last' }]; };
  const self = { postMessage: value => messages.push(value) };
  vm.runInNewContext(ts.transpile(source, { target: ts.ScriptTarget.ES2022 }), {
    self, Error, prepare, classicModel: () => ({}), buildModel: () => ({}),
    themeById: () => ({}), styleById: () => ({}), art: render, classicArt: render,
    frames, classicFrames: frames,
  });
  return { messages, generated, send: data => self.onmessage({ data }) };
}

test('preview reduced motion renders only the complete static artwork in both renderers', async () => {
  for (const original of [true, false]) {
    const still = { svg: '<svg>complete</svg>' }, w = worker(still);
    await w.send({ init: true, raw: {}, fonts: [] });
    await w.send({ seq: 1, original, staticOnly: true });
    assert.equal(w.generated.length, 0);
    assert.equal(w.messages.length, 1);
    assert.equal(w.messages[0].still, still);
    assert.equal(w.messages[0].complete, true);
  }
});

test('preview scroll sends one existing reel without asking for sampled frames', async () => {
  const still = { svg: '<svg>reel</svg>', motion: { kind: 'scroll' } }, w = worker(still);
  await w.send({ init: true, raw: {}, fonts: [] });
  await w.send({ seq: 2, original: false });
  assert.equal(w.generated.length, 0);
  assert.equal(w.messages[0].still, still);
  assert.equal(w.messages[0].complete, true);
});

test('preview waits for initialization and reports failure against the current request', async () => {
  let reject;
  const w = worker({}, () => new Promise((_, fail) => { reject = fail; }));
  await w.send({ init: true, raw: {}, fonts: [] });
  const request = w.send({ seq: 3, original: false });
  assert.equal(w.messages.length, 0);
  reject(new Error('font initialization failed'));
  await request;
  assert.equal(w.generated.length, 0);
  assert.equal(w.messages[0].seq, 3);
  assert.equal(w.messages[0].error, 'font initialization failed');
});

function framePreparation() {
  const source = fs.readFileSync(require.resolve('../../../packages/render/tools/motion-preview/entry.ts'), 'utf8');
  const file = ts.createSourceFile('entry.ts', source, ts.ScriptTarget.ES2022, true);
  const helpers = file.statements.filter(node => ts.isFunctionDeclaration(node)
    && ['prepareFrameImages', 'resetFrameImages'].includes(node.name?.text)).map(node => node.getText(file)).join('\n');
  const waiting = [], revoked = [];
  let next = 0;
  class Image {
    style = {};
    decode() { return new Promise((resolve, reject) => waiting.push({ resolve, reject })); }
  }
  const context = vm.createContext({ Image, Blob, Promise, seq: 1, frameImages: [], frameUrls: [],
    URL: { createObjectURL: () => `blob:frame-${next++}`, revokeObjectURL: url => revoked.push(url) } });
  vm.runInContext(ts.transpile(helpers, { target: ts.ScriptTarget.ES2022 }), context);
  return { context, waiting, revoked };
}

test('preview keeps static content until every independent SVG frame has decoded', async () => {
  const { context, waiting } = framePreparation();
  const preparation = context.prepareFrameImages([{ svg: '<svg><image/></svg>' }, { svg: '<svg/>' }], 1);
  assert.equal(waiting.length, 2);
  waiting[0].resolve();
  await Promise.resolve();
  assert.equal(context.frameImages.length, 0);
  waiting[1].resolve();
  await preparation;
  assert.equal(context.frameImages.length, 2);
  assert.equal(context.frameImages[0].src, 'blob:frame-0');
});

test('preview discards late image decodes and releases URLs after switching or reducing motion', async () => {
  const { context, waiting, revoked } = framePreparation();
  const preparation = context.prepareFrameImages([{ svg: '<svg><image/></svg>' }], 1);
  context.seq = 2;
  context.resetFrameImages();
  waiting[0].resolve();
  await preparation;
  assert.equal(context.frameImages.length, 0);
  assert.equal(context.frameUrls.length, 0);
  assert.deepEqual(revoked, ['blob:frame-0']);
});
