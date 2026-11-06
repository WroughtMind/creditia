import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

export const SCHEMA_VERSION = 1;
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const fileHash = file => sha256(fs.readFileSync(file));
export const stable = value => JSON.stringify(canonical(value));
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])]));
  return value;
}
export function args(argv = process.argv.slice(2)) {
  const result = {};
  for (let i = 0; i < argv.length; i++) {
    const name = argv[i];
    if (!/^--[a-z][a-z-]*$/.test(name)) throw Error(`Invalid option ${name}`);
    if (Object.hasOwn(result, name.slice(2))) throw Error(`Duplicate option ${name}`);
    const value = argv[i + 1];
    result[name.slice(2)] = value && !value.startsWith('--') ? argv[++i] : true;
  }
  return result;
}
export function positiveInteger(value, name, fallback) {
  if (value === undefined) return fallback;
  if (!/^\d+$/.test(String(value)) || +value < 1 || !Number.isSafeInteger(+value)) throw Error(`${name} must be a positive integer`);
  return +value;
}
export function parseMatrix(text) {
  const [header, ...lines] = text.replace(/(?:\r?\n)+$/, '').split(/\r?\n/).map(line => line.split('\t'));
  const rows = lines.map((values, i) => {
    // TSV writers may omit a final empty evidence cell; no populated field is
    // optional and missing status/identity columns still fail.
    if (values.length === header.length - 1 && header.at(-1) === 'evidence') values.push('');
    if (values.length !== header.length) throw Error(`Invalid matrix row ${i + 2}`);
    return Object.fromEntries(header.map((key, j) => [key, values[j]]));
  });
  const keys = new Set();
  for (const row of rows) {
    const key = caseKey(row);
    if (keys.has(key)) throw Error(`Duplicate matrix identity ${key}`);
    keys.add(key);
  }
  return rows;
}
export const caseKey = row => [row.renderer, row.style, row.piece, row.variant].join('/');
export const safeName = value => String(value).replace(/[^a-zA-Z0-9._-]/g, '_').slice(0, 120);
export function artifact(root, relative, bytes) {
  const target = path.resolve(root, relative);
  if (!target.startsWith(path.resolve(root) + path.sep)) throw Error('Artifact path escaped output directory');
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, bytes);
  return { path: relative.split(path.sep).join('/'), bytes: Buffer.byteLength(bytes), sha256: sha256(bytes) };
}
export function readArtifact(root, record) {
  if (!record || typeof record.path !== 'string' || !/^[a-f0-9]{64}$/.test(record.sha256)) throw Error('Artifact lacks a path or SHA-256');
  const target = path.resolve(root, record.path);
  if (!target.startsWith(path.resolve(root) + path.sep)) throw Error('Artifact path escaped evidence directory');
  if (!fs.realpathSync(target).startsWith(fs.realpathSync(root) + path.sep)) throw Error('Artifact symlink escaped evidence directory');
  const bytes = fs.readFileSync(target);
  if (fileHash(target) !== record.sha256) throw Error(`Artifact checksum mismatch: ${record.path}`);
  if (record.bytes !== undefined && bytes.length !== record.bytes) throw Error(`Artifact size mismatch: ${record.path}`);
  return bytes;
}
export function commitOf(root) {
  const result = spawnSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8' });
  if (result.status !== 0 || !/^[a-f0-9]{40}$/.test(result.stdout.trim())) throw Error(`Cannot identify Git commit of ${root}`);
  return result.stdout.trim();
}
export function cleanCheckout(root) {
  const result = spawnSync('git', ['-C', root, 'status', '--porcelain', '--untracked-files=no'], { encoding: 'utf8' });
  if (result.status !== 0) throw Error(`Cannot inspect checkout ${root}`);
  return result.stdout.trim() === '';
}
export function environment() {
  return { platform: process.platform, arch: process.arch, release: os.release(), node: process.version,
    cpu: os.cpus()[0]?.model ?? 'unknown', logical_cpus: os.cpus().length, memory_bytes: os.totalmem() };
}
// Only generated namespace prefixes differ across repeated calls. All shape,
// text, color, clipping and hit-region fields remain part of the comparison.
export function normalizeSvg(svg) {
  return svg.replace(/(\bid="|url\(#|\bhref="#)c[0-9a-z]+(?=-)/g, '$1scene')
    .replace(/(\bid="|url\(#|\bhref="#)s[0-9a-z]+(?=_)/g, '$1satori');
}
export function normalizedArt(art) {
  if (!art) return null;
  return { ...art, svg: normalizeSvg(art.svg), ...(art.crop ? { crop: { ...art.crop, svg: normalizeSvg(art.crop.svg) } } : {}) };
}
export function pixelDiff(a, b) {
  if (a.w !== b.w || a.h !== b.h) return { equal: false, dimension_mismatch: true, baseline: [a.w, a.h], candidate: [b.w, b.h] };
  if (a.data.length !== a.w * a.h * 4 || b.data.length !== b.w * b.h * 4) throw Error('Invalid RGBA buffer');
  let changed = 0, sum = 0, max = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    let pixel = false;
    for (let c = 0; c < 4; c++) { const d = Math.abs(a.data[i + c] - b.data[i + c]); sum += d; max = Math.max(max, d); pixel ||= d !== 0; }
    if (pixel) changed++;
  }
  return { equal: changed === 0, dimension_mismatch: false, changed_pixels: changed, total_pixels: a.w * a.h,
    changed_fraction: changed / (a.w * a.h), mean_channel_error: sum / a.data.length, maximum_channel_error: max };
}
export function countBy(items, read) {
  const counts = {};
  for (const item of items) { const key = read(item); counts[key] = (counts[key] ?? 0) + 1; }
  return counts;
}
