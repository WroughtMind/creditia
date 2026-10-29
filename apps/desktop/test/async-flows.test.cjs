const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const load = (ctx, from, to) => vm.runInContext(ts.transpile(main.slice(main.indexOf(from), main.indexOf(to, main.indexOf(from))), { target: ts.ScriptTarget.ES2022 }), ctx);
const tick = () => new Promise(setImmediate);
function deferred() { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; }
function scene() {
  const nodes = new Map(), painted = [], opened = [];
  const ctx = vm.createContext({
    state: { projects: [], current: null, raw: null, model: null, arts: new Map(), openSeq: 0, token: 0, lang: 'zh', size: 'native', style: { id: 'minimal', warm() {} } },
    $: s => { if (!nodes.has(s)) nodes.set(s, { hidden: false, innerHTML: '', textContent: '' }); return nodes.get(s); },
    closeFocus() {}, store: { set() {} }, roamReset() {}, slotWall() {}, mark: { set() {} }, markOf: () => null,
    UNREAD: [], loading: null, markErr: false, stampOf: '', syncStatus() {}, setTimeout: f => { f(); return 0; },
    groupPref: () => 'tool', namesOn: () => true, setGroup() {}, setNamed() {},
    rebuildModel() { ctx.state.model = ctx.state.raw; }, paint: async () => painted.push(ctx.state.raw), refreshIfChanged() {},
    emptyView: null, emptyErr: false, empty() {}, painted: () => true, withProgress: (_name, run) => run(),
    openProject: async p => { opened.push(p); ctx.state.current = p; },
    swapRaw: async raw => { ctx.state.raw = raw; }, done() {}, flash() {}, markError() {}, toast() {},
    L: (zh, en) => en, errText: String, console, view: () => 'wall', exportJobs: new Set(), renderExports() {},
  });
  return { ctx, painted, opened };
}

test('a delayed project stamp cannot overwrite a subsequently opened project', async () => {
  const { ctx, painted } = scene(), slow = deferred();
  ctx.readStamp = p => p.id === 'A' ? slow.promise : Promise.resolve('B-stamp');
  ctx.creditsOf = async p => ({ project: p.id });
  load(ctx, 'async function openProject(', '// ───────────────────────── 档案变了');
  const a = ctx.openProject({ id: 'A', name: 'A' }); await tick();
  await ctx.openProject({ id: 'B', name: 'B' }); slow.resolve('A-stamp'); await a;
  assert.equal(ctx.state.current.id, 'B');
  assert.equal(ctx.state.raw.project, 'B');
  assert.equal(ctx.stampOf, 'B-stamp');
  assert.deepEqual(painted.map(x => x.project), ['B']);
});

for (const pendingStage of ['analyze', 'credits', 'stamp']) test(`analysis finishing after a project switch during ${pendingStage} preserves the selected project`, async () => {
  const { ctx, opened } = scene(), slow = deferred(), a = { id: 'A', name: 'A' }, b = { id: 'B', name: 'B' };
  ctx.state.current = a; ctx.state.raw = { project: 'A' };
  ctx.api = async cmd => cmd === pendingStage ? slow.promise : cmd === 'projects' ? [a, b] : cmd === 'credits' ? { project: 'A-new' } : {};
  ctx.readStamp = () => pendingStage === 'stamp' ? slow.promise : Promise.resolve('A-stamp');
  load(ctx, 'async function reanalyze(', 'async function addProject(');
  const run = ctx.reanalyze(a, true); await tick();
  ctx.state.openSeq++; ctx.state.current = b; ctx.state.raw = { project: 'B' };
  slow.resolve(pendingStage === 'credits' ? { project: 'A-new' } : 'A-stamp'); await run;
  assert.equal(ctx.state.current.id, 'B'); assert.equal(ctx.state.raw.project, 'B'); assert.equal(opened.length, 0);
});

