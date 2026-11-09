import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const execute = promisify(execFile);
import { createHash } from 'node:crypto';
import { Resvg } from '@resvg/resvg-js';
import { FONTS, familiesIn } from '../../src/fonts.ts';
import { art, frames, buildModel, PIECES, STYLES, raster, fontFiles } from '../../src/index.ts';
import { makeExporter } from '../../src/export.ts';
import { scrollAtlas, createScrollCompositor } from '../../src/scroll-compositor.ts';
import * as classic from '../../src/classic/index.ts';
import { variantsOf } from '../../src/classic/art.ts';
import { BRIDGED, isBridged } from '../../src/classic/bridge.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const args = process.argv.slice(2);
const argument = name => { const i = args.indexOf(name); return i < 0 ? undefined : args[i + 1]; };
const [shard, shards] = (argument('--shard') ?? '1/1').split('/').map(Number);
if (!Number.isInteger(shard) || !Number.isInteger(shards) || shard < 1 || shard > shards) throw Error('invalid --shard i/n');
const suffix = shards === 1 ? '' : `-${shard}-of-${shards}`;
const dir = path.resolve(argument('--output') ?? root + '/output/playwright/material-matrix');
mkdirSync(dir, { recursive: true });
const raw = JSON.parse(readFileSync(root + '/packages/render/fixtures/catnap.json', 'utf8'));
const modelOptions = { lang: 'zh', sign: '', named: true };
const m = buildModel(raw, modelOptions), cm = classic.buildModel(raw, { lang: 'zh', director: false, named: true });
const allFiles = fontFiles();
const subsetDir = argument('--font-directory');
const manifest = subsetDir ? JSON.parse(readFileSync(root + '/apps/desktop/src/fonts/manifest.json', 'utf8')) : [];
const hash = data => createHash('sha256').update(data).digest('hex');
const norm = svg => svg.replace(/\bc[0-9a-z]+-/g, 'scene-').replace(/\bs[0-9a-z]+_/g, 'satori_');
const filesCache = new Map();
function filesFor(svg) {
  if (!/<text\b/.test(svg)) return [];
  const names = new Set([...svg.matchAll(/font-family="([^"]+)"/g)].flatMap(m => familiesIn(m[1].replace(/&quot;/g, '"').replace(/&apos;/g, "'"))));
  names.add('Noto Sans SC');
  const key = [...names].sort().join('|');
  if (!filesCache.has(key)) {
    const needed = FONTS.filter(f => names.has(f.family));
    const files = subsetDir ? needed.map(f => {
      const face = manifest.find(x => x.family === f.family && x.weight === f.weight && !!x.italic === !!f.italic);
      if (!face || !existsSync(path.join(subsetDir, face.file))) throw Error(`missing subset ${f.family}/${f.weight}`);
      return path.join(subsetDir, face.file);
    }) : allFiles.filter(p => needed.some(f => p.includes('/' + f.pkg + '/' + f.dir + '/')));
    filesCache.set(key, files);
  }
  return filesCache.get(key);
}
const renderSvg = (svg, width = 120) => new Resvg(svg, { font: { fontFiles: filesFor(svg), loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' }, fitTo: { mode: 'width', value: width }, shapeRendering: 2, textRendering: 1, imageRendering: 0 }).render();
const pngOf = svg => renderSvg(svg).asPng();
let scrollRasterCalls = 0, scrollRasterSource;
const cropExporter = makeExporter({ png: (svg, width) => renderSvg(svg, width).asPng(), rgba: (svg, width) => {
  const image = renderSvg(svg, width), pixels = { w: image.width, h: image.height, data: new Uint8Array(image.pixels) };
  scrollRasterCalls++; scrollRasterSource = { svg, width, pixels }; return pixels;
}, flat: svg => svg });
const modern = STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id));
const cases = modern.flatMap(st => Object.entries(PIECES).flatMap(([id, p]) => (p.variants ?? ['']).map(variant => ({ kind: 'modern', st, id, variant }))));
for (const st of classic.THEMES) for (const id of [...Object.keys(classic.FORMS), ...BRIDGED, 'badge']) {
  const vs = variantsOf(id);
  for (const variant of vs.length ? vs : ['']) cases.push({ kind: isBridged(id, variant) ? 'bridge' : 'native', st, id, variant });
}
const counts = Object.fromEntries(['modern', 'bridge', 'native'].map(k => [k, cases.filter(c => c.kind === k).length]));
if (cases.length !== 1523 || counts.modern !== 473 || counts.bridge !== 435 || counts.native !== 615) throw Error(`catalog changed: ${JSON.stringify(counts)}`);
const context = variant => ({ lang: 'zh', variant: variant || undefined, projects: [cm], person: cm.director, blame: { file: 'example.ts', line: 1, name: '演示记录', commit: 'demo', subject: '溯源版式演示', evidence: 'unattributed' } });
const git = async (...a) => (await execute('git', a, { cwd: root, encoding: 'utf8' })).stdout.trim();
const baselineDir = argument('--baseline');
let baseline;
if (baselineDir) {
  const original = await import(pathToFileURL(path.resolve(baselineDir, 'packages/render/src/index.ts')));
  const originalClassic = await import(pathToFileURL(path.resolve(baselineDir, 'packages/render/src/classic/index.ts')));
  baseline = { original, originalClassic, m: original.buildModel(raw, modelOptions), cm: originalClassic.buildModel(raw, { lang: 'zh', director: false, named: true }), commit: (await execute('git', ['rev-parse', 'HEAD'], { cwd: baselineDir, encoding: 'utf8' })).stdout.trim() };
}
// Preserve the original strict subset assertion and extend it to every product style.
const fontChecks = [];
for (const st of modern.concat(classic.THEMES.map(t => ({ ...t, classicTheme: true })))) {
  const candidate = st.classicTheme ? await classic.art('tier', cm, st, context('rows')) : art('tier', m, st, { t: .35 });
  const png = pngOf(candidate.svg), reference = Buffer.from(raster.png(candidate.svg, 120));
  if (!png.equals(reference)) { writeFileSync(dir + '/font-subset-' + st.id + '.png', png); writeFileSync(dir + '/font-original-' + st.id + '.png', reference); throw Error('font subset differs: ' + st.id); }
  fontChecks.push({ style: st.id, sha256: hash(png) });
}
const selected = cases.filter((c, i) => i % shards === shard - 1 && (!argument('--filter') || [c.st.id, c.id, c.variant].join('/').includes(argument('--filter'))));
const started = new Date().toISOString(), checks = [], failures = [];
const metadata = { commit: await git('rev-parse', 'HEAD'), sourceDiffSha256: hash(await git('diff', 'HEAD', '--', 'packages/render/src')), baselineCommit: baseline?.commit ?? null, fixtureSha256: hash(JSON.stringify(raw)), sampleTimes: [0, 1 / 3, 2 / 3, 1], rasterWidth: 120, fonts: subsetDir ? 'validated preview subsets' : 'original fonts selected by SVG family', catalog: counts, combinations: cases.length, selected: selected.length, shard, shards, started, fontChecks };
function save(final = false) { writeFileSync(`${dir}/${final ? 'results' : 'partial'}${suffix}.json`, JSON.stringify({ ...metadata, finished: final ? new Date().toISOString() : null, checked: checks.length, frames: checks.reduce((n, c) => n + c.frames.length, 0), failures, checks }, null, 2)); }
for (const [i, c] of selected.entries()) {
  const { kind, st, id, variant } = c, key = [st.id, id, variant || 'default'].join('/');
  writeFileSync(`${dir}/current${suffix}.txt`, key);
  try {
    const ctx = context(variant), opts = { variant: variant || undefined };
    const still = kind === 'modern' ? art(id, m, st, opts) : await classic.art(id, cm, st, ctx);
    if (!still) throw Error('missing static artwork');
    const all = kind === 'modern' ? frames(id, m, st, opts, 3) : await classic.frames(id, cm, st, ctx, 3);
    const elapsed = [0]; for (let j = 1; j < all.length; j++) elapsed.push(elapsed[j - 1] + all[j - 1].frameDelay);
    const indices = metadata.sampleTimes.map(t => elapsed.reduce((best, e, j) => Math.abs(e - t * elapsed.at(-1)) < Math.abs(elapsed[best] - t * elapsed.at(-1)) ? j : best, 0));
    const aa = indices.map(j => all[j]);
    if (aa.some(a => !a)) throw Error('missing animation sample');
    if (new Set(aa.map(a => `${a.w}/${a.h}`)).size !== 1) throw Error('unstable animation dimensions');
    if (still.motion?.kind === 'scroll') {
      if (new Set(all.map(a => a.crop?.svg)).size !== 1) throw Error('scroll re-renders its reel');
      if (new Set(all.map(a => a.crop?.y)).size < 2) throw Error('scroll has no movement');
      if (all[0].crop.y !== 0 || all.at(-1).crop.y !== Math.round(Math.max(0, still.h - still.motion.viewH))) throw Error('scroll misses reel endpoints');
      if (norm(all[0].crop.svg) !== norm(still.svg)) throw Error('scroll changes static reel');
    } else {
      if (aa.at(-1).w !== still.w || aa.at(-1).h !== still.h) throw Error('final dimensions differ from static');
      if (norm(aa.at(-1).svg) !== norm(still.svg)) throw Error('final SVG differs from static');
    }
    const output = path.join(dir, 'frames', st.id, id, variant || 'default'); mkdirSync(output, { recursive: true });
    let scrollEvidence, scrollFinalPixels;
    if (still.motion?.kind === 'scroll') {
      // Exercise the public GIF exporter: rasterize the reel once, then crop RGBA.
      // Re-rasterizing the offscreen SVG filters is both the wrong product path and a resvg panic.
      scrollRasterCalls = 0; scrollRasterSource = undefined;
      const gif = cropExporter.gif(aa, { width: 120, hold: 100 });
      writeFileSync(path.join(output, 'samples.gif'), gif);
      if (scrollRasterCalls !== 1) throw Error(`scroll GIF rasterized ${scrollRasterCalls} times instead of once`);
      const material = still.motion.material;
      await execute('python3', ['-c', 'from PIL import Image; import sys; from pathlib import Path; p=Path(sys.argv[1]); im=Image.open(p/"samples.gif"); [(im.seek(i),im.convert("RGBA").save(p/(sys.argv[2]+str(i)+".png"))) for i in range(im.n_frames)]', output, material ? 'gif-' : '']);
      scrollEvidence = { rasterCalls: scrollRasterCalls, gifSha256: hash(gif), materialStyle: material?.style ?? null, atlasPlanes: material ? 3 : 1 };
      if (material) {
        // frames() creates its own Scene IDs. Verify the exact reel handed to GIF,
        // after the separate normalized comparison with static art above.
        const reel = aa[0].crop;
        if (!reel.motion) throw Error('material scroll crop lacks its shared motion');
        const atlas = scrollAtlas(reel.svg, reel.w, reel.h, reel.motion);
        if (scrollRasterSource.svg !== atlas.svg || scrollRasterSource.width !== 360) throw Error('GIF did not rasterize the declared shared material atlas');
        const compositor = createScrollCompositor(scrollRasterSource.pixels, atlas, reel.motion);
        for (const [index, frame] of aa.entries()) {
          if (!Number.isFinite(frame.crop?.progress)) throw Error('material scroll frame lacks shared progress');
          if (Math.abs(frame.crop.progress - elapsed[indices[index]] / elapsed.at(-1)) > 1e-8) throw Error('material scroll progress differs from the shared frame timing');
          const img = compositor.render(frame.crop.progress, frame.crop.y);
          writeFileSync(path.join(output, `${index}.rgba`), img.data);
          if (index === aa.length - 1) {
            scrollFinalPixels = { w: img.w, h: img.h, data: Buffer.from(img.data) };
            const source = scrollRasterSource.pixels, scale = img.w / reel.w;
            const top = Math.max(0, Math.min(Math.round(frame.crop.y * scale), source.h - img.h));
            for (let y = 0; y < img.h; y++) {
              const originalPlaneRow = Buffer.from(source.data.buffer, source.data.byteOffset + ((top + y) * source.w) * 4, img.w * 4);
              if (!scrollFinalPixels.data.subarray(y * img.w * 4, (y + 1) * img.w * 4).equals(originalPlaneRow)) throw Error('material scroll final differs from atlas original plane');
            }
            scrollEvidence.finalViewportMatchesAtlasOriginalPlane = true;
          }
        }
        await execute('python3', ['-c', 'from PIL import Image; import sys; from pathlib import Path; p=Path(sys.argv[1]); size=(int(sys.argv[2]),int(sys.argv[3])); [(Image.frombytes("RGBA",size,(p/(str(i)+".rgba")).read_bytes()).save(p/(str(i)+".png")),(p/(str(i)+".rgba")).unlink()) for i in range(4)]', output, String(compositor.width), String(compositor.height)]);
        Object.assign(scrollEvidence, { sharedCompositorSamples: true, materialLayers: atlas.layers.length, frameBytes: compositor.frameBytes, sourceBytes: compositor.sourceBytes, progressSamples: aa.map(frame => frame.crop.progress) });
      }
    }
    const evidence = aa.map((a, index) => {
      if (/NaN|Infinity/.test(a.svg)) throw Error('non-finite SVG');
      if (a.hits.some(h => !Number.isFinite(h.x + h.y + h.w + h.h) || h.w < 0 || h.h < 0)) throw Error('invalid hover');
      writeFileSync(`${dir}/current-stage${suffix}.txt`, key + '/sample-' + index); writeFileSync(`${dir}/current-svg${suffix}.svg`, a.svg);
      const png = still.motion?.kind === 'scroll' ? readFileSync(path.join(output, index + '.png')) : pngOf(a.svg); if (!png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) || !png.readUInt32BE(16) || !png.readUInt32BE(20)) throw Error('invalid PNG');
      const filename = `${index}.png`; writeFileSync(path.join(output, filename), png);
      return { t: elapsed[indices[index]] / elapsed.at(-1), svgSha256: hash(norm(a.svg)), pngSha256: hash(png), image: path.relative(dir, path.join(output, filename)), cropY: a.crop?.y };
    });
    if (new Set(evidence.map(a => a.pngSha256)).size < 2) throw Error('no rasterized motion');
    let baselineHash, baselineMatch;
    if (baseline) {
      const oldCtx = { ...ctx, projects: [baseline.cm], person: baseline.cm.director };
      const oldStyle = kind === 'modern' ? baseline.original.STYLES.find(t => t.id === st.id) : baseline.originalClassic.THEMES.find(t => t.id === st.id);
      const old = kind === 'modern' ? baseline.original.art(id, baseline.m, oldStyle, opts) : await baseline.originalClassic.art(id, baseline.cm, oldStyle, oldCtx);
      if (!old || old.w !== still.w || old.h !== still.h) throw Error('baseline dimensions differ');
      if (scrollFinalPixels) {
        // 独立静态基线校验；不计入上面公共 GIF 路径只栅格化一次的断言。
        const reference = renderSvg(old.svg), scale = reference.width / old.w;
        const top = Math.max(0, Math.min(Math.round(aa.at(-1).crop.y * scale), reference.height - scrollFinalPixels.h));
        const crop = Buffer.from(reference.pixels).subarray(top * reference.width * 4, (top + scrollFinalPixels.h) * reference.width * 4);
        if (reference.width !== scrollFinalPixels.w || crop.length !== scrollFinalPixels.data.length) throw Error('material scroll independent crop dimensions differ');
        // 原 viewport 隔离必需：防止越界笔触污染透明材质平面。独立 Resvg 上下文
        // 的斜线/字边抗锯齿差异逐像素记录，不用容差吞掉，也不冒称完全一致。
        const pixels = [], bounds = [Infinity, Infinity, -1, -1]; let changedChannels = 0, maxChannelDifference = 0;
        for (let i = 0; i < crop.length; i += 4) {
          const before = [...crop.subarray(i, i + 4)], after = [...scrollFinalPixels.data.subarray(i, i + 4)];
          if (before.some((v, k) => v !== after[k])) {
            const x = i / 4 % reference.width, y = Math.floor(i / 4 / reference.width);
            pixels.push({ x, y, baselineRGBA: before, compositorRGBA: after });
            bounds[0] = Math.min(bounds[0], x); bounds[1] = Math.min(bounds[1], y); bounds[2] = Math.max(bounds[2], x); bounds[3] = Math.max(bounds[3], y);
            before.forEach((v, k) => { if (v !== after[k]) { changedChannels++; maxChannelDifference = Math.max(maxChannelDifference, Math.abs(v - after[k])); } });
          }
        }
        const delta = { exactMatch: pixels.length === 0, changedPixels: pixels.length, changedChannels, maxChannelDifference, bounds: pixels.length ? bounds : null, pixels };
        writeFileSync(path.join(output, 'independent-static-crop-delta.json'), JSON.stringify(delta, null, 2));
        scrollEvidence.finalViewportMatchesBaselinePixels = delta.exactMatch;
        scrollEvidence.independentStaticRasterDelta = { ...delta, pixels: undefined, evidence: path.relative(dir, path.join(output, 'independent-static-crop-delta.json')), interpretation: 'separate raster context AA; original SVG geometry and atlas original-plane equality checked independently' };
        if (norm(old.svg) !== norm(still.svg)) throw Error('material scroll original static SVG differs from baseline');
      }
      baselineHash = hash(norm(old.svg));
      baselineMatch = 'normalized-svg-identical';
      if (norm(old.svg) !== norm(still.svg)) {
        writeFileSync(`${dir}/current-stage${suffix}.txt`, key + '/baseline'); writeFileSync(`${dir}/current-svg${suffix}.svg`, old.svg);
        const oldPng = pngOf(old.svg), finalPng = pngOf(still.svg);
        if (!oldPng.equals(finalPng)) {
          writeFileSync(path.join(output, 'baseline.png'), oldPng); writeFileSync(path.join(output, 'static.png'), finalPng);
          throw Error(`static pixels differ from baseline ${baseline.commit}`);
        }
        // 不消去真差异：即使仅修 CSS 字体引号，也先完成上面的实际像素比较。
        const oldFontSpelling = svg => norm(svg).replace(/font-family="([^"]*)"/g, (whole, stack) => whole.replace(stack, stack.replace(/&quot;Press Start 2P&quot;/g, 'Press Start 2P')));
        baselineMatch = oldFontSpelling(old.svg) === oldFontSpelling(still.svg)
          ? 'pixel-identical-font-family-quoting-only' : 'pixel-identical-at-raster-width';
      }
    }
    checks.push({ key, kind, w: still.w, h: still.h, states: new Set(evidence.map(a => a.pngSha256)).size, baselineSvgSha256: baselineHash, baselineMatch, scrollEvidence, frames: evidence });
  } catch (error) { failures.push({ key, kind, error: error.stack ?? String(error) }); console.log('FAIL', key, error.message); }
  if (i % 10 === 0) { console.log(`${i + 1}/${selected.length} ${key}`); save(); }
}
save(true);
console.log(JSON.stringify({ selected: selected.length, checked: checks.length, failures: failures.map(({ key, error }) => ({ key, error: error.split('\n')[0] })) }));
process.exitCode = failures.length ? 1 : 0;
