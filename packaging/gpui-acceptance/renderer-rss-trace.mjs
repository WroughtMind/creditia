#!/usr/bin/env node
// Real JSONL preview traffic, matching native post-operation RSS retirement.
// This measures renderer subprocesses only; use resources.py for whole-app RSS.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import { args, fileHash, positiveInteger } from './common.mjs';

const options = args();
if (!options.fixture || !options.output) throw Error('Use --fixture ARCHIVE.json --output DIRECTORY [--threshold-mib 768]');
const fixture = path.resolve(options.fixture), output = path.resolve(options.output);
const bridge = path.resolve(options.bridge ?? path.join(import.meta.dirname, '../../packages/render/tools/bridge.mjs'));
const threshold = positiveInteger(options['threshold-mib'], 'threshold-mib', 768) * 1024 * 1024;
const raw = JSON.parse(fs.readFileSync(fixture, 'utf8'));
fs.mkdirSync(output, { recursive: true });
const log = fs.openSync(path.join(output, 'responses.jsonl'), 'w');
const errors = fs.openSync(path.join(output, 'stderr.log'), 'w');
let sequence = 0, generation = 0, worker, completed = 0, empty = 0, peak = 0, retired = 0;
const styleSummaries = [], failures = [], began = Date.now();
const write = value => fs.writeSync(log, JSON.stringify({ at: new Date().toISOString(), ...value }) + '\n');

function start() {
  const child = spawn(process.execPath, [bridge], { stdio: ['pipe', 'pipe', errors], windowsHide: true });
  const instance = { child, generation: ++generation, pending: null, closed: false };
  const lines = createInterface({ input: child.stdout });
  lines.on('line', line => {
    const pending = instance.pending; instance.pending = null;
    if (!pending) return;
    clearTimeout(pending.timer);
    try {
      const response = JSON.parse(line);
      if (response.id !== pending.id || response.version !== 1) throw Error('Renderer response identity mismatch');
      pending.resolve(response);
    } catch (error) { pending.reject(error); }
  });
  const failed = error => {
    if (instance.pending) { clearTimeout(instance.pending.timer); instance.pending.reject(error); instance.pending = null; }
  };
  child.on('error', failed);
  child.on('close', (code, signal) => { instance.closed = true; failed(Error(`Renderer closed: ${code ?? signal}`)); });
  return instance;
}
async function close(instance, reason) {
  if (!instance || instance.closed) return;
  const closed = once(instance.child, 'close'); instance.child.kill(); await closed;
  write({ event: 'retired', generation: instance.generation, reason });
}
async function request(instance, input) {
  if (instance.pending || instance.closed) throw Error('Renderer request lifecycle violation');
  const id = ++sequence, started = performance.now();
  const response = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { instance.pending = null; instance.child.kill(); reject(Error('Renderer timeout')); }, 120000);
    instance.pending = { id, resolve, reject, timer };
    instance.child.stdin.write(JSON.stringify({ version: 1, id, ...input }) + '\n', error => {
      if (error && instance.pending?.id === id) { clearTimeout(timer); instance.pending = null; reject(error); }
    });
  });
  const rss = response.runtime?.rssBytes;
  if (!Number.isSafeInteger(rss) || rss < 1) throw Error('Bridge did not report valid completed-response RSS');
  peak = Math.max(peak, rss);
  write({ event: 'response', sequence: id, generation: instance.generation, op: input.op, style: input.style,
    piece: input.piece, variant: input.variant, rss_bytes: rss, elapsed_ms: performance.now() - started,
    ok: response.ok, empty: response.ok && response.result?.art === null, error: response.error });
  return response;
}
async function registered() {
  if (!worker) {
    worker = start();
    const registered = await request(worker, { op: 'archive', archiveId: 'trace-archive', raw });
    if (!registered.ok) throw Error(`Archive registration failed: ${JSON.stringify(registered.error)}`);
  }
  return worker;
}

try {
  worker = start();
  const catalog = await request(worker, { op: 'catalog' });
  if (!catalog.ok) throw Error(`Catalog failed: ${JSON.stringify(catalog.error)}`);
  await close(worker, 'catalog-only'); worker = undefined;
  for (const style of catalog.result.styles) {
    const beganStyle = performance.now(); let count = 0, stylePeak = 0;
    for (const board of style.board) {
      const piece = style.pieces.find(piece => piece.id === board.id);
      const variant = board.variant ?? piece?.defaultVariant ?? piece?.variants?.[0]?.id;
      const current = await registered();
      const response = await request(current, { op: 'render', archiveId: 'trace-archive', style: style.id,
        piece: board.id, ...(variant ? { variant } : {}), lang: options.lang ?? 'zh', group: 'auto', named: false, flatten: true });
      count++; completed++; stylePeak = Math.max(stylePeak, response.runtime.rssBytes);
      if (!response.ok) failures.push({ style: style.id, piece: board.id, variant, error: response.error });
      else if (response.result.art === null) empty++;
      // Registration plus render is one operation. Never retire in between.
      if (response.runtime.rssBytes >= threshold) { retired++; await close(worker, 'rss-threshold'); worker = undefined; }
    }
    const summary = { style: style.id, requests: count, elapsed_ms: performance.now() - beganStyle, peak_rss_bytes: stylePeak };
    styleSummaries.push(summary); write({ event: 'style-complete', ...summary });
    console.log(JSON.stringify(summary));
  }
} catch (error) { failures.push({ fatal: error.message }); process.exitCode = 1; }
finally {
  await close(worker, 'trace-complete');
  const summary = { schema_version: 1, kind: 'renderer-rss-trace', complete: styleSummaries.length === 26 && failures.length === 0,
    fixture: { path: fixture, sha256: fileHash(fixture) }, bridge_sha256: fileHash(bridge), driver_sha256: fileHash(import.meta.filename),
    source_state: options['candidate-sha'] ?? 'working-tree; source hashes recorded', platform: process.platform, arch: process.arch, cpu: os.cpus()[0]?.model,
    node: process.version, threshold_bytes: threshold, requested_styles: 26, completed_styles: styleSummaries.length,
    requests: completed, empty, worker_generations: generation, rss_retirements: retired, peak_completed_response_rss_bytes: peak,
    elapsed_ms: Date.now() - began, styles: styleSummaries, failures,
    limits: ['Renderer subprocess RSS only, excludes GPUI/GPU/driver processes.', 'Completed-response samples can miss transient peaks.',
      'Public Git-only archives do not exercise real private agent logs.', '768 MiB is a retirement threshold, not a per-operation peak cap.'] };
  fs.writeFileSync(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2) + '\n');
  fs.closeSync(log); fs.closeSync(errors);
  if (!summary.complete) process.exitCode = 1;
  console.log(JSON.stringify({ complete: summary.complete, requests: completed, worker_generations: generation, rss_retirements: retired, peak_mib: peak / 1024 / 1024, failures: failures.length }));
}