test('GIF content, language, privacy and output dimensions use the export click snapshot', async () => {
  const { ctx } = scene(), slow = deferred(), raw = { project: 'A' }, calls = [], files = [];
  ctx.state.model = { project: 'A' }; ctx.state.raw = raw; ctx.state.size = 'square';
  ctx.state.style.frames = (...args) => { calls.push(args); return []; };
  ctx.curVariant = () => 'original'; ctx.drawArt = () => slow.promise;
  ctx.exportArt = async (_fmt, _style, _art, size, frames) => { await frames(); files.push(size); return { ext: 'gif', bytes: [], mime: 'image/gif' }; };
  ctx.saveBytes = async name => files.push(name);
  load(ctx, 'async function doExport(', '/** 存文件');
  const run = ctx.doExport('gif', 'roll', 'roll#');
  ctx.state.raw = { project: 'B' }; ctx.state.lang = 'en'; ctx.state.size = 'story';
  ctx.groupPref = () => 'model'; ctx.namesOn = () => false;
  slow.resolve({}); await run;
  assert.equal(calls[0][0], raw); assert.equal(calls[0][1], 'zh');
  assert.equal(calls[0][4].group, 'tool'); assert.equal(calls[0][4].named, true);
  assert.deepEqual(files, ['square', 'A-roll-minimal-square.gif']);
});

test('manual analysis of another project reports its success, warnings and failure without navigating', async () => {
  for (const fail of [false, true]) for (const empty of [false, true]) {
    const { ctx, opened } = scene(), notices = [];
    const a = { id: 'A', name: 'A' }, b = { id: 'B', name: 'B' };
    ctx.state.current = a; ctx.state.raw = empty ? null : { project: 'A' };
    ctx.emptyView = () => ['Not analyzed']; ctx.painted = () => !empty;
    ctx.api = async cmd => {
      if (cmd === 'analyze') { if (fail) throw new Error('core_failed'); return { warnings: ['missing source'] }; }
      return [a, b];
    };
    ctx.done = (...args) => notices.push(args); ctx.toast = (...args) => notices.push(args);
    load(ctx, 'async function reanalyze(', 'async function addProject(');
    await ctx.reanalyze(b);
    assert.equal(ctx.state.current, a); assert.equal(ctx.state.raw?.project ?? null, empty ? null : 'A'); assert.equal(opened.length, 0);
    assert.equal(ctx.$('#empty').hidden, false);
    assert.match(notices[0][0], fail ? /B: .*core_failed/ : /B updated/);
    if (!fail) assert.deepEqual(notices[0][1], ['missing source']);
  }
});

test('a retained old wall cannot export under a pending or failed new style', async () => {
  const { ctx } = scene(); let rendered = false;
  ctx.$('#board').inert = true;
  ctx.drawArt = async () => { rendered = true; return {}; };
  load(ctx, 'async function doExport(', '/** 存文件');
  await ctx.doExport('png', 'share', 'share#');
  assert.equal(rendered, false);
});

test('an unavailable old wall does not block exporting a successfully rendered roaming card', async () => {
  const { ctx } = scene(); let exported = false;
  ctx.$('#board').inert = true; ctx.view = () => 'roam';
  ctx.state.model = { project: 'A' }; ctx.state.raw = {};
  ctx.curVariant = () => 'original'; ctx.drawArt = async () => ({});
  ctx.exportArt = async () => ({ ext: 'png', bytes: [], mime: 'image/png' });
  ctx.saveBytes = async () => { exported = true; };
  load(ctx, 'async function doExport(', '/** 存文件');
  await ctx.doExport('png', 'share', 'share#');
  assert.equal(exported, true);
});

