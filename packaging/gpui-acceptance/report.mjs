#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { args, countBy, parseMatrix, readArtifact, stable } from './common.mjs';
import { preferences } from './fixtures.mjs';

const options = args();
if (options.help) {
  console.log('report.mjs --evidence DIR --candidate-sha SHA --output DIR [--matrix FILE] [--require-complete]');
  process.exit(0);
}
if (!options.evidence || !options['candidate-sha'] || !options.output) throw Error('evidence, candidate-sha and output are required');
if (!/^[a-f0-9]{40}$/.test(options['candidate-sha'])) throw Error('candidate-sha must be the full commit ID');
const root = path.resolve(options.evidence), output = path.resolve(options.output);
const files = [];
function walk(dir) { for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
  if (entry.isSymbolicLink()) continue;
  const file = path.join(dir, entry.name);
  if (entry.isDirectory()) walk(file);
  else if (entry.name === 'manifest.json') files.push(file);
} }
walk(root);
const matrix = parseMatrix(fs.readFileSync(options.matrix ?? path.resolve(import.meta.dirname, '../../docs/gpui-migration/matrix.tsv'), 'utf8'));
const features = JSON.parse(fs.readFileSync(path.resolve(import.meta.dirname, '../../docs/gpui-migration/features.json'), 'utf8'));
const expected = new Map(matrix.map(row => [[row.renderer, row.style, row.piece, row.variant].join('/'), row]));
const failures = [], manifests = [], rows = new Map(), native = [], performance = [], packages = [];
function checkArtifacts(value, dir) {
  if (!value || typeof value !== 'object') return;
  if (typeof value.path === 'string' && typeof value.sha256 === 'string') readArtifact(dir, value);
  else for (const item of Object.values(value)) if (item && typeof item === 'object') checkArtifacts(item, dir);
}
for (const file of files) {
  const relative = path.relative(root, file).split(path.sep).join('/');
  try {
    const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (manifest.schema_version !== 1) throw Error('Unsupported evidence schema');
    if (manifest.provenance?.candidate_sha !== options['candidate-sha']) throw Error('Evidence belongs to a different candidate commit');
    if (manifest.complete !== true) throw Error('Evidence run did not complete');
    if (['render-matrix', 'native-ui'].includes(manifest.kind) && (!Array.isArray(manifest.cases) || !manifest.cases.length)) throw Error('Rendering/UI evidence has no cases');
    // Only declared artifacts are rooted at the manifest; fixture input paths
    // can refer to the checkout and must not be mistaken for output artifacts.
    checkArtifacts(manifest.artifacts, path.dirname(file));
    for (const c of manifest.cases ?? []) checkArtifacts(c.artifacts, path.dirname(file));
    if (manifest.provenance.fonts) readArtifact(path.dirname(file), manifest.provenance.fonts);
    if (manifest.kind === 'render-matrix') {
      for (const c of manifest.cases) {
        if (!expected.has(c.row)) throw Error(`Unknown artwork matrix identity ${c.row}`);
        if (!['matched', 'different', 'empty', 'error'].includes(c.status)) throw Error(`Unknown render status ${c.status}`);
        if (c.status === 'matched' && !(c.comparison?.data_equal && c.comparison.svg_equal && c.comparison.pixels?.equal)) throw Error(`Matched case lacks successful data, SVG and pixel comparisons: ${c.key}`);
        if (c.status === 'matched' && !c.artifacts?.length) throw Error(`Matched artwork lacks an image artifact: ${c.key}`);
        if (c.status === 'empty' && c.comparison?.empty !== true) throw Error(`Empty case lacks paired empty outputs: ${c.key}`);
      }
      for (const c of manifest.cases) {
        if (!rows.has(c.row)) rows.set(c.row, []);
        rows.get(c.row).push({ status: c.status, key: c.key, manifest: relative, fixture_kind: manifest.scope.fixtures.find(f => f.id === c.fixture)?.kind,
          preference: c.options?.id, motion: c.motion?.status ?? 'not_run', exports: c.exports?.status ?? 'not_run' });
      }
    } else if (manifest.kind === 'native-ui') {
      for (const c of manifest.cases) {
        if (!['passed', 'failed', 'not_run'].includes(c.status)) throw Error(`Unknown native status ${c.status}`);
        if (c.status === 'passed' && (!c.artifacts?.length || !c.checks?.length || c.checks.some(check => check.passed !== true))) throw Error(`Passed native case lacks checked behavior or artifacts: ${c.key}`);
      }
      native.push({ manifest: relative, platform: manifest.provenance.environment?.platform, cases: manifest.cases });
    } else if (manifest.kind === 'performance') performance.push({ manifest: relative, ...manifest });
    else if (manifest.kind === 'package') {
      if (manifest.acceptance?.passed === true && !manifest.artifacts?.length) throw Error('Passed package lacks installer/startup evidence');
      packages.push({ manifest: relative, ...manifest });
    }
    else throw Error(`Unknown evidence kind ${manifest.kind}`);
    manifests.push({ path: relative, kind: manifest.kind, cases: manifest.cases?.length ?? 0 });
  } catch (error) { failures.push({ manifest: relative, error: error.message }); }
}
const artwork = [...expected].map(([key, row]) => {
  const evidence = rows.get(key) ?? [];
  const preferencesSeen = [...new Set(evidence.filter(e => e.status === 'matched').map(e => e.preference))];
  return { key, desktop: row.desktop === 'true', status: !evidence.length ? 'not_run' : evidence.some(e => ['error', 'different'].includes(e.status)) ? 'needs_review' : evidence.every(e => e.status === 'empty') ? 'empty_only' : 'machine_matched',
    preference_coverage: preferencesSeen, all_preferences: preferences.every(p => preferencesSeen.includes(p.id)),
    motion_sampled: row.animated !== 'true' || evidence.some(e => e.motion === 'matched'),
    exports_checked: evidence.some(e => e.exports === 'matched'), evidence };
});
const nativeFailures = native.flatMap(m => m.cases.filter(c => c.status !== 'passed').map(c => ({ manifest: m.manifest, key: c.key, status: c.status })));
const performanceComparisons = [];
const comparisonScenarios = ['startup', 'scroll', 'roam', 'zoom', 'idle', 'hidden-idle'];
for (const current of performance.filter(p => p.comparison_role === 'candidate' && comparisonScenarios.includes(p.scenario))) {
  const key = p => stable({ environment: p.provenance.environment, fixture: p.provenance.fixture.sha256,
    workload: p.provenance.workload, scenario: p.scenario, cache: p.cache_state });
  const before = performance.find(p => p.comparison_role === 'baseline' && key(p) === key(current));
  performanceComparisons.push(before && before.workload_verified && current.workload_verified ? {
    scenario: current.scenario, cache_state: current.cache_state, status: 'comparable', baseline: before.manifest, candidate: current.manifest,
    rss_peak_bytes: [before.summary.rss_peak_bytes, current.summary.rss_peak_bytes],
    cpu_p50_percent_one_core: [before.summary.cpu_p50_percent_one_core, current.summary.cpu_p50_percent_one_core],
  } : { scenario: current.scenario, cache_state: current.cache_state, status: before ? 'missing_workload_evidence' : 'missing_identical_baseline', candidate: current.manifest });
}
const manual = options['manual-review'] ? JSON.parse(fs.readFileSync(options['manual-review'], 'utf8')) : null;
if (manual && manual.candidate_sha !== options['candidate-sha']) failures.push({ manifest: options['manual-review'], error: 'Manual review candidate differs' });
const evidenceCases = new Set(manifests.flatMap(m => {
  const data = JSON.parse(fs.readFileSync(path.join(root, m.path), 'utf8'));
  return (data.cases ?? []).map(c => `${m.path}#${c.key}`);
}));
const manualEntries = new Map((manual?.artworks ?? []).map(e => [e.key, e]));
const featureEntries = new Map((manual?.features ?? []).map(e => [e.id, e]));
const hasEvidence = e => Array.isArray(e?.evidence) && e.evidence.length > 0 && e.evidence.every(ref => evidenceCases.has(ref));
const reviewedArtwork = [...expected].every(([key, row]) => {
  const review = manualEntries.get(key);
  return review?.visual === 'accepted' && hasEvidence(review) && (row.animated !== 'true' || review.motion === 'accepted');
});
const reviewedFeatures = features.every(feature => {
  const review = featureEntries.get(feature.id);
  return review?.status === 'accepted' && hasEvidence(review) && Array.isArray(review.checks) &&
    feature.checks.every(check => review.checks.some(c => c.name === check && c.status === 'accepted'));
});
const manualComplete = manual?.candidate_sha === options['candidate-sha'] && typeof manual.reviewer === 'string' && manual.reviewer.trim().length > 0 &&
  !Number.isNaN(Date.parse(manual.reviewed_at)) && reviewedArtwork && reviewedFeatures;
