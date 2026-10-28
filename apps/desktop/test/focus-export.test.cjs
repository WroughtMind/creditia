const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const from = main.indexOf('function openFocus(');
const code = ts.transpile(main.slice(from, main.indexOf('// ───────────────────────── 漫游', from)), { target: ts.ScriptTarget.ES2022 });
const tick = () => new Promise(setImmediate);
function deferred() { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; }
const artwork = title => ({ svg: `<svg id="${title}"/>`, w: 1200, h: 630, title, hits: [] });
function scene() {
  const exports = [], notices = [], classes = new Set();
  const kits = Object.fromEntries(['A', 'B', 'C'].map(id => [id, {
    id, variants: () => ['original', 'alt'], current: () => 'original', pieceName: () => id,
    variantName: x => x, name: () => id, preview: async () => ({ blob: id }),
  }]));
  const xp = { disabled: true }, element = {
    style: {}, classList: { add: n => classes.add(n), remove: n => classes.delete(n), toggle() {} },
    replaceChildren() {}, scrollIntoView() {}, remove() {},
  };
  const focus = {
    hidden: true, innerHTML: '', querySelectorAll: () => [],
    querySelector: s => s === '.xp' ? xp : ['.kits', '.vs .on'].includes(s) ? null : element,
  };
  const ctx = vm.createContext({
    state: { style: kits.A, raw: {}, arts: new Map(), lang: 'en' }, $: () => focus,
    curVariant: () => 'original', changeVariant: async () => {},
    document: { querySelectorAll: () => [{ dataset: { k: 'share#' } }], querySelector: () => null, body: { classList: { add() {}, remove() {} } } },
    resolveGroup: () => 'tool', MODEL_VARIANTS: {}, closeBtn: () => '', esc: x => x, L: (_zh, en) => en,
    icon: () => '', innerHeight: 900, mountPreview() {}, releasePreview() {}, mountRoamHits() {},
    drawArt: async (_id, _variant, kit) => artwork(kit.id), kitById: id => kits[id], closeFormats() {},
    toggleFormats: (_host, _id, run) => run('png'), doExport: (_fmt, _id, _key, options) => exports.push(options),
    toast: (...args) => notices.push(args), errText: String, closeFocus: null, focusKeys: null,
  });
  vm.runInContext(code, ctx);
  const click = dataset => focus.onclick({ target: { closest: () => ({ dataset, closest: s => s === '.kits' && dataset.s ? {} : null, parentElement: {} }) }, stopPropagation() {} });
  return { ctx, xp, classes, exports, notices, click, kits };
}

test('switching a focused card cannot export the old artwork under the new style', async () => {
  const s = scene(), slow = deferred();
  s.ctx.openFocus('share', 'share#'); await tick();
  assert.equal(s.xp.disabled, false);
  s.ctx.drawArt = (_id, _v, kit) => kit.id === 'B' ? slow.promise : Promise.resolve(artwork(kit.id));
  s.click({ s: 'B' });
  assert.equal(s.xp.disabled, true); assert(s.classes.has('loading'));
  s.click({ act: 'export' }); assert.equal(s.exports.length, 0);
  slow.resolve(artwork('B')); await tick();
  assert.equal(s.xp.disabled, false);
  s.click({ act: 'export' });
  assert.equal(s.exports[0].kit.id, 'B'); assert.equal(s.exports[0].art.title, 'B');
});

test('a failed focused render retains the old picture but requires a successful retry before export', async () => {
  const s = scene();
  s.ctx.openFocus('share', 'share#'); await tick();
  s.ctx.drawArt = async () => { throw new Error('font unavailable'); };
  s.click({ s: 'B' }); await tick();
  assert.equal(s.xp.disabled, true); assert(!s.classes.has('loading'));
  s.click({ act: 'export' }); assert.equal(s.exports.length, 0);
  assert.match(s.notices[0][0], /font unavailable/);
  s.ctx.drawArt = async () => artwork('B');
  s.notices[0][2].acts[0].run(); await tick();
  assert.equal(s.xp.disabled, false);
  s.click({ act: 'export' }); assert.equal(s.exports[0].art.title, 'B');
});

test('an obsolete focused render failure cannot disable or replace a newer completed style', async () => {
  const s = scene(), slow = deferred();
  s.ctx.openFocus('share', 'share#'); await tick();
  s.ctx.drawArt = (_id, _v, kit) => kit.id === 'B' ? slow.promise : Promise.resolve(artwork(kit.id));
  s.click({ s: 'B' }); s.click({ s: 'C' }); await tick();
  slow.reject(new Error('obsolete failure')); await tick();
  assert.equal(s.xp.disabled, false); assert.equal(s.notices.length, 0);
  s.click({ act: 'export' }); assert.equal(s.exports[0].kit.id, 'C'); assert.equal(s.exports[0].art.title, 'C');
});

test('a global layout change disables focused export while the wall selection is still pending', async () => {
  const s = scene(), slow = deferred();
  s.ctx.openFocus('share', 'share#'); await tick();
  s.ctx.changeVariant = () => slow.promise;
  s.click({ v: 'alt' });
  assert.equal(s.xp.disabled, true);
  s.click({ act: 'export' }); assert.equal(s.exports.length, 0);
  slow.resolve(); await tick();
  assert.equal(s.xp.disabled, false);
});

test('an earlier global layout completion cannot enable export while the last choice is still pending', async () => {
  const s = scene(), earlier = deferred(), last = deferred();
  s.ctx.openFocus('share', 'share#'); await tick();
  s.ctx.changeVariant = (_id, _key, variant) => variant === 'alt' ? earlier.promise : last.promise;
  s.click({ v: 'alt' }); s.click({ v: 'original' });
  earlier.resolve(); await tick();
  assert.equal(s.xp.disabled, true); assert(s.classes.has('loading'));
  s.click({ act: 'export' }); assert.equal(s.exports.length, 0);
  last.resolve(); await tick();
  assert.equal(s.xp.disabled, false);
});
