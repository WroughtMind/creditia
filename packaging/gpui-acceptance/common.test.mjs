import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import { artifact, normalizedArt, parseMatrix, pixelDiff, readArtifact, sha256 } from './common.mjs';
import { contexts, fixtureCases } from './fixtures.mjs';

const root = path.resolve(import.meta.dirname, '../..');
test('acceptance matrix keeps every shared combination distinct from desktop scope', () => {
  const rows = parseMatrix(fs.readFileSync(path.join(root, 'docs/gpui-migration/matrix.tsv'), 'utf8'));
  assert.equal(rows.length, 1738);
  assert.equal(rows.filter(r => r.desktop === 'true').length, 1482);
  assert.equal(new Set(rows.filter(r => r.desktop === 'true').map(r => r.style)).size, 26);
  assert.throws(() => parseMatrix('style\trenderer\tpiece\tvariant\na\tx\tp\tv\na\tx\tp\tv\n'), /Duplicate/);
});
test('pixel comparison measures all RGBA channels and dimensions without a tolerance', () => {
  const a = { w: 1, h: 1, data: Uint8Array.of(10, 20, 30, 0) };
  assert.equal(pixelDiff(a, a).equal, true);
  const b = { ...a, data: Uint8Array.of(10, 20, 30, 1) };
  assert.deepEqual(pixelDiff(a, b), { equal: false, dimension_mismatch: false, changed_pixels: 1, total_pixels: 1,
    changed_fraction: 1, mean_channel_error: .25, maximum_channel_error: 1 });
  assert.equal(pixelDiff(a, { ...a, w: 2 }).dimension_mismatch, true);
});
test('normalization changes generated namespaces but keeps geometry and hit evidence', () => {
  const a = { svg: '<svg><path id="c1-star" d="M1 2"/><use href="#c1-star"/></svg>', hits: [{ x: 1, t: 'evidence' }] };
  const b = { ...a, svg: a.svg.replaceAll('c1-', 'c9-') };
  assert.deepEqual(normalizedArt(a), normalizedArt(b));
  assert.notDeepEqual(normalizedArt(a), normalizedArt({ ...b, svg: b.svg.replace('M1 2', 'M2 2') }));
  assert.equal(normalizedArt(a).hits[0].t, 'evidence');
});
test('artifact verification rejects edited bytes and escaped paths', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'creditia-acceptance-'));
  try {
    const item = artifact(dir, 'images/a.png', Buffer.from('recorded bytes'));
    assert.equal(readArtifact(dir, item).toString(), 'recorded bytes');
    fs.writeFileSync(path.join(dir, item.path), 'edited');
    assert.throws(() => readArtifact(dir, item), /checksum/);
    assert.throws(() => artifact(dir, '../outside', 'bad'), /escaped/);
    assert.throws(() => readArtifact(dir, { path: '../outside', sha256: sha256('bad') }), /escaped/);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
test('synthetic stress and blame contexts remain explicitly separate from source provenance', () => {
  const fixtures = fixtureCases(path.join(root, 'packages/render/fixtures/catnap.json'));
  assert.equal(fixtures[0].provenance.kind, 'synthetic_render_sample');
  assert.equal(fixtures[1].provenance.kind, 'synthetic_stress_derived');
  assert.notEqual(fixtures[0].provenance.sha256, fixtures[1].provenance.sha256);
  assert.equal(contexts(fixtures[0].raw).blame.evidence, 'declared');
  assert.match(contexts(fixtures[0].raw).blame.subject, /Synthetic/);
});
test('an empty evidence directory never produces a completed release gate', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'creditia-report-'));
  try {
    const result = spawnSync(process.execPath, [path.join(import.meta.dirname, 'report.mjs'), '--evidence', dir,
      '--candidate-sha', 'a'.repeat(40), '--output', path.join(dir, 'report'), '--require-complete'], { encoding: 'utf8' });
    assert.equal(result.status, 1, result.stderr);
    const report = JSON.parse(fs.readFileSync(path.join(dir, 'report/report.json')));
    assert.equal(report.release_ready, false);
    assert.equal(report.artwork_summary.not_run, 1738);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