const realVisualInput = artwork.some(row => row.evidence.some(e => e.fixture_kind === 'real_public_git_archive' && e.status === 'matched'));
const releaseReady = failures.length === 0 && artwork.every(r => r.status === 'machine_matched' && r.all_preferences && r.motion_sampled && r.exports_checked) && native.length > 0 && nativeFailures.length === 0 &&
  comparisonScenarios.every(scenario => ['cold', 'warm'].every(cache => performanceComparisons.some(p => p.scenario === scenario && p.cache_state === cache && p.status === 'comparable'))) && performanceComparisons.every(p => p.status === 'comparable') &&
  ['darwin-arm64', 'darwin-x64', 'win32-x64', 'linux-x64'].every(platform => packages.some(p => p.platform === platform && p.acceptance?.passed === true)) &&
  manualComplete && realVisualInput;
const report = { schema_version: 1, candidate_sha: options['candidate-sha'], generated_at: new Date().toISOString(),
  release_ready: releaseReady, evidence_manifests: manifests, invalid_evidence: failures,
  real_project_visual_sample: realVisualInput,
  artwork_summary: countBy(artwork, r => r.status), artwork, native, native_failures: nativeFailures,
  performance_comparisons: performanceComparisons, additional_performance: performance.filter(p => !comparisonScenarios.includes(p.scenario)).map(p => ({ manifest: p.manifest, scenario: p.scenario, verified: p.workload_verified })), package_platforms: packages.map(p => ({ platform: p.platform, acceptance: p.acceptance, manifest: p.manifest })),
  manual_review: { status: manual ? manualComplete ? 'complete' : 'incomplete' : 'not_provided', reviewed_artworks: manualEntries.size,
    reviewed_features: featureEntries.size, source: options['manual-review'] ?? null },
  limitations: ['Machine rendering matches do not verify native UI placement or interaction.',
    'Empty outputs require a data-rich fixture before a combination can be accepted.',
    'Human visual/motion review and every feature check remain independent acceptance gates.',
    'Performance is compared only for matching machine, data, workload, scenario and cache labels.'] };
