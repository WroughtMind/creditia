#!/usr/bin/env node
// resvg owns native font objects whose memory is reclaimed with their process.
// Sequential disjoint subshards keep a long all-preferences/export run bounded
// without skipping rows or weakening comparisons. Each child owns its manifest.
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { args, positiveInteger } from './common.mjs';

export function partitions(shards, shard, batches) {
  if (![shards, batches].every(n => Number.isSafeInteger(n) && n > 0) || !Number.isSafeInteger(shard) || shard < 0 || shard >= shards) throw Error('Invalid shard partition');
  return Array.from({ length: batches }, (_, index) => ({ shards: shards * batches, shard: shard + index * shards }));
}

async function main() {
  const options = args();
  if (!options.output) throw Error('output is required');
  const batches = positiveInteger(options.batches, 'batches', 8);
  const shards = positiveInteger(options.shards, 'shards', 1), shard = Number(options.shard ?? 0);
  const output = path.resolve(options.output), reports = [];
  fs.mkdirSync(output, { recursive: true });
  for (const [index, partition] of partitions(shards, shard, batches).entries()) {
    const directory = `batch-${index}`, childOutput = path.join(output, directory);
    const childOptions = { ...options, ...partition, output: childOutput };
    delete childOptions.batches;
    const argv = Object.entries(childOptions).flatMap(([key, value]) => value === true ? [`--${key}`] : [`--${key}`, String(value)]);
    console.log(JSON.stringify({ batch: index + 1, batches, ...partition }));
    const child = spawn(process.execPath, [path.join(import.meta.dirname, 'render-matrix.mjs'), ...argv], { stdio: 'inherit' });
    const result = await new Promise(resolve => {
      child.once('error', error => resolve({ code: null, signal: null, error: error.message }));
      child.once('exit', (code, signal) => resolve({ code, signal }));
    });
    const filename = path.join(childOutput, 'manifest.json');
    const manifest = fs.existsSync(filename) ? JSON.parse(fs.readFileSync(filename, 'utf8')) : null;
    // Empty modulo partitions are not rendering evidence. They are explicitly
    // retained in the orchestration record and excluded from report discovery.
    if (manifest?.complete && manifest.cases.length === 0) fs.renameSync(filename, path.join(childOutput, 'empty-partition.json'));
    reports.push({ directory, ...partition, ...result, complete: manifest?.complete === true,
      cases: manifest?.cases?.length ?? 0, summary: manifest?.summary ?? {} });
    fs.writeFileSync(path.join(output, 'orchestration.json'), JSON.stringify({ batches: reports }, null, 2) + '\n');
  }
  fs.writeFileSync(path.join(output, 'index.html'), `<!doctype html><meta charset="utf-8"><title>Creditia artwork batches</title><h1>Artwork parity evidence</h1><p>Sequential native-renderer processes; every nonempty batch has independent source, font and artifact hashes.</p><ul>${reports.map(r => `<li><a href="${r.directory}/index.html">${r.directory}</a>: ${r.cases} cases, ${JSON.stringify(r.summary)}, exit ${r.code}, complete ${r.complete}</li>`).join('')}</ul>`);
  if (!reports.some(r => r.cases > 0) || reports.some(r => r.code !== 0 || !r.complete)) process.exitCode = 1;
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) await main();
