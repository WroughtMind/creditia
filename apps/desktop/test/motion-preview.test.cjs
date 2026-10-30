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
