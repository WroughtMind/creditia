const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const tick = () => new Promise(setImmediate);
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };
const source = fs.readFileSync(require.resolve('../src/focus-motion.ts'), 'utf8').replace(/^import .*;\n/gm, '').replace(/^export /gm, '');
const scrollSource = fs.readFileSync(require.resolve('../../../packages/render/src/scroll-motion.ts'), 'utf8').replace(/^export /gm, '');
const art = { w: 100, h: 100, bg: '#000' };
function scene({ reduced = false, decode = async () => {} } = {}) {
  const classes = new Set(), children = [], urls = new Set(), revoked = [], tasks = new Map(), errors = [], placed = [], animations = [];
  const listeners = new Set(), media = { matches: reduced, addEventListener: (_, f) => listeners.add(f), removeEventListener: (_, f) => listeners.delete(f) };
  let seq = 0;
  const root = { parentElement: { scrollTop: 0, scrollHeight: 500 }, classList: { add: (...xs) => xs.forEach(x => classes.add(x)), remove: (...xs) => xs.forEach(x => classes.delete(x)) }, appendChild(e) { children.push(e); e.parent = this; } };
  const element = () => ({ style: {}, appendChild(e) { this.image = e; }, remove() { const at = children.indexOf(this); if (at !== -1) children.splice(at, 1); } });
  const ctx = vm.createContext({ AbortController, Error, matchMedia: () => media,
    URL: { createObjectURL: () => { const u = 'blob:' + ++seq; urls.add(u); return u; }, revokeObjectURL: u => { revoked.push(u); urls.delete(u); } },
    Image: class { constructor() { Object.assign(this, element()); } decode() { return decode(); } animate(keys, options) { const a = { keys, options, cancelled: false, cancel() { this.cancelled = true; } }; animations.push(a); return a; } },
    document: { createElement: element },
    requestAnimationFrame: f => { const id = ++seq; tasks.set(id, f); return id; }, cancelAnimationFrame: id => tasks.delete(id),
  });
  vm.runInContext(ts.transpile(scrollSource + '\n' + source, { target: ts.ScriptTarget.ES2022 }), ctx);
  return { ctx, root, classes, children, urls, revoked, errors, listeners, media, animations, placed,
    play: (frames, a = art) => ctx.playFocusMotion(root, a, {}, frames, 1600, size => placed.push(size), e => errors.push(e)),
    frame(now) { const entries = [...tasks.values()]; tasks.clear(); entries.forEach(f => f(now)); },
  };
}

test('focus stays static until frames are decoded, plays once, then frees every animation URL', async () => {
  const s = scene(), ready = deferred();
  s.play(() => ready.promise);
  assert.equal(s.children.length, 0);
  ready.resolve([{}, {}, {}]); await tick();
  assert.equal(s.children.length, 0);
  s.frame(100); assert.equal(s.children.length, 1); assert(s.classes.has('motion-playing'));
  s.frame(1000); assert.equal(s.children.length, 1);
  s.frame(1700); assert.equal(s.children.length, 0); assert.equal(s.urls.size, 0);
  assert.equal(s.revoked.length, 3); assert.equal(s.listeners.size, 0);
  assert.equal(s.errors.length, 0); assert.equal(s.placed.at(-1), art);
});

test('cancelling a pending animation aborts its renderer and ignores its late result', async () => {
  const s = scene(), ready = deferred(); let signal;
  const stop = s.play(input => { signal = input; return ready.promise; });
  stop(); assert(signal.aborted);
  ready.resolve([{}, {}]); await tick(); s.frame(1);
  assert.equal(s.urls.size, 0); assert.equal(s.children.length, 0); assert.equal(s.errors.length, 0);
});

test('cancelling while images decode releases their URLs once and never starts playback', async () => {
  const decoding = deferred(), s = scene({ decode: () => decoding.promise });
  const stop = s.play(async () => [{}, {}]); await tick();
  assert.equal(s.urls.size, 2); stop(); stop();
  assert.equal(s.urls.size, 0); assert.equal(s.revoked.length, 2);
  decoding.resolve(); await tick(); s.frame(1);
  assert.equal(s.children.length, 0); assert.equal(s.errors.length, 0);
});

