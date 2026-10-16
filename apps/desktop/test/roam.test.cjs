const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

// Run the shipped placement/growth functions with cheap SVGs, without booting Tauri.
const main = fs.readFileSync(require.resolve('../src/main.ts'), 'utf8');
const code = ts.transpile(main.slice(main.indexOf('type RItem ='), main.indexOf('/** 墙上那件作品借用')), { target: ts.ScriptTarget.ES2022 });
function scene(missing) {
  const calls = [];
  const frames = [];
  const hitMounts = [];
  const node = () => ({ style: {}, querySelector: node, appendChild() {}, remove() {}, addEventListener() {} });
  const plane = node();
  let ctx;
  const host = { console, requestAnimationFrame: (cb) => { frames.push(cb); return frames.length; }, innerWidth: 1000, innerHeight: 700,
    store: { get: () => 'roam' }, tiles: () => Array.from({ length: 25 }, (_, i) => ({ id: String(i) })),
    keyOf: (b) => b.id, curVariant: () => undefined,
    state: { style: { variants: () => [], pieceName: (id) => id } },
    drawArt: async (id) => {
      calls.push(id);
      // The old bug never yields to a timer; stop it deterministically to report failure.
      if (calls.length === 500) vm.runInContext('roam.gen++; roam.ready = false', ctx);
      return missing(id) ? null : { w: 500, h: 400, svg: '<svg/>', hits: [] };
    },
    $: () => plane, document: { createElement: node }, esc: String, L: (s) => s, mountHits: (...args) => hitMounts.push(args),
  };
  ctx = vm.createContext(host);
  vm.runInContext(code + '\nglobalThis.roam = roam; globalThis.grow = grow; globalThis.place = place; globalThis.mountRoamHits = mountRoamHits; globalThis.resizeRoam = resizeRoam; globalThis.mountPreview = mountPreview; globalThis.releasePreview = releasePreview;', ctx);
  ctx.roam.ready = true;
  return { ctx, calls, frames, hitMounts };
}
const settled = () => new Promise(setImmediate);

test('an entirely empty column yields instead of retrying forever', async () => {
  const { ctx, calls } = scene(() => true);
  await ctx.grow(8, -700, 1400);
  await settled();
  assert.ok(calls.length < 500, 'empty artwork repeatedly restarted the growth loop');
  assert.equal(ctx.roam.gen, 0);
  const before = calls.length;
  await settled();
  assert.equal(calls.length, before);
});

test('missing first artwork is skipped and later artwork fills the viewport', async () => {
  const { ctx, calls } = scene((id) => id === '8');
  await ctx.grow(8, -700, 1400);
  await settled();
  assert.ok(calls.length < 500);
  const items = ctx.roam.cols.get(8).items;
  assert.ok(items.length > 0);
  assert.equal(items.find((x) => x.r >= 0).r, 1);
  assert.equal(new Set(items.map((x) => x.r)).size, items.length);
  assert.ok(items[0].y - 30 <= -700);
  assert.ok(items.at(-1).y + items.at(-1).h >= 1400);
});

test('skipping artwork in either direction preserves existing tile identities', async () => {
  const { ctx } = scene((id) => id === '8');
  await ctx.grow(3, -700, 1400);
  await settled();
  const col = ctx.roam.cols.get(3);
  for (const item of col.items) assert.equal(String(((3 + item.r * 5) % 25 + 25) % 25), item.id);
  const before = col.items.map((x) => [x, x.r]);
  await ctx.grow(3, -4000, 5000);
  await settled();
  for (const [item, row] of before) assert.equal(item.r, row);
  assert.equal(new Set(col.items.map((x) => x.r)).size, col.items.length);
});


test('many placement requests are coalesced into one animation frame', () => {
  const { ctx, frames } = scene(() => true);
  for (let i = 0; i < 100; i++) ctx.place();
  assert.equal(frames.length, 1);
  ctx.roam.ready = false;
  frames.shift()(0);
  ctx.roam.ready = true;
  ctx.place();
  assert.equal(frames.length, 1);
});

