// Check downloaded candidate bytes and exercise the installed analysis core.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';
import { apps, cores, coreFile } from './release.mjs';
const [command, input, output] = process.argv.slice(2);
if (command === 'verify') {
  const sums = new Map(readFileSync(path.join(input, 'SHA256SUMS'), 'utf8').trim().split('\n').map(line => {
    const match = /^([a-f0-9]{64})  ([^/\\]+)$/.exec(line);
    assert(match, `Invalid checksum line: ${line}`);
    return [match[2], match[1]];
  }));
  for (const file of [...apps.map(a => a.file), ...cores.map(coreFile)]) {
    assert.equal(createHash('sha256').update(readFileSync(path.join(input, file))).digest('hex'), sums.get(file), file);
  }
  console.log('All 10 candidate artifacts match SHA256SUMS');
} else if (command === 'analyze') {
  mkdirSync(output, { recursive: true });
  const env = { ...process.env, CREDITIA_HOME: path.resolve(output, 'library') };
  delete env.CREDITIA_CORE;
  const result = spawnSync(path.resolve(input), ['analyze', '--repo', process.cwd(), '--out', path.resolve(output, 'credits.json'), '--json'], { env, encoding: 'utf8' });
  writeFileSync(path.join(output, 'analysis.stdout.json'), result.stdout ?? '');
  writeFileSync(path.join(output, 'analysis.stderr.log'), result.stderr ?? '');
  assert(!result.error, String(result.error));
  assert([0, 2].includes(result.status), `Core exit: ${result.status}; ${result.stderr}; ${result.stdout}`);
  const report = JSON.parse(result.stdout);
  const head = spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' });
  assert.equal(head.status, 0);
  assert.equal(report.head, head.stdout.trim());
  assert(report.commits > 0);
  const credits = JSON.parse(readFileSync(path.join(output, 'credits.json'), 'utf8'));
  assert.equal(credits.head, report.head);
  assert(credits.files.length > 0);
  console.log(JSON.stringify({ head: report.head, commits: report.commits, files: credits.files.length, exitCode: result.status, warnings: report.warnings ?? [] }));
} else throw Error('Usage: acceptance.mjs verify <candidate-dir> | analyze <installed-core> <evidence-dir>');