test('decode and renderer errors retain static content and reach the error handler', async () => {
  for (const decode of [false, true]) {
    const error = new Error(decode ? 'decode failed' : 'render failed');
    const s = scene({ decode: async () => { if (decode) throw error; } });
    s.play(async () => { if (!decode) throw error; return [{}, {}]; }); await tick();
    assert.equal(s.errors[0], error); assert.equal(s.urls.size, 0); assert.equal(s.children.length, 0); assert.equal(s.listeners.size, 0);
  }
});

test('reduced motion never requests frames and changing the preference stops an active animation', async () => {
  const reduced = scene({ reduced: true });
  reduced.play(() => { throw new Error('must not generate'); });
  assert.equal(reduced.listeners.size, 0); assert.equal(reduced.urls.size, 0);
  const s = scene(); s.play(async () => [{}, {}]); await tick(); s.frame(1);
  s.media.matches = true; [...s.listeners].forEach(f => f());
  assert.equal(s.children.length, 0); assert.equal(s.urls.size, 0);
});

test('scrolling credits use the existing long image and authored duration without generating frames', async () => {
  const s = scene(), scroll = { ...art, h: 500, motion: { kind: 'scroll', viewW: 100, viewH: 150, secs: 12, keys: [{ offset: 0, y: 0 }, { offset: 0.3, y: 100 }, { offset: 0.5, y: 100 }, { offset: 1, y: 350 }] } };
  s.play(() => { throw new Error('scroll must not generate frames'); }, scroll); await tick();
  assert.equal(s.animations[0].options.duration, 12000);
  assert.equal(s.animations[0].keys.at(-1).transform, 'translateY(-70%)');
  assert.equal(s.animations[0].keys[1].transform, s.animations[0].keys[2].transform);
  assert.equal(s.animations[0].keys[2].offset, 0.5);
  assert(s.classes.has('motion-scroll')); assert.equal(s.urls.size, 1);
  s.animations[0].onfinish();
  assert.equal(s.urls.size, 0); assert.equal(s.children.length, 0); assert.equal(s.placed.at(-1), scroll);
  assert.equal(s.root.parentElement.scrollTop, 500);
});

test('a queued scroll finish cannot move a replacement artwork after cancellation', async () => {
  const s = scene(), scroll = { ...art, h: 500, motion: { kind: 'scroll', viewW: 100, viewH: 150, secs: 12, keys: [{ offset: 0, y: 0 }, { offset: 0.3, y: 100 }, { offset: 0.5, y: 100 }, { offset: 1, y: 350 }] } };
  const stop = s.play(async () => [], scroll); await tick();
  const finish = s.animations[0].onfinish;
  stop(); finish();
  assert.equal(s.root.parentElement.scrollTop, 0);
  assert.equal(s.revoked.length, 1);
});

test('animation workers are isolated, cancellable, and preserve their request preferences', async () => {
  const kits = fs.readFileSync(require.resolve('../src/kits.ts'), 'utf8');
  const part = kits.slice(kits.indexOf('function framesAway'), kits.indexOf('const classicKits')).replace('import.meta.url', JSON.stringify('file:///kits.ts'));
  const workers = [];
  const ctx = vm.createContext({ AbortController, DOMException, Error, URL, Worker: class {
    constructor() { workers.push(this); } postMessage(job) { this.job = structuredClone(job); } terminate() { this.stopped = true; }
  } });
  vm.runInContext(ts.transpile(part, { target: ts.ScriptTarget.ES2022 }), ctx);
  const abort = new AbortController(), prefs = { group: 'model', named: false }, raw = { project: { name: 'original' } };
  const first = ctx.framesAway(raw, 'minimal-light', 'zh', 'share', 'bars', true, prefs, true, abort.signal);
  const exportFrames = ctx.framesAway(raw, 'minimal-light', 'en', 'share', 'bars', true, prefs, false);
  raw.project.name = 'changed'; prefs.named = true;
  assert.equal(workers[0].job.raw.project.name, 'original'); assert.equal(workers[0].job.named, false);
  const cancelled = assert.rejects(first, { name: 'AbortError' }); abort.abort(); await cancelled;
  assert(workers[0].stopped); assert(!workers[1].stopped);
  workers[1].onmessage({ data: { frames: [art] } }); assert.equal((await exportFrames)[0], art); assert(workers[1].stopped);
});
