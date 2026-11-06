import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import { artifact } from './common.mjs';

const candidate = 'a'.repeat(40);
const feature = { feature_id: 'new-zoom', check: '缓存按缩放档和像素密度失效',
  behavior: 'Actual assertion checks density and cache epoch.', test_id: 'resolution-test' };
function write(dir, name, value) {
  const location = path.join(dir, name);
  fs.mkdirSync(location, { recursive: true });
  const proof = artifact(location, 'test.log', 'explicit test proof');
  fs.writeFileSync(path.join(location, 'manifest.json'), JSON.stringify({ schema_version: 1,
    complete: true, provenance: { candidate_sha: candidate }, artifacts: [proof], ...value }));
  return proof;
}
function report(dir) {
  const output = path.join(dir, 'report');
  const result = spawnSync(process.execPath, [path.join(import.meta.dirname, 'report.mjs'),
    '--evidence', dir, '--candidate-sha', candidate, '--output', output], { encoding: 'utf8' });
  assert.ok([0, 1].includes(result.status), result.stderr);
  return JSON.parse(fs.readFileSync(path.join(output, 'report.json')));
}
const verdict = result => result.feature_coverage.find(f => f.id === feature.feature_id)
  .checks.find(c => c.name === feature.check);

test('feature proof combines executed suites and preserves actual failures', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'creditia-feature-report-'));
  try {
    for (const [name, passed] of [['unit', 'density'], ['native', 'epoch']]) {
      write(dir, name, { kind: 'feature-checks', cases: [{ ...feature, key: name, status: 'not_run',
        tests: ['density', 'epoch'].map(test_id => ({ test_id,
          outcomes: test_id === passed ? [{ status: 'passed', line: 1 }] : [] })) }] });
    }
    assert.equal(verdict(report(dir)).status, 'passed');
    write(dir, 'failed', { kind: 'feature-checks', cases: [{ ...feature, key: 'failed', status: 'failed',
      tests: [{ test_id: 'epoch', outcomes: [{ status: 'failed', line: 1 }] }] }] });
    assert.equal(verdict(report(dir)).status, 'failed');
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});

test('an unsuccessful native case cannot supply passed semantic claims', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'creditia-failed-claim-'));
  try {
    write(dir, 'native', { kind: 'native-ui', cases: [{ key: 'interrupted-input', status: 'failed',
      feature_checks: [{ ...feature, passed: true }] }] });
    const result = report(dir);
    assert.equal(verdict(result).status, 'not_run');
    assert.match(result.invalid_evidence[0].error, /unsuccessful case/);
    assert.equal(result.recorded_review.executable_feature_checks, 0);
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
});