fs.mkdirSync(output, { recursive: true });
fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
const md = [`# Creditia GPUI acceptance`, '', `Candidate: \`${report.candidate_sha}\``, '',
  `Release gate: **${releaseReady ? 'ready' : 'not ready'}**`, '',
  '## Artwork coverage', '', '| Status | Combinations |', '|---|---:|',
  ...Object.entries(report.artwork_summary).map(([k, v]) => `| ${k} | ${v} |`), '',
  `Desktop scope: ${artwork.filter(r => r.desktop).length}; shared catalog total: ${artwork.length}.`, '',
  '## Evidence', '', '| Kind | Manifest | Cases |', '|---|---|---:|',
  ...manifests.map(m => `| ${m.kind} | ${m.path} | ${m.cases} |`), '',
  '## Remaining gates', '',
  `- Invalid or incomplete evidence: ${failures.length}.`, `- Native UI cases not passed: ${nativeFailures.length}.`,
  `- Matched baseline/candidate performance scenarios: ${performanceComparisons.filter(p => p.status === 'comparable').length}.`,
  `- Human review: ${manual ? 'provided; see report.json for candidate and gate checks' : 'not provided'}.`, '',
  ...report.limitations.map(l => `- ${l}`), '',
  ...(failures.length ? ['## Evidence errors', '', ...failures.map(f => `- ${f.manifest}: ${f.error}`), ''] : []),
].join('\n');
fs.writeFileSync(path.join(output, 'REPORT.md'), md);
console.log(JSON.stringify({ release_ready: releaseReady, artwork_summary: report.artwork_summary, invalid_evidence: failures.length, output }));
if (failures.length || options['require-complete'] && !releaseReady) process.exitCode = 1;