test('parallel exports retain independent outcomes and suppress only an identical running export', async () => {
  const { ctx } = scene(), png = deferred(), calls = [], files = [];
  ctx.state.model = { project: 'A' }; ctx.state.raw = {};
  ctx.curVariant = () => 'original'; ctx.drawArt = async () => ({});
  ctx.exportArt = async fmt => { calls.push(fmt); if (fmt === 'png') await png.promise; return { ext: fmt, bytes: [], mime: 'test' }; };
  ctx.saveBytes = async name => { files.push(name); return '/exports/' + name; };
  load(ctx, 'async function doExport(', '/** 存文件');
  const first = ctx.doExport('png', 'share', 'share#'); await tick();
  await ctx.doExport('png', 'share', 'share#');
  await ctx.doExport('svg', 'share', 'share#');
  assert.deepEqual(calls, ['png', 'svg']);
  assert.deepEqual([...ctx.exportJobs].map(j => j.status), ['running', 'saved']);
  png.reject(new Error('rasterizer failed')); await first;
  assert.deepEqual([...ctx.exportJobs].map(j => j.status), ['failed', 'saved']);
  assert.match([...ctx.exportJobs][0].error, /rasterizer failed/);
  ctx.exportArt = async fmt => ({ ext: fmt, bytes: [], mime: 'test' });
  await ctx.doExport('png', 'share', 'share#');
  assert.equal([...ctx.exportJobs].at(-1).status, 'saved');
  assert.deepEqual(files, ['A-share-minimal.svg', 'A-share-minimal.png']);
});

test('a failed discovery preserves its error and can be retried', async () => {
  const { ctx } = scene(); let calls = 0;
  const error = { code: 'core_missing' };
  ctx.api = async () => { if (++calls === 1) throw error; return { projects: [{ name: 'A', path: '/A', exists: true }] }; };
  load(ctx, 'type Found =', 'const toolsOf');
  await assert.rejects(ctx.discover(), e => e === error);
  assert.equal((await ctx.discover())[0].path, '/A');
  assert.equal(calls, 2);
});

test('cancelling a first analysis restores that selection and does not start the next project', async () => {
  const { ctx } = scene(), events = [], calls = [];
  ctx.api = async (_cmd, args) => { calls.push(args.path); throw { code: 'cancelled' }; };
  load(ctx, 'async function addProjects(', '// ───────────────────────── 第一次打开');
  await ctx.addProjects(['/A', '/B'], (...args) => events.push(args));
  assert.deepEqual(events, [['/A', 'run'], ['/A', 'cancelled']]);
  assert.deepEqual(calls, ['/A']);
});

test('empty and failed startup register resize before taking their recovery paths', async () => {
  for (const fail of [false, true]) {
    const { ctx } = scene(), events = [];
    Object.assign(ctx, { native: false, bootKits() {}, renderDots() {}, renderDock() {}, wireTop() {}, wireRoam() {}, wireMarkTip() {}, wireKeys() {}, installMenu() {}, applyLang() {}, BRAND: [],
      innerWidth: 1440, innerHeight: 900, rcw: () => 300, addEventListener: name => events.push(name),
      INTERACT_CSS: '', document: { createElement: () => ({}), head: { appendChild() {} }, querySelectorAll: () => [], body: { classList: { add() {} } } },
      api: async () => { if (fail) throw new Error('unreadable'); return []; }, welcome() {},
      $: () => ({ appendChild() {}, addEventListener() {} }),
    });
    load(ctx, 'async function boot(', '/** 空状态。');
    await ctx.boot(); assert.ok(events.includes('resize'));
  }
});


test('an unexpected archive read failure is an error, not an unanalyzed project', async () => {
  for (const error of [new Error('connection lost'), { code: 'core_failed' }, { code: 'no_credits' }]) {
    const { ctx, painted } = scene(), states = [];
    ctx.native = false;
    ctx.readStamp = async () => '';
    ctx.creditsOf = async () => { throw error; };
    ctx.empty = (view, keepName, failed) => states.push({ text: view()[0], failed: !!failed });
    load(ctx, 'async function openProject(', '// ───────────────────────── 档案变了');
    await ctx.openProject({ id: 'A', name: 'A', path: '/A' });
    assert.equal(painted.length, 0);
    assert.equal(states[0].failed, error.code !== 'no_credits');
    assert.equal(states[0].text, error.code === 'no_credits' ? 'Not analyzed yet' : 'Cannot read this project');
  }
});
