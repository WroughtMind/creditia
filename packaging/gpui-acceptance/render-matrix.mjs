#!/usr/bin/env node
// Compare a pinned original checkout with the candidate, without conflating a
// shared-renderer comparison with native-window or human visual acceptance.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { args, artifact, caseKey, cleanCheckout, commitOf, countBy, environment, fileHash,
  normalizedArt, parseMatrix, pixelDiff, positiveInteger, safeName, sha256, stable } from './common.mjs';
import { contexts, fixtureCases, preferences } from './fixtures.mjs';
import { exportChecks } from './export-checks.mjs';
import { decodePng } from './export-validation.mjs';

const options = args();
if (options.help) {
  console.log('render-matrix.mjs --baseline-root DIR --candidate-root DIR --output DIR [--profile smoke|full] [--fixture FILE] [--fixture-kind synthetic_render_sample|real_public_git_archive] [--preferences all|rotate] [--stress] [--shards N --shard INDEX] [--width 320] [--motion] [--exports] [--save-all-svg]');
  process.exit(0);
}
if (!options['baseline-root'] || !options['candidate-root'] || !options.output) throw Error('baseline-root, candidate-root and output are required');
const baselineRoot = path.resolve(options['baseline-root']), candidateRoot = path.resolve(options['candidate-root']);
const output = path.resolve(options.output);
if (baselineRoot === candidateRoot) throw Error('Baseline and candidate must be separate pinned checkouts');
const profile = options.profile ?? 'smoke';
if (!['smoke', 'full'].includes(profile)) throw Error('profile must be smoke or full');
const width = positiveInteger(options.width, 'width', 320);
if (width > 2048) throw Error('Comparison width exceeds 2048px budget');
const shards = positiveInteger(options.shards, 'shards', 1);
const shard = options.shard === undefined ? 0 : Number(options.shard);
if (!Number.isSafeInteger(shard) || shard < 0 || shard >= shards) throw Error('shard must be zero based and smaller than shards');
const inventoryPath = path.join(candidateRoot, 'docs/gpui-migration/inventory.json');
const inventory = JSON.parse(fs.readFileSync(inventoryPath));
const baselineSha = commitOf(baselineRoot), candidateSha = commitOf(candidateRoot);
if (baselineSha !== inventory.baselineCommit) throw Error(`Baseline is ${baselineSha}, expected ${inventory.baselineCommit}`);
if (!cleanCheckout(baselineRoot) || !cleanCheckout(candidateRoot)) throw Error('Tracked sources must be clean; commit the candidate before producing acceptance evidence');
const fixturePath = path.resolve(options.fixture ?? path.join(candidateRoot, 'packages/render/fixtures/catnap.json'));
const fixtureKind = options['fixture-kind'] ?? 'synthetic_render_sample';
if (!['synthetic_render_sample', 'real_public_git_archive'].includes(fixtureKind)) throw Error('Unrecognized fixture provenance kind');
const fixtures = fixtureCases(fixturePath, fixtureKind).filter((_, i) => i === 0 || options.stress);
const rows = parseMatrix(fs.readFileSync(path.join(candidateRoot, 'docs/gpui-migration/matrix.tsv'), 'utf8'));
if (rows.length !== inventory.counts.stylePieceLayoutRows) throw Error('Matrix and inventory disagree');
let selected = profile === 'full' ? rows : rows.filter(r =>
  (r.default_variant === 'true' && ['asterisk', 'roll', 'codemap', 'tier'].includes(r.piece)) ||
  (r.piece === 'roll' && r.variant === 'scroll') ||
  (['ink', 'minimal-light'].includes(r.style) && ['profile', 'blame'].includes(r.piece)));
