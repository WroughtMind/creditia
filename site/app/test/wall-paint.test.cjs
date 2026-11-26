const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const code = main.slice(main.indexOf('async function paint('), main.indexOf('/** 风格按钮外圈'));
const tick = () => new Promise(setImmediate);
function deferred() { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; }
class Element {
  constructor() {
    this.children = []; this.dataset = {}; this.style = {}; this.className = '';
    this.classList = { contains: c => this.className.split(' ').includes(c), add: (...cs) => { this.className += ' ' + cs.join(' '); },
      remove: c => { this.className = this.className.split(' ').filter(x => x !== c).join(' '); }, toggle: (c, on) => on ? this.classList.add(c) : this.classList.remove(c) };
  }
  appendChild(n) { n.remove(); this.children.push(n); n.parent = this; return n; }
  replaceChildren(...nodes) { this.children.forEach(n => { n.parent = null; }); this.children = []; nodes.forEach(n => this.appendChild(n)); }
  replaceWith(n) { const p = this.parent, i = p.children.indexOf(this); p.children[i] = n; n.parent = p; this.parent = null; }
  remove() { if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1); this.parent = null; }
  querySelectorAll(selector) { return this.children.filter(n => selector === '.slot' ? n.classList.contains('slot') : n.classList.contains('tile') && (!selector.includes(':not') || !n.classList.contains('slot'))); }
  querySelector(selector) { return selector === '.art' ? (this.art ??= new Element()) : this.querySelectorAll(selector)[0] ?? null; }
}
function scene(render) {
  const board = new Element(), old = new Element(), body = new Element(), notices = [];
  old.className = 'tile'; old.dataset.k = 'old'; board.appendChild(old);
  const style = { id: 'new', preview: async a => a, variants: () => [] };
  const ctx = vm.createContext({
    state: { model: {}, style, token: 0, arts: new Map() }, $: s => s === '#board' ? board : { scrollTop: 0 },
    applyView() {}, roam: { ready: false }, view: () => 'wall', tiles: () => [{ id: 'first' }, { id: 'later' }],
    document: { body, createElement: () => new Element() }, innerHeight: 600, keyOf: b => b.id, curVariant: () => '',
    slotFor(b) { const n = new Element(); n.className = 'tile slot'; n.dataset = { w: '640', h: '480', span: '' }; return n; },
    layout(target = board) { target.children.forEach((n, i) => Object.assign(n.style, { left: '0px', top: `${i * 1000}px`, width: '640px' })); Object.assign(target.style, { width: '640px', height: '2000px' }); },
    drawArt: render, dims: {}, dimKey: id => id, ring() {}, tileA11y() {}, tileHtml: () => '', mountPreview() {}, mountRoamHits() {}, wireTile() {},
    loaded() {}, loading: null, styleDone() {}, syncStatus() {}, store: { set() {} }, markError() {},
    toast: (...args) => notices.push(args), L: (_, en) => en, errText: String, console: { error() {} },
  });
  vm.runInContext(ts.transpile(code, { target: ts.ScriptTarget.ES2022 }), ctx);
  return { ctx, board, body, old, notices };
}
const art = { w: 640, h: 480 };

test('a new visible card replaces the wall while offscreen artwork is still pending', async () => {
  const slow = deferred(), s = scene(id => id === 'first' ? Promise.resolve(art) : slow.promise);
  const work = s.ctx.paint(); await tick();
  assert.equal(s.board.children[0].dataset.k, 'first');
  assert.equal(s.board.querySelectorAll('.slot').length, 1);
  assert.equal(s.body.classList.contains('busy'), true);
  assert.equal(s.board.inert, false);
  assert.equal(s.ctx.state.arts.get('first'), art);
  slow.resolve(art); await work;
  assert.deepEqual(s.board.children.map(n => n.dataset.k), ['first', 'later']);
  assert.equal(s.body.classList.contains('busy'), false);
  assert.equal(s.board.inert, false);
});

test('a failure before publishing keeps old artwork data and disables old-wall actions until retry succeeds', async () => {
  let fail = true;
  const s = scene(async id => { if (id === 'later' && fail) throw new Error('preview failed'); return art; });
  s.ctx.innerHeight = 1800;
  const oldArt = { w: 100, h: 100 }; s.ctx.state.arts.set('old', oldArt);
  await s.ctx.paint();
  assert.equal(s.board.children[0], s.old);
  assert.equal(s.ctx.state.arts.get('old'), oldArt);
  assert.equal(s.ctx.state.arts.has('first'), false);
  assert.equal(s.board.inert, true);
  fail = false; await s.ctx.paint();
  assert.equal(s.board.inert, false);
  assert.equal(s.ctx.state.arts.has('old'), false);
  assert.deepEqual([...s.ctx.state.arts.keys()], ['first', 'later']);
});

test('a failed first visible render preserves the old wall and offers retry', async () => {
  const s = scene(async () => { throw new Error('renderer stopped'); });
  await s.ctx.paint();
  assert.equal(s.board.children[0], s.old);
  assert.equal(s.body.classList.contains('busy'), false);
  assert.equal(s.board.inert, true);
  assert.match(s.notices[0][0], /renderer stopped/);
  assert.equal(s.notices[0][2].acts[0].label, 'Retry');
});

test('an obsolete paint cannot replace the latest wall or report its stale error', async () => {
  const slow = deferred(), s = scene(() => slow.promise);
  const stale = s.ctx.paint();
  s.ctx.drawArt = async () => art;
  await s.ctx.paint();
  const current = [...s.board.children];
  slow.reject(new Error('old request')); await stale;
  assert.deepEqual(s.board.children, current);
  assert.equal(s.notices.length, 0);
});
