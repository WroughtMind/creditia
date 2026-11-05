// Run against a built dist directory. Playwright may come from the development
// runtime via NODE_PATH; no network requests are needed by the tested page.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, stat, mkdir, writeFile } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import { createHash } from 'node:crypto';
import { resolve, extname, sep } from 'node:path';
import { strict as assert } from 'node:assert';
const require = createRequire(import.meta.url);
const playwrightPath = require.resolve('playwright', { paths: [process.env.CREDITIA_PLAYWRIGHT_MODULES || '', process.env.CODEX_PRIMARY_RUNTIME_NODE_MODULES || '', process.cwd()] });
const { chromium } = require(playwrightPath);
const root = resolve(process.argv[2] || resolve(import.meta.dirname, '../dist'));
const output = resolve(process.argv[3] || resolve(root, '../browser-evidence'));
await mkdir(output, { recursive: true });
const document = '<!doctype html><html><head><meta charset="utf-8"><style>body{margin:40px;background:#eee}creditia-card{max-width:960px;margin:auto}</style></head><body><script type="module" src="/creditia-card.js"></script></body></html>';
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/') { res.setHeader('Content-Type', 'text/html'); res.end(document); return; }
    const path = resolve(root, '.' + new URL(req.url, 'http://localhost').pathname);
    if (!path.startsWith(root + sep) || !(await stat(path)).isFile()) { res.writeHead(404).end(); return; }
    res.setHeader('Content-Type', ({ '.js':'text/javascript', '.json':'application/json', '.wasm':'application/wasm', '.ttf':'font/ttf' })[extname(path)] || 'application/octet-stream');
    res.end(await readFile(path));
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 1360, height: 1000 }, deviceScaleFactor: 1 });
const errors = []; page.on('pageerror', error => errors.push(error.message));
const requests = []; page.on('request', request => requests.push(request.url()));
function normalize(svg) {
  let n = 0; const ids = new Map();
  for (const match of svg.matchAll(/\bid="([^"]+)"/g)) if (!ids.has(match[1])) ids.set(match[1], `id${n++}`);
  return svg.replace(/\bid="([^"]+)"/g, (_, id) => `id="${ids.get(id)}"`)
    .replace(/url\(#([^)]+)\)/g, (_, id) => `url(#${ids.get(id) ?? id})`)
    .replace(/(href|xlink:href)="#([^"]+)"/g, (_, attr, id) => `${attr}="#${ids.get(id) ?? id}"`);
}
const report = { fixtureKind: 'synthetic_render_sample', candidateSha:process.env.GITHUB_SHA || null,
  bundleSha256:createHash('sha256').update(await readFile(resolve(root,'creditia-card.js'))).digest('hex'),
  cases: [], screenshots: [], errors };
