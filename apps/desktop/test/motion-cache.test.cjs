const { test } = require('node:test');
const assert = require('node:assert/strict');
const { cachedMotion } = require('../src/motion-cache.ts');

test('completed motion is reused only for the same archive and display key', async () => {
  const raw = {}, signal = new AbortController().signal;
  let calls = 0;
  const generate = async () => { calls++; return [new Blob(['start']), new Blob(['end'])]; };
  const first = await cachedMotion(raw, 'ink/zh/tier/rows/tool/named', signal, generate);
  assert.equal(await cachedMotion(raw, 'ink/zh/tier/rows/tool/named', signal, generate), first);
  await cachedMotion(raw, 'ink/en/tier/rows/tool/named', signal, generate);
  await cachedMotion({}, 'ink/zh/tier/rows/tool/named', signal, generate);
  assert.equal(calls, 3);
});

test('cancelled and failed preparation cannot poison a later replay', async () => {
  const raw = {}, controller = new AbortController();
  const frames = [new Blob(['start']), new Blob(['end'])];
  await assert.rejects(cachedMotion(raw, 'key', controller.signal, async () => { controller.abort(); return frames; }), { name: 'AbortError' });
  let calls = 0;
  const signal = new AbortController().signal;
  await assert.rejects(cachedMotion(raw, 'key', signal, async () => { throw Error('failed'); }));
  assert.equal(await cachedMotion(raw, 'key', signal, async () => { calls++; return frames; }), frames);
  assert.equal(calls, 1);
  await assert.rejects(cachedMotion(raw, 'key', controller.signal, async () => frames), { name: 'AbortError' });
});

test('completed frame cache evicts the least recently used animation within its memory budget', async () => {
  const raw = {}, signal = new AbortController().signal;
  const frames = [new Blob([new Uint8Array(9 * 1024 * 1024)]), new Blob(['end'])];
  let calls = 0;
  const generate = async () => { calls++; return frames; };
  for (const key of ['a', 'b', 'c', 'a', 'd', 'a']) await cachedMotion(raw, key, signal, generate);
  assert.equal(calls, 4);
  await cachedMotion(raw, 'b', signal, generate);
  assert.equal(calls, 5);
});
