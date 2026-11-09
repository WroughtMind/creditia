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
  const host = { console, requestAnimationFrame: (cb) => { frames.push(cb); return frames.length; }, innerWidth: 1000, innerHeight: 700, devicePixelRatio: 2,
    mountRoamPreview() {}, releaseRoamPreview() {},
    store: { get: () => 'roam' }, tiles: () => Array.from({ length: 25 }, (_, i) => ({ id: String(i) })),
    keyOf: (b) => b.id, curVariant: () => undefined,
    state: { model: {}, style: { board: Array.from({ length: 25 }, (_, i) => ({ id: String(i) })), current: () => undefined, variants: () => [], pieceName: (id) => id, preview: async () => ({}) } },
    drawArt: async (id) => {
      calls.push(id);
      // The old bug never yields to a timer; stop it deterministically to report failure.
      if (calls.length === 500) vm.runInContext('roam.gen++; roam.ready = false', ctx);
      return missing(id) ? null : { w: 500, h: 400, svg: '<svg/>', hits: [] };
    },
    $: () => plane, document: { createElement: node }, esc: String, L: (s) => s, mountHits: (...args) => { hitMounts.push(args); return () => { hitMounts.pop(); }; },
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

test('leaving roam releases mounted pixels and returning keeps its position and artwork', async () => {
  const {ctx, frames} = scene(() => false);
  await ctx.grow(0,-700,1400);
  const columns = ctx.roam.cols, items = columns.get(0).items, released = [], cancelled = [];
  ctx.roam.x = 800; ctx.roam.y = 1200;
  let stops = 0;
  const moves = [];
  const native = freeViewport((x,y)=>{ctx.roam.x=x;ctx.roam.y=y;});
  native.viewport.moveTo(800,1200);
  native.viewport.moveTo(890,1320);
  ctx.viewportSpy = {
    stop(){stops++;native.viewport.stop();[ctx.roam.x,ctx.roam.y]=native.changes.at(-1);},
    moveTo(...p){moves.push(p);native.viewport.moveTo(...p);},
  };
  vm.runInContext('roamViewport = viewportSpy;',ctx);
  let selected = 'wall';
  ctx.store.get = () => selected;
  ctx.state.model = {};
  ctx.cancelAnimationFrame = id => cancelled.push(id);
  ctx.releaseRoamPreview = art => released.push(art);
  const classes = new Set(['roam']);
  ctx.document.body = {classList: {contains: key => classes.has(key), toggle: (key,on) => on ? classes.add(key) : classes.delete(key)}};
  ctx.document.querySelectorAll = () => [];
  const plane = {innerHTML:'cards'}, root = native.root, home = {};
  ctx.$ = selector => ({'#empty':{hidden:true},'#welcome':{hidden:true},'#roam':root,'#home':home,'#roam .plane':plane})[selector];
  const art = {};
  ctx.roam.els.set('0:0',{querySelector: () => art});
  const reset = main.slice(main.indexOf('function clearRoamElements()'), main.indexOf('function glide('));
  const apply = main.slice(main.indexOf('function applyView()'), main.indexOf('// ───────────────────────── 风格'));
  vm.runInContext(ts.transpile(reset + apply, {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.applyView = applyView;',ctx);
  ctx.applyView();
  assert.deepEqual(released,[art]);
  assert.equal(ctx.roam.els.size,0); assert.equal(plane.innerHTML,'');
  assert.equal(stops,1); assert.ok(cancelled.length); assert.equal(root.hidden,true);
  assert.equal(ctx.roam.cols,columns); assert.equal(columns.get(0).items,items);
  assert.equal(ctx.roam.x,890); assert.equal(ctx.roam.y,1320);
  const frameCount = frames.length;
  selected = 'roam'; ctx.applyView();
  assert.equal(root.hidden,false); assert.equal(frames.length,frameCount+1);
  assert.equal(columns.get(0).items,items);
  assert.deepEqual(moves,[[890,1320]]);
});

test('preview hit regions exist only while hovered and are skipped during a pan', () => {
  const { ctx, hitMounts } = scene(() => true);
  const listeners = new Map(); let panning = false, hovered = false;
  const root = { addEventListener: (name, fn) => listeners.set(name, fn), removeEventListener: (name) => listeners.delete(name), closest: () => panning, matches: () => hovered };
  const art = { hits: [{x: 1}], w: 100, h: 200 };
  ctx.mountRoamHits(root, art);
  assert.equal(hitMounts.length, 0);
  panning = true; listeners.get('pointerenter')();
  assert.equal(hitMounts.length, 0);
  panning = false; listeners.get('pointerenter')(); listeners.get('pointerenter')();
  assert.equal(hitMounts.length, 1);
  assert.equal(hitMounts[0][0], root);
  assert.equal(hitMounts[0][1], art.hits);
  listeners.get('pointerleave')();
  assert.equal(hitMounts.length, 0);
  listeners.get('pointerenter')();
  hovered = true;
  ctx.mountRoamHits(root, {...art, hits: []});
  assert.equal(hitMounts.length, 1);
  assert.equal(hitMounts[0][1].length, 0, 'a hovered replacement immediately uses the new artwork without a pointer re-entry');
});

function renderClient() {
  const workers = [];
  class Worker {
    constructor() { this.messages = []; workers.push(this); }
    postMessage(message) { this.messages.push(structuredClone(message)); }
    terminate() { this.terminated = true; }
  }
  const kits = fs.readFileSync(require.resolve('../src/kits.ts'), 'utf8');
  const snippet = kits.slice(kits.indexOf('let renderer: Worker'), kits.indexOf('const classicKits: Kit[]')).replaceAll('import.meta.url', "'file:///kits.ts'");
  const ctx = vm.createContext({ Worker, URL, group: 'auto', named: true, devicePixelRatio: 2 });
  vm.runInContext(ts.transpile(snippet.replace('export function wakeRenderer', 'function wakeRenderer'), {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.renderAway = renderAway; globalThis.previewAway = previewAway;', ctx);
  return {ctx, workers};
}

test('worker jobs share the raw snapshot and match out-of-order replies by request', async () => {
  const {ctx, workers} = renderClient(), raw = { project: {name:'sample'} };
  const first = ctx.renderAway(raw,'minimal-light','zh','share',undefined,true);
  const second = ctx.renderAway(raw,'minimal-light','zh','tier',undefined,true);
  const worker = workers[0], [a,b] = worker.messages;
  assert.deepEqual(a.raw, raw);
  assert.equal('raw' in b, false);
  // 设置里「写其他贡献者的名字」跟着每张作品传给排版线程
  assert.equal(a.named, true);
  worker.onmessage({data:{seq:b.seq,art:{title:'tier'}}});
  worker.onmessage({data:{seq:a.seq,art:{title:'share'}}});
  assert.equal((await first).title,'share');
  assert.equal((await second).title,'tier');
});

test('all views reuse the same resolution-independent preview per artwork', async () => {
  const {ctx, workers} = renderClient(), art = { svg: '<svg/>', w: 10, h: 10 };
  void ctx.renderAway({},'minimal-light','zh','share',undefined,true);
  const worker = workers[0];
  assert.equal('width' in worker.messages[0], false);
  const first = ctx.previewAway(art,'minimal-light',true), again = ctx.previewAway(art,'minimal-light',true);
  assert.equal(first, again);
  assert.equal(worker.messages.length, 2);
  const job = worker.messages[1];
  assert.equal(job.svg, art.svg); assert.equal('width' in job, false);
  worker.onmessage({data:{seq:job.seq,preview:'svg'}});
  assert.equal(await first, 'svg');
});

test('changing the aggregate language sends its updated project name to the worker', () => {
  const {ctx, workers} = renderClient();
  const original = { project: {name:'全部项目'}, contributors: [] };
  ctx.state = {current: {all:true}, raw: original, lang:'zh'};
  ctx.L = (zh,en) => ctx.state.lang === 'zh' ? zh : en;
  ctx.groupPref = () => 'auto'; ctx.namesOn = () => true;
  ctx.petalsOf = () => []; ctx.mark = { set() {} };
  ctx.buildModel = raw => ({project:raw.project.name, commits:0, linesAdded:0});
  ctx.$ = () => ({}); ctx.fmt = String; ctx.renderGroup = ctx.renderPlugs = () => {};
  const rebuild = main.slice(main.indexOf('function rebuildModel()'), main.indexOf('// ───────────────────────── 接入'));
  vm.runInContext(ts.transpile(rebuild, {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.rebuildModel = rebuildModel;', ctx);
  ctx.rebuildModel();
  ctx.renderAway(ctx.state.raw,'minimal-light','zh','share',undefined,true);
  ctx.state.lang = 'en'; ctx.rebuildModel();
  ctx.renderAway(ctx.state.raw,'minimal-light','en','share',undefined,true);
  const [first,second] = workers[0].messages;
  assert.equal(first.raw.project.name,'全部项目');
  assert.equal(second.raw?.project.name,'All projects');
  assert.equal(original.project.name,'全部项目');
  const snapshot = ctx.state.raw;
  ctx.rebuildModel();
  assert.equal(ctx.state.raw,snapshot);
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

test('resizing keeps a distant gap at the viewport center without jumping artwork', async () => {
  const {ctx} = scene(() => false);
  for (let i=0;i<3;i++) await ctx.grow(0,-700,45000);
  const anchor = ctx.roam.cols.get(0).items.find(t => t.r === 99);
  assert.ok(anchor);
  ctx.roam.x = (380+56)/2 - 1000/2;
  ctx.roam.y = anchor.y + anchor.h + 43 - 700/2;
  ctx.innerWidth = 2000; ctx.innerHeight = 1000;
  ctx.resizeRoam(380,1000,700);
  assert.ok(Math.abs(ctx.roam.y + 500 - (anchor.y + anchor.h + 43)) < 1e-6);
  ctx.innerWidth = 1000; ctx.innerHeight = 700;
  ctx.resizeRoam(520,2000,1000);
  assert.ok(Math.abs(ctx.roam.y + 350 - (anchor.y + anchor.h + 43)) < 1e-6);
});


test('thumbnail object URLs are released after load or removal, only once', () => {
  const {ctx} = scene(() => false);
  const released = []; let seq = 0, image;
  ctx.URL = {createObjectURL: () => `blob:${++seq}`, revokeObjectURL: url => released.push(url)};
  const root = {appendChild: img => {image=img;}};
  ctx.mountPreview(root,{w:100,h:200,title:'sample'},{});
  image.onload(); ctx.releasePreview(root);
  assert.deepEqual(released,['blob:1']);
  ctx.mountPreview(root,{w:100,h:200,title:'sample'},{});
  ctx.releasePreview(root); image.onload();
  assert.deepEqual(released,['blob:1','blob:2']);
});

test('an old image finishing cannot revoke a newer preview URL', () => {
  const {ctx} = scene(() => false);
  const released = [], images = []; let seq = 0;
  ctx.URL = {createObjectURL: () => `blob:${++seq}`, revokeObjectURL: url => released.push(url)};
  const root = {appendChild: img => images.push(img)};
  ctx.mountPreview(root,{w:100,h:200,title:'first'},{});
  ctx.mountPreview(root,{w:100,h:200,title:'second'},{});
  images[0].onload();
  assert.deepEqual(released,['blob:1']);
  images[1].onload();
  assert.deepEqual(released,['blob:1','blob:2']);
});

test('switching projects while artwork is queued does not enqueue its old preview', async () => {
  const {ctx} = scene(() => false);
  let finish, previews = 0;
  ctx.drawArt = () => new Promise(resolve => { finish = resolve; });
  ctx.state.style.preview = async () => { previews++; return {}; };
  const work = ctx.grow(0,-700,1400);
  ctx.roam.gen++;
  finish({w:500,h:400,svg:'<svg/>',hits:[]});
  await work;
  assert.equal(previews,0);
  assert.equal(ctx.roam.cols.get(0).items.length,0);
});

test('starting a slow project switch releases old roaming cards before its data arrives', async () => {
  const {ctx} = scene(() => false);
  let finish, previews = 0, released = 0;
  ctx.drawArt = () => new Promise(resolve => {finish=resolve;});
  ctx.state.style.preview = async () => {previews++;return {};};
  const work = ctx.grow(0,-700,1400);
  ctx.state.openSeq = 0;ctx.state.token = 0;ctx.state.arts = new Map();
  ctx.store.set = () => {};
  ctx.renderPlugs = ctx.slotWall = ctx.cancelAnimationFrame = () => {};
  ctx.mark = { set() {} }; ctx.markOf = () => null; ctx.UNREAD = [];
  ctx.syncStatus = () => {}; ctx.setTimeout = () => {}; ctx.closeFocus = null;
  ctx.readStamp = async () => '';
  ctx.creditsOf = () => new Promise(() => {});
  ctx.releaseRoamPreview = () => released++;
  const nodes = new Map();
  ctx.$ = selector => {if(!nodes.has(selector))nodes.set(selector,{});return nodes.get(selector);};
  ctx.roam.els.set('0:0',{querySelector:()=>({})});
  const reset = main.slice(main.indexOf('function clearRoamElements()'),main.indexOf('function glide('));
  const open = main.slice(main.indexOf('async function openProject('),main.indexOf('// ───────────────────────── 档案变了'));
  vm.runInContext(ts.transpile(reset+open,{target:ts.ScriptTarget.ES2022})+'\nglobalThis.openProject = openProject;',ctx);
  void ctx.openProject({id:'new',name:'New project'});
  assert.equal(ctx.state.current.id,'new');assert.equal(ctx.state.model,null);
  assert.equal(released,1);assert.equal(ctx.roam.els.size,0);assert.equal(ctx.roam.cols.size,0);
  finish({w:500,h:400,svg:'<svg/>',hits:[]});await work;
  assert.equal(previews,0,'the old project must not schedule its preview during the new project load');
});

test('preview keeps filters and outlines text without rasterizing on the shared worker', async () => {
  const source = fs.readFileSync(require.resolve('../src/render-worker.ts'), 'utf8');
  const snippet = source.slice(source.indexOf('async function preview('), source.indexOf('self.onmessage'));
  let constructed = 0, freed = 0;
  const outlined = '<svg><filter id="paper"/><path d="M0 0"/></svg>';
  const ctx = vm.createContext({ Blob, rasterReady: Promise.resolve(), themeById: () => ({}), bridgeStyle: () => ({}),
    fontBytes: async () => [], Resvg: class {
      constructor() { constructed++; }
      render() { throw new Error('Preview must not block artwork rendering with full-image rasterization'); }
      toString() { return outlined; }
      free() { freed++; }
    },
  });
  const flatten = fs.readFileSync(require.resolve('../../../packages/render/src/flatten.ts'), 'utf8').replace(/^export /gm, '');
  vm.runInContext(ts.transpile(flatten, {target:ts.ScriptTarget.ES2022}), ctx);
  vm.runInContext(ts.transpile(snippet, {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.preview = preview;', ctx);
  const paths = '<svg><path d="M0 0"/></svg>';
  assert.equal(await (await ctx.preview(paths,'minimal-light',true)).text(), paths);
  assert.equal(constructed,0);
  const blob = await ctx.preview('<svg><filter id="paper"/><text>creditia</text></svg>','minimal-light',true);
  assert.equal(blob.type,'image/svg+xml');
  assert.equal(await blob.text(),outlined);
  assert.equal(constructed,1); assert.equal(freed,1);
});

function canvasPreviews() {
  const source = fs.readFileSync(require.resolve('../src/roam-preview.ts'), 'utf8').replace(/^import type .*;\n/m, '').replaceAll('export function', 'function');
  let decodes = 0, releases = 0, draws = 0;
  const canvases = [], images = [];
  const ctx = vm.createContext({ console, Blob,
    URL: {createObjectURL: image => {images.push(image);return 'blob:preview';}, revokeObjectURL: () => releases++},
    Image: class { async decode() { decodes++; } },
    document: {createElement() {
      const canvas = {width:0,height:0,dataset:{},ops:[],setAttribute(){},getContext:()=>({
        drawImage(){ draws++; canvas.ops.push('draw'); }, clearRect(){ canvas.ops.push('clear'); },
      }),remove(){}};
      canvases.push(canvas); return canvas;
    }},
  });
  vm.runInContext(ts.transpile(source, {target:ts.ScriptTarget.ES2022}) + '\nglobalThis.mount = mountRoamPreview; globalThis.release = releaseRoamPreview; globalThis.previews = previews;', ctx);
  return {ctx,canvases,images,stats:()=>({decodes,releases,draws})};
}

test('visible roaming cards share decoded pixels and release them after the last card leaves', async () => {
  const {ctx,canvases,stats} = canvasPreviews(), svg = {}, art = {w:520,h:400,title:'card'};
  const first = {appendChild(){},querySelector(){}}, second = {appendChild(){},querySelector(){}};
  ctx.mount(first,art,svg,1040); ctx.mount(second,art,svg,1040);
  await settled();
  assert.deepEqual(stats(),{decodes:1,releases:1,draws:3});
  const source = canvases[2];
  ctx.release(first); await settled();
  assert.equal(source.width,1040);
  ctx.release(second); ctx.release(second); await settled();
  assert.equal(source.width,1); assert.equal(source.height,1);
  assert.equal(ctx.previews.size,0);
  ctx.mount(first,art,svg,520); await settled();
  assert.equal(stats().decodes,2);
  ctx.release(first);
});

test('a roaming card removed before its decode turn does not create an image or retain a canvas', async () => {
  const {ctx,canvases,stats} = canvasPreviews(), root={appendChild(){},querySelector(){}};
  ctx.mount(root,{w:100,h:100,title:'card'},{},520);
  ctx.release(root); await settled();
  assert.deepEqual(stats(),{decodes:0,releases:0,draws:0});
  assert.equal(canvases[0].width,1);
  assert.equal(ctx.previews.size,0);
});

test('resizing a roaming preview keeps existing pixels until the sharper image is ready', async () => {
  const {ctx,stats} = canvasPreviews(), svg = {}, art = {w:520,h:400,title:'card'};
  let current;
  const root = {appendChild: canvas => {current=canvas;},querySelector: () => current};
  ctx.mount(root,art,svg,760); await settled();
  const old = current, before = stats().draws;
  ctx.mount(root,art,svg,1040);
  assert.equal(current.dataset.ready,'true');
  assert.equal(current.width,1040); assert.equal(old.width,1);
  assert.equal(stats().draws,before+1, 'copy the old pixels before releasing them');
  await settled();
  assert.equal(stats().decodes,2); assert.equal(stats().draws,before+3);
  assert.deepEqual(current.ops,['draw','clear','draw'], 'replace the temporary pixels so transparent edges do not become darker');
  ctx.release(root); await settled();
  assert.equal(ctx.previews.size,0);
});

for (const rawArray of [false,true]) test(`the desktop decodes a shared native PNG (${rawArray ? 'postMessage' : 'binary IPC'})`, async () => {
  const {ctx,images,stats} = canvasPreviews(), calls = [];
  ctx.__TAURI__ = {core:{invoke:async (command,args) => {calls.push({command,...args});return rawArray ? [1,2,3] : new Uint8Array([1,2,3]).buffer;}}};
  const root = () => ({appendChild(){},querySelector(){}});
  const first=root(), second=root(), source=new Blob(['<svg/>'],{type:'image/svg+xml'});
  ctx.mount(first,{w:520,h:400,title:'card'},source,878);
  ctx.mount(second,{w:520,h:400,title:'card'},source,878);
  await settled();
  assert.deepEqual(calls,[{command:'render_preview',svg:'<svg/>',width:878}]);
  assert.equal(images[0].type,'image/png'); assert.equal(stats().decodes,1);
  assert.deepEqual([...new Uint8Array(await images[0].arrayBuffer())],[1,2,3]);
  ctx.release(first);ctx.release(second);await settled();
  assert.equal(ctx.previews.size,0);
});

test('leaving while SVG bytes are being read does not start a stale native render', async () => {
  const {ctx,images,stats} = canvasPreviews();
  let finish, calls = 0;
  ctx.__TAURI__ = {core:{invoke:async () => {calls++;return new ArrayBuffer(0);}}};
  const source = {text:() => new Promise(resolve => {finish=resolve;})};
  const root = {appendChild(){},querySelector(){}};
  ctx.mount(root,{w:520,h:400,title:'card'},source,878);
  await settled();ctx.release(root);finish('<svg/>');await settled();
  assert.equal(calls,0);assert.equal(images.length,0);assert.equal(stats().decodes,0);
  assert.equal(ctx.previews.size,0);
});

test('returning to a rendered card reuses its PNG without waiting behind a new render', async () => {
  const {ctx,stats} = canvasPreviews(), calls=[];
  let finish;
  ctx.__TAURI__={core:{invoke:async(_command,args)=>{
    calls.push(args.svg);
    if(args.svg==='slow')await new Promise(resolve=>{finish=resolve;});
    return [1,2,3];
  }}};
  const root=()=>({appendChild(){},querySelector(){}}), art={w:100,h:100,title:'card'};
  const first=root(), source=new Blob(['cached']);
  ctx.mount(first,art,source,200);await settled();ctx.release(first);await settled();
  const slow=root();ctx.mount(slow,art,new Blob(['slow']),200);await settled();
  const back=root();ctx.mount(back,art,source,100);await settled();
  assert.deepEqual(calls,['cached','slow']);
  assert.equal(stats().decodes,2,'the cached card is visible while an unrelated native render is still waiting');
  finish();await settled();ctx.release(slow);ctx.release(back);
});

test('queued native rendering prioritizes the current viewport and skips departed cards', async () => {
  const {ctx} = canvasPreviews(), calls=[];
  let finish;
  ctx.__TAURI__={core:{invoke:async(_command,args)=>{
    calls.push(args.svg);if(args.svg==='first')await new Promise(resolve=>{finish=resolve;});return [1,2,3];
  }}};
  const roots=Array.from({length:4},()=>({appendChild(){},querySelector(){}})), art={w:100,h:100,title:'card'};
  ctx.mount(roots[0],art,new Blob(['first']),200);await settled();
  ctx.mount(roots[1],art,new Blob(['overscan']),200,()=>700);
  ctx.mount(roots[2],art,new Blob(['visible']),200,()=>0);
  ctx.mount(roots[3],art,new Blob(['departed']),200,()=>0);
  ctx.release(roots[3]);finish();await settled();
  assert.deepEqual(calls,['first','visible','overscan']);
  roots.forEach(root=>ctx.release(root));
});

function freeViewport(changed = () => {}) {
  const source=fs.readFileSync(require.resolve('../src/roam-viewport.ts'),'utf8').replace('export class','class');
  const frames=new Map();let seq=0;
  const ctx=vm.createContext({performance:{now:()=>0},requestAnimationFrame:fn=>{const id=++seq;frames.set(id,now=>{frames.delete(id);fn(now);});return id;},cancelAnimationFrame:id=>frames.delete(id)});
  vm.runInContext(ts.transpile(source,{target:ts.ScriptTarget.ES2022})+'\nglobalThis.Viewport=RoamViewport;',ctx);
  const plane={style:{}},changes=[];
  const root={hidden:false,querySelector:()=>plane};
  return {viewport:new ctx.Viewport(root,(x,y)=>{changes.push([x,y]);changed(x,y);}),root,plane,changes,frames};
}

test('free movement updates both axes immediately even when animation frames are paused', () => {
  const {viewport,plane,changes,frames}=freeViewport();
  viewport.moveTo(-213,-96);viewport.moveTo(-133,54);
  assert.deepEqual(changes.at(-1),[-133,54]);
  assert.equal(plane.style.transform,'translate3d(133px, -54px, 0)');
  assert.equal(frames.size,0,'direct interaction must not wait for a throttled animation frame');
  viewport.moveTo(-4000000,8000000);
  assert.deepEqual([...viewport.position()],[-4000000,8000000]);
});

test('direct movement cancels a previous glide and cannot be moved by an old animation frame', () => {
  const {viewport,plane,frames}=freeViewport();
  viewport.moveTo(0,0);viewport.moveTo(600,-300,'smooth');
  assert.equal(frames.size,1);
  frames.values().next().value(260);
  assert.ok(viewport.position()[0]>0&&viewport.position()[1]<0);
  viewport.moveTo(-50,75);
  assert.equal(frames.size,0);
  assert.equal(plane.style.transform,'translate3d(50px, -75px, 0)');
});