test('roaming hit regions are created only on the first hover', () => {
  const { ctx, hitMounts } = scene(() => true);
  let listener;
  const root = { addEventListener: (name, fn, options) => { assert.equal(name, 'pointerenter'); assert.equal(options.once, true); listener = fn; } };
  const art = { hits: [{x: 1}], w: 100, h: 200 };
  ctx.mountRoamHits(root, art);
  assert.equal(hitMounts.length, 0);
  listener();
  assert.equal(hitMounts.length, 1);
  assert.equal(hitMounts[0][0], root);
  assert.equal(hitMounts[0][1], art.hits);
});

function renderClient() {
  const workers = [];
  class Worker {
    constructor() { this.messages = []; workers.push(this); }
    postMessage(message) { this.messages.push(message); }
    terminate() { this.terminated = true; }
  }
  const kits = fs.readFileSync(require.resolve('../src/kits.ts'), 'utf8');
  const snippet = kits.slice(kits.indexOf('let renderer: Worker'), kits.indexOf('const classicKits: Kit[]')).replaceAll('import.meta.url', "'file:///kits.ts'");
  const ctx = vm.createContext({ Worker, URL, group: 'auto', devicePixelRatio: 2 });
  vm.runInContext(ts.transpile(snippet.replace('export const previewOf', 'const previewOf'), {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.renderAway = renderAway;', ctx);
  return {ctx, workers};
}

test('worker jobs share the raw snapshot and match out-of-order replies by request', async () => {
  const {ctx, workers} = renderClient(), raw = { project: {name:'sample'} };
  const first = ctx.renderAway(raw,'minimal-light','zh','share',undefined,true);
  const second = ctx.renderAway(raw,'minimal-light','zh','tier',undefined,true);
  const worker = workers[0], [a,b] = worker.messages;
  assert.equal(a.raw, raw);
  assert.equal('raw' in b, false);
  worker.onmessage({data:{seq:b.seq,art:{title:'tier'}}});
  worker.onmessage({data:{seq:a.seq,art:{title:'share'}}});
  assert.equal((await first).title,'share');
  assert.equal((await second).title,'tier');
});

test('a worker failure rejects pending artwork instead of leaving it waiting', async () => {
  const {ctx, workers} = renderClient();
  const job = ctx.renderAway({},'minimal-light','zh','share',undefined,true);
  const rejected = assert.rejects(job,/renderer broke/);
  workers[0].onerror({message:'renderer broke'});
  await rejected;
  assert.equal(workers[0].terminated,true);
});


test('fullscreen resizing reflows existing artwork without discarding it', async () => {
  const {ctx} = scene(() => false);
  await ctx.grow(0,-700,1400);
  const items = ctx.roam.cols.get(0).items;
  const before = [...items];
  ctx.innerWidth = 2000; ctx.innerHeight = 1000;
  ctx.resizeRoam(380,1000,700);
  assert.equal(ctx.roam.cols.get(0).items,items);
  assert.equal(items.length,before.length);
  for(let i=0;i<items.length;i++) {
    assert.equal(items[i],before[i]);
    assert.equal(items[i].w,520);
    if(i) assert.ok(items[i].y >= items[i-1].y+items[i-1].h+86);
  }
});


test('thumbnail object URLs are released after load or removal, only once', () => {
  const {ctx} = scene(() => false);
  const released = []; let seq = 0, image;
  ctx.previewOf = () => ({});
  ctx.URL = {createObjectURL: () => `blob:${++seq}`, revokeObjectURL: url => released.push(url)};
  const root = {appendChild: img => {image=img;}};
  ctx.mountPreview(root,{w:100,h:200,title:'sample'});
  image.onload(); ctx.releasePreview(root);
  assert.deepEqual(released,['blob:1']);
  ctx.mountPreview(root,{w:100,h:200,title:'sample'});
  ctx.releasePreview(root); image.onload();
  assert.deepEqual(released,['blob:1','blob:2']);
});