selected = selected.filter((_, i) => i % shards === shard);
fs.mkdirSync(output, { recursive: true });
const importAt = (root, file) => import(pathToFileURL(path.join(root, file)).href);
async function backend(root, protocol) {
  const [svg, classic, exports] = await Promise.all([
    importAt(root, 'packages/render/src/index.ts'), importAt(root, 'packages/render/src/classic/index.ts'), importAt(root, 'packages/render/src/export.ts'),
  ]);
  const native = protocol ? await importAt(root, 'packages/render/src/native-protocol.ts') : null;
  const require = createRequire(path.join(root, 'packages/render/package.json'));
  const raster = value => value.includes('<text') ? svg.raster : classic.raster;
  const fontRoot = path.dirname(require.resolve('@expo-google-fonts/noto-sans-sc/package.json'));
  const files = [];
  function walk(dir) { for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(p);
    else if (/\.(ttf|otf)$/.test(entry.name)) files.push({ path: path.relative(path.dirname(fontRoot), p), sha256: fileHash(p) });
  } }
  walk(path.dirname(fontRoot));
  files.sort((a, b) => a.path.localeCompare(b.path));
  return { svg, classic, exports, native, raster, fonts: files, fontHash: sha256(stable(files)),
    resvgVersion: require('@resvg/resvg-js/package.json').version };
}
const baseline = await backend(baselineRoot, false), candidate = await backend(candidateRoot, true);
if (baseline.fontHash !== candidate.fontHash) throw Error('Baseline and candidate font bytes differ; review and pin fonts before comparing artwork');
const fontManifest = artifact(output, 'font-manifest.json', JSON.stringify(candidate.fonts, null, 2) + '\n');
const manifest = {
  schema_version: 1, kind: 'render-matrix', started_at: new Date().toISOString(), profile,
  provenance: { baseline_sha: baselineSha, candidate_sha: candidateSha, environment: environment(),
    fixture: { path: path.relative(candidateRoot, fixturePath), sha256: fileHash(fixturePath), kind: fixtureKind },
    fonts: fontManifest, resvg: { baseline: baseline.resvgVersion, candidate: candidate.resvgVersion } },
  scope: { catalog_rows: rows.length, desktop_rows: rows.filter(r => r.desktop === 'true').length,
    selected_rows: selected.length, shard, shards, preferences: options.preferences === 'all' ? 'all' : 'rotated',
    fixtures: fixtures.map(({ id, provenance }) => ({ id, ...provenance })), width,
    native_ui_tested: false, human_visual_reviewed: false },
  cases: [],
};
function variantOf(row) { return row.variant === 'default' ? undefined : row.variant; }
function requestFor(row, raw, pref) {
  const context = contexts(raw);
  return { version: 1, id: caseKey(row), op: 'render', renderer: row.renderer, style: row.style,
    piece: row.piece, variant: variantOf(row), raw, lang: pref.lang, group: pref.group, named: pref.named,
    ...(row.piece === 'profile' ? { projects: context.projects, person: context.person } : {}),
    ...(row.piece === 'blame' ? { blame: context.blame } : {}) };
}
async function direct(api, row, raw, pref) {
  const { lang, group, named } = pref, variant = variantOf(row), context = contexts(raw);
  if (row.renderer === 'classic') {
    const model = api.classic.buildModel(raw, { director: false, lang, group, named });
    return api.classic.art(row.piece, model, api.classic.themeById(row.style), { lang, variant,
      projects: context.projects.map(r => api.classic.buildModel(r, { director: false, lang, group, named })), person: context.person, blame: context.blame });
  }
  return api.svg.art(row.piece, api.svg.buildModel(raw, { lang, group, named, sign: '' }), api.svg.STYLES.find(s => s.id === row.style), { variant });
}
function unwrap(response) {
  if (!response.ok) throw Error(`${response.error.code}: ${response.error.message}`);
  return response.result;
}
function compareArt(a, b) {
  if (!a || !b) return { data_equal: a === b, svg_equal: a === b, pixels: null, empty: a === null && b === null };
  const { svg: asvg, crop: acrop } = normalizedArt(a), { svg: bsvg, crop: bcrop } = normalizedArt(b);
  // Selection/timeline metadata is new API information, not a visual change.
  // Geometry, visible data and every hover region remain exact requirements.
  const visibleData = art => ({ w: art.w, h: art.h, bg: art.bg, title: art.title, hits: art.hits });
  const adata = visibleData(a), bdata = visibleData(b);
  const pixelWidth = Math.max(1, Math.floor(Math.min(width, 4096 * a.w / a.h, 4096 * b.w / b.h)));
  // Compare actual PNG pixels across revisions. The old raw resvg buffer uses
  // premultiplied alpha; comparing adapter memory conventions would falsely
  // label a corrected export contract as a visible artwork regression.
  const ar = decodePng(baseline.raster(a.svg).png(a.svg, pixelWidth)), br = decodePng(candidate.raster(b.svg).png(b.svg, pixelWidth));
  return { data_equal: stable(adata) === stable(bdata), svg_equal: asvg === bsvg, crop_equal: stable(acrop) === stable(bcrop),
    pixels: pixelDiff(ar, br), empty: false, raster_width: pixelWidth,
    baseline_rgba_sha256: sha256(ar.data), candidate_rgba_sha256: sha256(br.data) };
}
async function motionChecks(request, row) {
  if (row.animated !== 'true') return { status: 'not_applicable' };
  const result = unwrap(await candidate.native.renderRequest({ ...request, op: 'frames', frameCount: 4 }));
  if (!Array.isArray(result.frames) || !Array.isArray(result.times) || result.frames.length !== result.times.length) throw Error('Frames response lacks explicit aligned sample times');
  const checks = [];
  for (let i = 0; i < result.frames.length; i++) {
    const sampled = unwrap(await candidate.native.renderRequest({ ...request, op: 'render', t: result.times[i] })).art;
    checks.push(stable(normalizedArt(sampled)) === stable(normalizedArt(result.frames[i])));
  }
  const reduced = unwrap(await candidate.native.renderRequest({ ...request, op: 'frames', frameCount: 4, reducedMotion: true }));
  const final = unwrap(await candidate.native.renderRequest({ ...request, op: 'render', t: 1 })).art;
  return { status: checks.every(Boolean) && reduced.frames?.length === 1 && stable(normalizedArt(reduced.frames[0])) === stable(normalizedArt(final)) ? 'matched' : 'different',
    deterministic_samples: checks, times: result.times, timeline: result.timeline,
    reduced_motion_final_frame: reduced.frames?.length === 1 && stable(normalizedArt(reduced.frames[0])) === stable(normalizedArt(final)),
    limitation: 'Checks shared sampling only; native playback, interruption and style motion direction require separate UI and visual evidence.' };
}

