const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const source = main.slice(main.indexOf('function projectSettings('), main.indexOf('// ───────────────────────── 设置'));

test('project settings keep an integration failure visible and allow retry; only manual paths can unlink', async () => {
  let failed = true, calls = 0, removed = false;
  const wait = { remove() { removed = true; } };
  const sheet = { hidden: true, innerHTML: '', querySelector: () => wait };
  const project = { id: 'p', name: 'Project', path: '/main', paths: ['/worktree', '/manual'], pathKinds: { '/worktree': 'worktree', '/manual': 'manual' } };
  const ctx = vm.createContext({
    $: () => sheet, closeMenu() {}, closeBtn: () => '', markEl: () => '', markOf: () => null,
    UNREAD: [], autoOwn: () => null, autoDefault: () => true, canAnalyze: () => true,
    L: (_, en) => en, esc: String, short: String, icon: () => '', dayOf: String,
    sheetRedraw: null, plugProject: null, plugs: null,
    state: { projects: [project], current: project, lang: 'en' },
    api: async (cmd) => cmd === 'projects' ? [{ ...project }] : {},
    renderPlugs() { if (ctx.plugs) wait.remove(); },
    async loadPlugs(p) { calls++; if (p === ctx.plugProject) ctx.plugs = failed ? null : {}; },
  });
  vm.runInContext(ts.transpile(source, { target: ts.ScriptTarget.ES2022 }), ctx);
  ctx.projectSettings(project);
  await new Promise(setImmediate);
  assert.match(sheet.innerHTML, /Couldn't read the integrations/);
  assert.match(sheet.innerHTML, /data-x="plugs-retry"/);
  assert.match(sheet.innerHTML, /data-x="unlink" data-path="\/manual"/);
  assert.doesNotMatch(sheet.innerHTML, /data-x="unlink" data-path="\/worktree"/);
  assert.equal(removed, false);
  const unlink = { dataset: { x: 'unlink', path: '/manual' }, disabled: false };
  await sheet.onclick({ target: { closest: (selector) => selector === '[data-x]' ? unlink : null } });
  assert.notEqual(ctx.state.current, project, 'path changes refresh the project object');
  failed = false;
  const button = { dataset: { x: 'plugs-retry' }, disabled: false };
  await sheet.onclick({ target: { closest: (selector) => selector === '[data-x]' ? button : null } });
  await new Promise(setImmediate);
  assert.equal(calls, 2);
  assert.equal(removed, true);
  assert.doesNotMatch(sheet.innerHTML, /Couldn't read the integrations/);
});
