#!/usr/bin/env node
// Analyze a pinned public checkout on a fresh hosted runner; fail rather than
// label archives with local agent/session evidence as public Git-only input.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { args, commitOf, fileHash } from './common.mjs';
const o = args();
if (!o.core || !o.repo || !o.output) throw Error('Use --core PATH --repo PUBLIC_CHECKOUT --output ARCHIVE.json');
if (process.env.GITHUB_ACTIONS !== 'true') throw Error('Public Git-only fixture capture is restricted to the fresh hosted CI job');
const expected = JSON.parse(fs.readFileSync(path.resolve(import.meta.dirname, '../../docs/gpui-migration/inventory.json'))).baselineCommit;
const repo = path.resolve(o.repo), output = path.resolve(o.output), core = path.resolve(o.core);
if (commitOf(repo) !== expected) throw Error('Fixture checkout is not the immutable migration baseline');
fs.mkdirSync(path.dirname(output), { recursive: true });
const command = ['analyze', '--repo', repo, '--fresh', '--json', '--lang', 'en', '--out', output];
const result = spawnSync(core, command, { env: { ...process.env, CREDITIA_HOME: path.join(path.dirname(output), 'isolated-library') }, encoding: 'utf8' });
fs.writeFileSync(output + '.analysis.log', (result.stdout ?? '') + '\n' + (result.stderr ?? ''));
if (![0, 2].includes(result.status)) throw Error(`Public project analysis failed: ${result.status}`);
const raw = JSON.parse(fs.readFileSync(output, 'utf8'));
const sources = { sessions: raw.sessions?.length ?? 0, total_sessions: raw.totals?.sessions ?? 0,
  local_log_lines_written: raw.totals?.linesWritten ?? 0, tokens: raw.totals?.tokens ?? 0 };
if (raw.head !== expected || Object.values(sources).some(value => value !== 0)) {
  fs.unlinkSync(output); // Do not upload unexpected local-log content.
  throw Error(`Archive cannot be labeled public Git-only: ${JSON.stringify({ head: raw.head, sources })}`);
}
const provenance = { schema_version: 1, kind: 'real_public_git_archive', repository: 'https://github.com/WroughtMind/creditia',
  source_commit: expected, candidate_sha: process.env.GITHUB_SHA, core_sha256: fileHash(core), fixture_sha256: fileHash(output),
  generated_at: new Date().toISOString(), collection: 'public Git commits on a fresh hosted runner; no GitHub enrichment; no local agent session evidence',
  observed_source_counts: sources, totals: raw.totals, limitation: 'This fixture does not verify rich real agent-log attribution or private project migration.' };
fs.writeFileSync(output + '.provenance.json', JSON.stringify(provenance, null, 2) + '\n');
console.log(JSON.stringify(provenance));
