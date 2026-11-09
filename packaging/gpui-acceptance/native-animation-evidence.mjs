#!/usr/bin/env node
// Seal the actual installed-window streaming driver; no fixture-only render can
// substitute for its OS input, decoded-frame state and captured pixels.
import fs from 'node:fs';
import path from 'node:path';
import { args, artifact, readArtifact } from './common.mjs';
const options = args();
if (!options.input || !options.output || !options['candidate-sha']) throw Error('input, output and candidate-sha are required');
const input = path.resolve(options.input), source = path.dirname(input), output = path.resolve(options.output);
const report = JSON.parse(fs.readFileSync(input));
if (report.schema_version !== 1 || report.kind !== 'installed-streaming-animation-dpi2' ||
    report.candidate_sha !== options['candidate-sha'] || !/^[a-f0-9]{40}$/.test(report.candidate_sha)) throw Error('Unexpected native animation report or candidate');
for (const field of ['binary_sha256', 'archive_sha256', 'driver_sha256']) {
  if (!/^[a-f0-9]{64}$/.test(report[field])) throw Error(`Native animation report lacks ${field}`);
}
const checks = new Map((report.checks ?? []).map(check => [check.id, check]));
const actions = new Map((report.actions ?? []).map(action => [action.action, action]));
const requiredChecks = ['wall-dpi2', 'paused-first', 'paused-later', 'progressive-painted-frame-change',
  'paused-deep-zoom', 'paused-pan', 'hidden-paused-resource-sample', 'restored-paused', 'hidden-playing-resource-sample', 'normal-quit'];
const requiredActions = ['focus', 'play', 'pause', 'resume', 'pause-later', 'paused-deep-zoom', 'paused-pan',
  'hide-paused', 'restore-paused', 'replay', 'hide-playing', 'restore-playing', 'close-player'];
const passed = report.complete === true && !report.error && requiredChecks.every(id => checks.get(id)?.passed === true) &&
  requiredActions.every(id => actions.get(id)?.after) && checks.get('normal-quit')?.exit_code === 0;
const artifacts = (report.artifacts ?? []).map(record => artifact(output, record.path, readArtifact(source, record)));
artifacts.push(artifact(output, 'animation-driver-report.json', fs.readFileSync(input)));
const semantic = (feature_id, check, behavior) => ({ feature_id, check, behavior, test_id: 'installed-streaming-animation-dpi2', passed });
const feature_checks = [
  semantic('new-reduced-motion', '连续操作、中断、重播', 'Uses real OS Play/Pause/Resume/Replay/Close actions, verifies different live frames paint different artwork pixels, preserves a paused playhead during pan/zoom, and rejects stale epochs.'),
  semantic('new-reduced-motion', '不可见动画暂停', 'Minimizes both paused and playing states, observes the native suspended clock remain unchanged, restores playback intent correctly and captures the restored frame.'),
  semantic('new-zoom', 'GPUI 画布连续缩放', 'Repeated real native zoom commands increase the focused viewport by more than 2x while the installed production renderer supplies a correctly decoded current frame.'),
  semantic('new-zoom', '缓存按缩放档和像素密度失效', 'Native DPI2 and paused deep-zoom/pan update the epoch and current physical resolution; actual decoded chunks remain at most 8 MiB and resident frames at most 64 MiB.'),
  { feature_id: 'new-performance', check: '空闲 CPU 与不可见动画',
    test_id: 'performance-idle-cpu + native-hidden-animation', status: passed ? 'not_run' : 'failed',
    behavior: 'The real installed player suspends its hidden clock in paused and playing states; accepting the combined checklist item also requires separately measured idle CPU.',
    tests: [{ test_id: 'performance-idle-cpu', outcomes: [] },
      { test_id: 'native-hidden-animation', outcomes: [{ status: passed ? 'passed' : 'failed', source: 'installed animation hidden-state assertions' }] }] },
];
const manifest = { schema_version: 1, kind: 'native-ui', complete: report.complete === true,
  provenance: { candidate_sha: report.candidate_sha, environment: { platform: 'linux', arch: 'x86_64', renderer: 'installed X11 software graphics', dpi_scale: 2 },
    fixture: { sha256: report.archive_sha256, kind: 'synthetic_render_sample' }, binary_sha256: report.binary_sha256, driver_sha256: report.driver_sha256 },
  cases: [{ key: 'installed-streaming-animation-dpi2', status: passed ? 'passed' : 'failed',
    checks: requiredChecks.map(id => ({ ...checks.get(id), id, passed: checks.get(id)?.passed === true })),
    feature_checks, artifacts, actions: report.actions, error: report.error ?? null }],
  scope: { tested_artwork: 'minimal-light/share', actual_installed_binary: true, operating_system_input: true,
    limitation: 'One real streaming path at native DPI2. Cadence uses decoded frame-selection timestamps before GPU presentation; OS screenshots independently verify sampled painted output. This does not establish all 26 styles, other operating systems, physical-GPU metrics or a visual sharpness review.' } };
fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify({ output, passed }));
if (!passed) process.exitCode = 1;