try {
  await page.goto(base); await page.waitForFunction(() => !!customElements.get('creditia-card'));
  const fixture = JSON.parse(await readFile(resolve(root, 'acceptance/cases.json'), 'utf8'));
  report.fixtureHash = fixture.fixtureHash;
  let source = null;
  try { source = JSON.parse(await readFile(resolve(root,'acceptance/source.json'),'utf8')); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (source) await page.evaluate(value => { window.creditiaAcceptanceSource = value; }, source);
  const catalogMeta = source ? JSON.parse(await readFile(resolve(root,'acceptance/catalog-meta.json'),'utf8')) : null;
  if (source) assert.equal(createHash('sha256').update(await readFile(resolve(root,'acceptance/source.json'))).digest('hex'),catalogMeta.sourceSha256);
  let catalogRows = 0;
  const catalogDigest = createHash('sha256');
  async function* cases() {
    yield* fixture.cases;
    if (source) for await (const line of createInterface({input:createReadStream(resolve(root,'acceptance/catalog.jsonl')),crlfDelay:Infinity})) {
      if (line) { catalogRows++; catalogDigest.update(line + '\n'); yield JSON.parse(line); }
    }
  }
  for await (const item of cases()) {
    const {art,painted} = await page.evaluate(({selection,context}) => new Promise((resolve, reject) => {
      document.querySelector('creditia-card')?.remove();
      const card = document.createElement('creditia-card');
      for (const [key, value] of Object.entries({ src:'/acceptance/fixture.json', theme:selection.style,
        renderer:selection.renderer, piece:selection.piece, lang:selection.lang, group:selection.group,
        named:String(selection.named), sign:selection.sign || '', variant:selection.variant || '',
        transparent:String(!!selection.transparent) })) card.setAttribute(key, value);
      if (context) card.data = window.creditiaAcceptanceSource;
      const timer = setTimeout(() => reject(new Error(`Timed out: ${selection.style}`)), 60000);
      card.addEventListener('creditia-ready', event => {
        clearTimeout(timer);
        const svg = card.querySelector('svg'), rect = svg?.getBoundingClientRect();
        resolve({art:event.detail.art,painted:rect ? {width:rect.width,height:rect.height,hits:card.querySelectorAll('.ct-hit').length} : null});
      }, { once:true });
      card.addEventListener('creditia-error', event => { clearTimeout(timer); reject(new Error(String(event.detail))); }, { once:true });
      document.body.appendChild(card);
    }), {selection:item.selection,context:item.context});
    if (!item.art) assert.equal(art,null,`Expected empty artwork: ${item.selection.id}`);
    else {
      assert(art,`Expected visible artwork: ${item.selection.id}`);
      assert.equal(art.w, item.art.w); assert.equal(art.h, item.art.h);
      // The native transport is JSON; an optional undefined key has the same
      // wire representation as an absent key. Compare that actual contract.
      assert.deepEqual(JSON.parse(JSON.stringify(art.hits)), item.art.hits,`Hover regions: ${item.selection.id}`);
      assert.equal(art.title, item.art.title); assert.equal(art.bg,item.art.bg);
      assert.deepEqual(art.crop,item.art.crop,`Crop: ${item.selection.id}`);
      assert.equal(normalize(art.svg), normalize(item.art.svg), `Native/web SVG mismatch: ${item.selection.id}`);
      assert(painted?.width > 0 && painted.height > 0,`Artwork did not occupy the browser layout: ${item.selection.id}`);
      assert(Math.abs(painted.height - painted.width * art.h / art.w) < 0.1,`Browser aspect ratio: ${item.selection.id}`);
      assert.equal(painted.hits,art.hits.length,`Mounted hover regions: ${item.selection.id}`);
    }
    report.cases.push({ id: item.selection.id, status: 'passed', rendered:!!art, catalog:!!item.context,
      row:{renderer:item.selection.renderer,style:item.selection.style,piece:item.selection.piece,variant:item.selection.variant || 'default'},
      checks:['canonical-svg','dimensions','title','background','crop','hover-data','actual-dom-bounds','mounted-hover-regions'].map(name=>({name,passed:true})) });
    if (item.selection.lang === 'zh' && !item.context) {
      const file = `${item.selection.id}.png`;
      await page.locator('creditia-card').screenshot({ path: resolve(output, file) }); report.screenshots.push(file);
    }
    if (report.cases.length % 100 === 0) console.log(`Browser catalog: ${report.cases.length} passed`);
  }
  if (catalogMeta) {
    assert.equal(catalogRows,catalogMeta.rows,'Incomplete browser catalog');
    assert.equal(catalogDigest.digest('hex'),catalogMeta.sha256,'Catalog fixture bytes changed');
    report.catalog = {...catalogMeta,checkedRows:catalogRows};
  }
  // Exercise the public animation controls and the OS motion preference in the
  // actual custom element; timer mocks would miss its visibility/RAF lifecycle.
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  const motion = await page.evaluate(() => new Promise((resolve, reject) => {
    document.querySelector('creditia-card')?.remove();
    const card = document.createElement('creditia-card');
    Object.entries({ src:'/acceptance/fixture.json', theme:'ink', renderer:'svg', piece:'asterisk', animated:'' })
      .forEach(([key,value]) => card.setAttribute(key,value));
    const timer = setTimeout(() => reject(new Error('Animated web artwork timed out')),60000);
    card.addEventListener('creditia-animation-ready', event => { clearTimeout(timer); resolve(event.detail); }, { once:true });
    card.addEventListener('creditia-animation-error', event => { clearTimeout(timer); reject(new Error(String(event.detail))); }, { once:true });
    document.body.appendChild(card);
  }));
  assert(motion.frameCount > 1 && motion.timeline.durationMs > 0);
  const svg = () => page.locator('creditia-card .ct-art').innerHTML();
  await page.evaluate(() => document.querySelector('creditia-card').pause());
  const paused = await svg(); await page.waitForTimeout(150); assert.equal(await svg(),paused);
  await page.evaluate(() => document.querySelector('creditia-card').replay());
  await page.waitForTimeout(400); const playing = await svg();
  await page.waitForTimeout(400); assert.notEqual(await svg(),playing,'Replay must advance the artwork');
  await page.emulateMedia({ reducedMotion:'reduce' });
  await page.waitForTimeout(80); const reduced = await svg();
  await page.waitForTimeout(200); assert.equal(await svg(),reduced,'Reduced motion must show a stable final frame');
  assert.equal(await page.locator('creditia-card .ct-play').isVisible(),false);
  await page.emulateMedia({ reducedMotion:'no-preference' });
  await page.waitForTimeout(400); assert.notEqual(await svg(),reduced,'Animation must recover when reduced motion is disabled');
  await page.emulateMedia({ reducedMotion:'reduce' });
  const initialReduced = await page.evaluate(() => new Promise((resolve,reject) => {
    const card = document.querySelector('creditia-card');
    const timeout = setTimeout(() => reject(new Error('Reduced initial playback timed out')),60000);
    card.addEventListener('creditia-animation-ready', event => { clearTimeout(timeout); resolve(event.detail); },{once:true});
    // Consecutive attribute changes must discard intermediate paint results.
    card.setAttribute('theme','minimal-light'); card.setAttribute('renderer','classic');
  }));
  assert.equal(initialReduced.frameCount,1); assert.equal(initialReduced.timeline.durationMs,0);
  await page.evaluate(() => {
    const card = document.querySelector('creditia-card');
    window.creditiaResumed = new Promise((resolve,reject) => {
      const timeout=setTimeout(() => reject(new Error('Playback did not resume after reduced motion')),60000);
      card.addEventListener('creditia-animation-ready',event => { clearTimeout(timeout); resolve(event.detail); },{once:true});
    });
  });
  await page.emulateMedia({ reducedMotion:'no-preference' });
  const resumed = await page.evaluate(() => window.creditiaResumed);
  assert(resumed.frameCount > 1 && resumed.timeline.durationMs > 0);
  report.motion = { controls:'passed', reducedMotion:'passed', frameCount:motion.frameCount, timeline:motion.timeline };
  assert.deepEqual(errors, []); assert(requests.every(url => url.startsWith(base)), 'Web renderer requested an external asset');
  report.status = 'passed';
} catch (error) { report.status = 'failed'; report.failure = error.stack; throw error; }
finally {
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  await browser.close(); await new Promise(resolve => server.close(resolve));
}
console.log(`${report.cases.length} browser/native parity cases passed.`);
