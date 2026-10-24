const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

const exportsOf = {};
vm.runInNewContext(ts.transpile(fs.readFileSync(require.resolve('../src/wall.ts'), 'utf8'), {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
}), { exports: exportsOf });
const { wallTiles, tileKey, wallVariant, selectWallVariant } = exportsOf;

function kit() {
  const variants = { tier: ['rows', 'center', 'split'], badge: ['flat', 'stack', 'emblem'] };
  return {
    board: [{ id: 'share', span: 2 }, { id: 'tier' }, { id: 'badge' }, { id: 'badge', variant: 'flat' }, { id: 'badge', variant: 'stack' }],
    variants: id => variants[id] ?? [],
    current: (id, variant) => variant ?? variants[id]?.[0],
  };
}
const values = (style, selected) => Array.from(wallTiles(style), tile => `${tile.id}#${wallVariant(style, selected, tileKey(tile)) ?? ''}`).sort();

test('wall contains every distinct layout once, including implicit defaults and explicit badge entries', () => {
  const style = kit(), original = JSON.stringify(style.board);
  assert.deepEqual(values(style, {}), ['badge#emblem', 'badge#flat', 'badge#stack', 'share#', 'tier#center', 'tier#rows', 'tier#split']);
  assert.deepEqual(Array.from(wallTiles(style).slice(0, 4), tileKey), ['share#', 'tier#rows', 'badge#flat', 'badge#stack']);
  assert.equal(wallTiles(style)[0].span, 2);
  assert.equal(JSON.stringify(style.board), original, 'roaming keeps its existing source sequence');
});

test('hover cycling swaps layouts without dropping one or adding a duplicate, even after rapid choices', () => {
  const style = kit(), selected = {}, original = values(style, selected);
  for (const variant of ['center', 'split', 'rows', 'center', 'rows']) {
    selectWallVariant(style, selected, 'tier#rows', variant);
    assert.equal(wallVariant(style, selected, 'tier#rows'), variant);
    assert.deepEqual(values(style, selected), original);
  }
  selectWallVariant(style, selected, 'tier#center', 'split');
  assert.deepEqual(values(style, selected), original);
  assert.equal(wallVariant(style, selected, 'share#'), undefined);
  assert.equal(wallVariant(style, selected, 'badge#flat'), 'flat');
});

test('adopting a roaming layout and selecting a layout in focus share the same unique catalog', () => {
  const style = kit(), selected = {}, original = values(style, selected);
  selectWallVariant(style, selected, 'badge#flat', 'emblem');
  assert.equal(wallVariant(style, selected, 'badge#emblem'), 'flat');
  selectWallVariant(style, selected, 'badge#flat', 'stack');
  assert.deepEqual(values(style, selected), original);
});

test('an unavailable layout or an old style key cannot corrupt a new wall', () => {
  const style = kit(), selected = {};
  assert.equal(selectWallVariant(style, selected, 'tier#rows', 'missing').length, 0);
  assert.equal(selectWallVariant(style, selected, 'tier#old', 'rows').length, 0);
  assert.deepEqual(selected, {});
});

function switching(render) {
  const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
  const code = main.slice(main.indexOf('function applyVariant('), main.indexOf('/** 同一作品的版式一起换好'));
  const style = kit(), notices = [], actions = [];
  const state = { style, token: 0, openSeq: 0, variants: {} };
  style.preview = async art => art;
  let busy = false;
  const ctx = vm.createContext({
    state, selectWallVariant, drawArt: render, view: () => 'wall', groupPref: () => 'tool',
    curVariant: key => wallVariant(style, state.variants, key),
    document: { body: { classList: { contains: () => busy } } },
    paint: () => { state.token++; actions.push('paint'); },
    repaintPiece: id => actions.push(id),
    L: zh => zh, toast: e => notices.push(e), errText: String,
  });
  vm.runInContext(ts.transpile(code, { target: ts.ScriptTarget.ES2022 }) + '\nglobalThis.requestedVariant = requestedVariant;', ctx);
  return { ctx, state, notices, actions, setBusy: value => { busy = value; } };
}

test('a layout returning null or a failed preview leaves the previous wall selection intact', async () => {
  const scene = switching(async () => null);
  await scene.ctx.changeVariant('tier', 'tier#rows', 'center');
  assert.deepEqual(scene.state.variants, {});
  assert.deepEqual(scene.actions, []);
  assert.equal(scene.notices.length, 1);
  const failure = switching(async () => ({}));
  failure.state.style.preview = async () => { throw new Error('preview failed'); };
  await failure.ctx.changeVariant('tier', 'tier#rows', 'center');
  assert.deepEqual(failure.state.variants, {});
  assert.equal(failure.notices.length, 1);
});

test('rapid choices commit only the latest successful result and ignore replies from a previous project', async () => {
  const pending = new Map();
  const scene = switching((_id, variant) => new Promise(resolve => pending.set(variant, resolve)));
  const first = scene.ctx.changeVariant('tier', 'tier#rows', 'center');
  assert.equal(scene.ctx.requestedVariant('tier#rows'), 'center');
  const last = scene.ctx.changeVariant('tier', 'tier#rows', 'split');
  pending.get('split')({}); await last;
  pending.get('center')({}); await first;
  assert.equal(wallVariant(scene.state.style, scene.state.variants, 'tier#rows'), 'split');
  assert.deepEqual(scene.actions, ['tier']);
  const stale = scene.ctx.changeVariant('tier', 'tier#rows', 'center');
  scene.state.token++; scene.state.openSeq++; scene.state.variants = {};
  pending.get('center')({}); await stale;
  assert.deepEqual(scene.state.variants, {});
});

test('a choice during a batch repaint invalidates the older whole-wall batch', async () => {
  const scene = switching(async () => ({})); scene.setBusy(true);
  await scene.ctx.changeVariant('tier', 'tier#rows', 'center');
  assert.equal(scene.state.token, 1);
  assert.deepEqual(scene.actions, ['paint']);
  assert.equal(wallVariant(scene.state.style, scene.state.variants, 'tier#rows'), 'center');
});

test('restarting a batch preserves pending choices on other artwork in the same context', async () => {
  const pending = new Map();
  const scene = switching((id) => new Promise(resolve => pending.set(id, resolve))); scene.setBusy(true);
  const tier = scene.ctx.changeVariant('tier', 'tier#rows', 'center');
  const badge = scene.ctx.changeVariant('badge', 'badge#flat', 'stack');
  pending.get('tier')({}); await tier;
  pending.get('badge')({}); await badge;
  assert.equal(wallVariant(scene.state.style, scene.state.variants, 'tier#rows'), 'center');
  assert.equal(wallVariant(scene.state.style, scene.state.variants, 'badge#flat'), 'stack');
  assert.deepEqual(scene.actions, ['paint', 'paint']);
});
