import test from 'node:test';
import assert from 'node:assert/strict';
import { testOutcomes } from './feature-evidence.mjs';
test('behavior evidence reads actual Rust outcomes, retaining failed and ignored tests', () => {
  const results = testOutcomes('\uFEFF2026-11-06T00:01:02.1234567Z test service::recovery ... \u001b[32mok\u001b[0m\r\n' +
    '2026-11-06T00:01:02Z test ui::mouse ... FAILED\n' +
    'test manual::helper ... ignored\n' +
    '2026-11-06T00:01:02Z assertion text: test fake::pass ... ok\n' +
    'running 3 tests\ntest result: FAILED. 1 passed; 1 failed; 1 ignored\n');
  assert.deepEqual(results, [
    { test_id: 'service::recovery', status: 'passed', line: 1 },
    { test_id: 'ui::mouse', status: 'failed', line: 2 },
    { test_id: 'manual::helper', status: 'not_run', line: 3 },
  ]);
});

test('Node spec and TAP results preserve exact titles and never count skipped tests', () => {
  assert.deepEqual(testOutcomes('2026-11-06T00:01:02Z ✔ 中文字体 (12.23ms)\n✖ render frames (1.2s)\nok 3 - an optional case # SKIP unavailable\nnot ok 4 - transparent output\n'), [
    {test_id:'中文字体',status:'passed',line:1},
    {test_id:'render frames',status:'failed',line:2},
    {test_id:'an optional case',status:'not_run',line:3},
    {test_id:'transparent output',status:'failed',line:4},
  ]);
});
