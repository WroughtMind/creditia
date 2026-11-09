#!/usr/bin/env node
// Convert the real browser driver's report to the common evidence contract.
// Never promotes local/partial reports to a commit-tagged acceptance result.
import fs from 'node:fs';
import path from 'node:path';
import { args, artifact, caseKey, environment, fileHash, parseMatrix } from './common.mjs';
const options = args();
if (!options['browser-report'] || !options['catalog-root'] || !options['candidate-sha'] || !options.output) throw Error('browser-report, catalog-root, candidate-sha and output are required');
const input = path.resolve(options['browser-report']), catalogRoot = path.resolve(options['catalog-root']), output = path.resolve(options.output);
const report = JSON.parse(fs.readFileSync(input, 'utf8'));
if (!/^[a-f0-9]{40}$/.test(options['candidate-sha']) || report.candidateSha !== options['candidate-sha']) throw Error('Browser report must belong to the supplied immutable candidate SHA');
if (report.status !== 'passed' || report.errors?.length || report.failure) throw Error('Browser run failed or did not complete');
const metadata = JSON.parse(fs.readFileSync(path.join(catalogRoot, 'catalog-meta.json')));
for (const [filename, expected] of [['catalog.jsonl', metadata.sha256], ['source.json', metadata.sourceSha256]]) {
  if (!expected || fileHash(path.join(catalogRoot, filename)) !== expected) throw Error(`Browser fixture changed after generation: ${filename}`);
}
const matrix = parseMatrix(fs.readFileSync(path.resolve(import.meta.dirname, '../../docs/gpui-migration/matrix.tsv'), 'utf8'));
const expected = new Set(matrix.map(caseKey));
const cases = report.cases.filter(c => c.catalog).map(c => ({ key: caseKey(c.row), row: caseKey(c.row), status: c.status,
  rendered: c.rendered, checks: c.checks }));
if (cases.length !== expected.size || new Set(cases.map(c => c.row)).size !== expected.size ||
    cases.some(c => !expected.has(c.row) || c.status !== 'passed' || !c.rendered || !c.checks?.length || c.checks.some(check => check.passed !== true))) throw Error('Browser catalog lacks exhaustive rendered and checked artwork coverage');
if (report.catalog?.checkedRows !== expected.size || report.catalog.sha256 !== metadata.sha256 || report.catalog.sourceSha256 !== metadata.sourceSha256) throw Error('Browser report and catalog provenance differ');
const bundle = path.resolve(catalogRoot, '../creditia-card.js');
if (fileHash(bundle) !== report.bundleSha256) throw Error('Browser bundle changed after the run');
fs.mkdirSync(output, { recursive: true });
const artifacts = [];
for (const [source, relative] of [[input, 'browser-report.json'], [bundle, 'creditia-card.js'],
  [path.join(catalogRoot, 'catalog.jsonl'), 'expected-catalog.jsonl'], [path.join(catalogRoot, 'catalog-meta.json'), 'catalog-meta.json'],
  [path.join(catalogRoot, 'source.json'), 'source.json']]) artifacts.push(artifact(output, relative, fs.readFileSync(source)));
for (const filename of report.screenshots ?? []) {
  if (path.basename(filename) !== filename) throw Error('Screenshot path must be a basename');
  artifacts.push(artifact(output, `screenshots/${filename}`, fs.readFileSync(path.join(path.dirname(input), filename))));
}
const manifest = { schema_version: 1, kind: 'browser-catalog', complete: true,
  provenance: { candidate_sha: options['candidate-sha'], environment: environment(),
    fixture: { sha256: report.fixtureHash, kind: report.fixtureKind }, source_sha256: metadata.sourceSha256,
    catalog_sha256: metadata.sha256, bundle_sha256: report.bundleSha256 }, cases, artifacts,
  scope: { catalog_rows: cases.length, additional_browser_cases: report.cases.filter(c => !c.catalog).length,
    actual_component: true, representative_screenshots: report.screenshots?.length ?? 0,
    limitation: 'All catalog rows checked in the production browser component; screenshots are representative, not a claim of manual visual review of every row.' } };
fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify({ output, cases: cases.length, artifacts: artifacts.length }));
