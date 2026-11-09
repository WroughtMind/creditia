#!/usr/bin/env node
/** Apply a narrowly scoped, hash-recorded benchmark overlay to the old app. */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { args, commitOf, fileHash, sha256 } from './common.mjs';
function replaceOne(source, find, value, label) {
  const count = typeof find === 'string' ? source.split(find).length - 1 : [...source.matchAll(new RegExp(find.source, 'g'))].length;
  if (count !== 1) throw Error(`Expected one ${label} anchor, found ${count}`);
  return source.replace(find, value);
}

export function instrumentSources(sources) {
  if (sources.some(source => source.includes('creditiaAcceptanceStart') || source.includes('creditia_acceptance_enabled'))) throw Error('Baseline is already instrumented');
let frontend = replaceOne(sources[0], 'async function api(cmd: string, args: any = {}): Promise<any> {',
 'async function api(cmd: string, args: any = {}): Promise<any> {\n  if (creditiaAcceptanceRaw) return creditiaAcceptanceApi(cmd, args);', 'native API');
frontend = replaceOne(frontend, /(?:void )?boot\(\);/g, '', 'baseline startup');
frontend += '\n' + fs.readFileSync(path.join(import.meta.dirname, 'baseline-observer.ts'), 'utf8') + '\nvoid creditiaAcceptanceStart();\n';
let rust = replaceOne(sources[1], 'fn main() {', fs.readFileSync(path.join(import.meta.dirname, 'baseline-bridge.rs'), 'utf8') + '\nfn main() {', 'native entry');
rust = replaceOne(rust, '.setup(|app| {', '.setup(|app| {\n            if creditia_acceptance_enabled() { return Ok(()); }', 'background setup');
rust = replaceOne(rust, 'tauri::generate_handler![', 'tauri::generate_handler![acceptance_fixture, acceptance_event,', 'IPC registry');
return [frontend, rust];
}

function main() {
const o = args();
if (!o.root || !o.output) throw Error('Use --root BASELINE_CHECKOUT --output OVERLAY_RECORD.json');
const root = path.resolve(o.root), dir = import.meta.dirname;
const baseline = JSON.parse(fs.readFileSync(path.resolve(dir, '../../docs/gpui-migration/inventory.json'))).baselineCommit;
if (commitOf(root) !== baseline) throw Error(`Expected immutable baseline ${baseline}`);
const files = ['apps/desktop/src/main.ts', 'apps/desktop/src-tauri/src/main.rs'];
const sources = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'));
const after = instrumentSources(sources);
for (let i = 0; i < files.length; i++) fs.writeFileSync(path.join(root, files[i]), after[i]);
const record = { schema_version: 1, kind: 'baseline-instrumentation', baseline_sha: baseline,
 purpose: 'Isolated fixture and passive real-view telemetry; original renderer, layout and input handlers are unchanged.',
 background_services: 'disabled identically to GPUI benchmark mode',
 files: files.map((file, i) => ({ path: file, before_sha256: sha256(sources[i]), after_sha256: sha256(after[i]) })),
 instrumentation: ['baseline-observer.ts', 'baseline-bridge.rs', 'instrument-tauri.mjs'].map(file => ({ path: file, sha256: fileHash(path.join(dir, file)) })) };
fs.mkdirSync(path.dirname(path.resolve(o.output)), { recursive: true });
fs.writeFileSync(o.output, JSON.stringify(record, null, 2) + '\n');
console.log(JSON.stringify(record));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