let ordinal = 0, diagnosticFailures = 0;
for (const [rowIndex, row] of selected.entries()) {
  const prefs = options.preferences === 'all' ? preferences : [preferences[(rowIndex * shards + shard) % preferences.length]];
  for (const fixture of fixtures) for (const pref of prefs) {
    const key = `${caseKey(row)}/${fixture.id}/${pref.id}`;
    const record = { key, row: caseKey(row), desktop: row.desktop === 'true', fixture: fixture.id,
      options: pref, context_kind: ['profile', 'blame'].includes(row.piece) ? 'synthetic_presentation_context' : null,
      started_at: new Date().toISOString(), artifacts: [] };
    const start = performance.now();
    try {
      const a = await direct(baseline, row, fixture.raw, pref);
      const request = requestFor(row, fixture.raw, pref);
      const b = unwrap(await candidate.native.renderRequest(request)).art;
      record.comparison = compareArt(a, b);
      record.status = record.comparison.empty ? 'empty' : record.comparison.data_equal && record.comparison.svg_equal && record.comparison.pixels?.equal ? 'matched' : 'different';
      if (a && b) {
        const name = safeName(key), w = record.comparison.raster_width;
        const apng = baseline.raster(a.svg).png(a.svg, w), bpng = candidate.raster(b.svg).png(b.svg, w);
        const first = artifact(output, `images/${name}-baseline.png`, apng);
        const second = sha256(apng) === sha256(bpng) ? first : artifact(output, `images/${name}-candidate.png`, bpng);
        record.baseline = { svg_sha256: sha256(a.svg), normalized_svg_sha256: sha256(normalizedArt(a).svg), image: first };
        record.candidate = { svg_sha256: sha256(b.svg), normalized_svg_sha256: sha256(normalizedArt(b).svg), image: second };
        record.artifacts.push(first, ...(first === second ? [] : [second]));
        if (record.status !== 'matched' || options['save-all-svg']) {
          record.artifacts.push(artifact(output, `svg/${name}-baseline.svg`, a.svg), artifact(output, `svg/${name}-candidate.svg`, b.svg));
        }
        const representative = fixture.id === fixtures[0].id && pref.id === prefs[(rowIndex * shards + shard) % prefs.length].id;
        if (options.motion && representative) record.motion = await motionChecks(request, row);
        if (options.exports && representative) record.exports = await exportChecks(candidate, request, row, rowIndex * shards + shard, (filename, bytes) => {
          const file = artifact(output, `exports/${name}-${filename}`, bytes); record.artifacts.push(file); return file;
        });
        if (record.motion?.status === 'different' || record.exports?.status === 'different') record.status = 'different';
      }
    } catch (error) { record.status = 'error'; record.error = error instanceof Error ? error.stack : String(error); }
    record.duration_ms = Math.round((performance.now() - start) * 100) / 100;
    manifest.cases.push(record);
    if (['different', 'error'].includes(record.status) && diagnosticFailures++ < 20) console.warn(JSON.stringify({
      case: record.key, status: record.status, data_equal: record.comparison?.data_equal,
      svg_equal: record.comparison?.svg_equal, pixels_equal: record.comparison?.pixels?.equal,
      motion: record.motion?.status, export_failures: record.exports?.formats.filter(format => !format.passed).map(format => format.format),
      error: record.error?.slice(0, 500),
    }));
    // Checkpoint after every case; an interrupted CI run still has honest partial evidence.
    manifest.summary = countBy(manifest.cases, c => c.status);
    fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
    if (++ordinal % 20 === 0) console.log(JSON.stringify({ completed: ordinal, selected_rows: selected.length, summary: manifest.summary }));
  }
}
manifest.completed_at = new Date().toISOString();
manifest.complete = true;
manifest.summary = countBy(manifest.cases, c => c.status);
fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
const esc = value => String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
fs.writeFileSync(path.join(output, 'index.html'), `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Creditia artwork comparison</title><style>body{font:14px system-ui;margin:24px;background:#eee;color:#222}header{position:sticky;top:0;background:#eee;padding:12px 0}input{font:inherit;width:28em;max-width:80%;padding:8px}section{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:18px}article{background:white;padding:12px;min-width:0}h2{font:12px monospace;overflow-wrap:anywhere}figure{margin:0;display:flex;gap:6px}img{width:calc(50% - 3px);object-fit:contain;align-self:flex-start}.different,.error{outline:2px solid #bd3b32}p{overflow-wrap:anywhere}</style><header><h1>Creditia artwork comparison</h1><p>${esc(baselineSha)} → ${esc(candidateSha)} · ${esc(JSON.stringify(manifest.summary))}</p><p>Shared-renderer evidence. Native UI and human visual acceptance remain separate.</p><input aria-label="Filter cases" placeholder="Filter by style, piece, language or status" oninput="for(const e of document.querySelectorAll('article'))e.hidden=!e.dataset.search.includes(this.value.toLowerCase())"></header><section>${manifest.cases.map(c => `<article class="${esc(c.status)}" data-search="${esc((c.key + ' ' + c.status).toLowerCase())}"><h2>${esc(c.key)}</h2><p>${esc(c.status)}</p>${c.baseline ? `<figure><img loading="lazy" alt="Baseline" src="${esc(c.baseline.image.path)}"><img loading="lazy" alt="Candidate" src="${esc(c.candidate.image.path)}"></figure>` : `<p>${esc(c.error ?? 'Both renderers produced no artwork for this input.')}</p>`}</article>`).join('')}</section></html>`);
console.log(JSON.stringify({ output, complete: true, summary: manifest.summary }));
if (manifest.summary.error || manifest.summary.different) process.exitCode = 1;
