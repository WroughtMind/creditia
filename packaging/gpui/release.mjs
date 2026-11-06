#!/usr/bin/env node
/** Prepare a draft GPUI prerelease from immutable, already-tested artifacts.
 * No rebuild, branch update, tag replacement, or stable release promotion occurs.
 */
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { createWriteStream } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const TARGETS = ['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu'];
const REPORT_ASSETS = ['report.json','REPORT.md','ACCEPTANCE.zh.md','ARTWORK-STATUS.tsv','FEATURE-STATUS.tsv'];
const SHA = /^[a-f0-9]{40}$/;
const HASH = /^[a-f0-9]{64}$/;
const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;
const TAG = /^gpui-v\d+\.\d+\.\d+-[A-Za-z0-9][A-Za-z0-9.-]*$/;
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export async function fileHash(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}
const json = async file => JSON.parse(await fs.readFile(file, 'utf8'));
export function validateRequest(input) {
  if (input.enabled === false) return { enabled: false };
  if (input.enabled !== true) throw Error('A repository request must explicitly set enabled:true');
  const fields = ['candidate_sha', 'native_run', 'performance_run', 'core_run', 'review_commit', 'review_path', 'tag'];
  for (const field of fields) if (typeof input[field] !== 'string') throw Error(`Release request needs string ${field}`);
  if (!SHA.test(input.candidate_sha) || !SHA.test(input.review_commit) || !TAG.test(input.tag)) throw Error('Release request needs immutable commits and a GPUI prerelease tag');
  for (const field of fields.filter(field => field.endsWith('_run'))) if (!/^[1-9]\d*$/.test(input[field])) throw Error(`Invalid workflow run ID: ${field}`);
  if (!/^[A-Za-z0-9_./-]+\.json$/.test(input.review_path)) throw Error('Review path must be a repository JSON path');
  safeRelative(input.review_path);
  if (input.publish_downloads !== undefined && typeof input.publish_downloads !== 'boolean') throw Error('publish_downloads must be a boolean');
  return Object.fromEntries([['enabled', true], ['publish_downloads', input.publish_downloads === true], ...fields.map(field => [field, input[field]])]);
}
export function safeRelative(name) {
  if (typeof name !== 'string' || !name || name.includes('\\') || name.startsWith('/') || name.split('/').some(part => !part || part === '.' || part === '..') || /^[A-Za-z]:/.test(name)) throw Error(`Unsafe relative path: ${name}`);
  return name;
}
async function files(root, prefix = '') {
  const result = [];
  for (const item of await fs.readdir(path.join(root, prefix), { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${item.name}` : item.name;
    if (item.isSymbolicLink()) throw Error(`Symbolic links are not accepted in a release bundle: ${relative}`);
    if (item.isDirectory()) result.push(...await files(root, relative));
    else if (item.isFile()) result.push(relative);
    else throw Error(`Non-regular release artifact: ${relative}`);
  }
  return result.sort();
}
export function runMatchesCandidate(run, candidate, workflow, repo) {
  if (run.status !== 'completed' || run.conclusion !== 'success') throw Error(`${workflow} run did not complete successfully`);
  if (run.path !== `.github/workflows/${workflow}.yml`) throw Error(`Wrong source workflow: ${run.path}`);
  if (run.repository?.full_name !== repo || run.head_repository?.full_name !== repo) throw Error('Release evidence must come from this repository');
  // A PR run records the branch head, while checkout/GITHUB_SHA use the tested
  // merge commit. Require its exact parents as well as artifact-level SHA checks.
  if (run.head_sha === candidate.sha) return;
  const parents = candidate.parents?.map(parent => parent.sha) ?? [];
  if (run.event !== 'pull_request' || parents.length !== 2 || !parents.includes(run.head_sha) ||
      !run.pull_requests?.some(pr => pr.head?.sha === run.head_sha && parents.includes(pr.base?.sha))) {
    throw Error(`${workflow} run is not bound to the exact tested candidate commit`);
  }
}
export function validateReport(report, candidate) {
  if (report.schema_version !== 1 || report.candidate_sha !== candidate || report.release_ready !== true) throw Error('Combined acceptance report is not release-ready for this candidate');
  if (report.invalid_evidence?.length !== 0 || report.native_failures?.length !== 0 || report.recorded_review?.status !== 'complete' || report.native_platforms_complete !== true || report.feature_failures?.length !== 0) throw Error('Combined acceptance report has missing or failed migration gates');
  const platforms = new Set((report.package_platforms ?? []).filter(item => item.acceptance?.passed === true && item.installed_workflow === true).map(item => item.platform));
  for (const platform of ['darwin-arm64', 'darwin-x64', 'win32-x64', 'linux-x64']) if (!platforms.has(platform)) throw Error(`Missing accepted package platform ${platform}`);
  for (const scenario of ['startup', 'scroll', 'roam', 'hidden-idle']) {
    if (!report.performance_comparisons?.some(item => item.scenario === scenario && item.status === 'comparable')) throw Error(`Missing same-host performance: ${scenario}`);
  }
}
export async function verifyInstallers(root, candidate, target) {
  const info = await json(path.join(root, 'candidate.json'));
  if (info.candidate_sha !== candidate || info.target !== target || !Array.isArray(info.installers) || info.installers.length !== 2) throw Error(`Installer provenance/scope differs for ${target}`);
  const sums = new Map();
  for (const line of (await fs.readFile(path.join(root, 'SHA256SUMS'), 'utf8')).trim().split('\n')) {
    const match = /^([a-f0-9]{64})  ([A-Za-z0-9_.-]+)$/.exec(line);
    if (!match || sums.has(match[2])) throw Error('Invalid or duplicate installer checksum');
    sums.set(match[2], match[1]);
  }
  if (sums.size !== info.installers.length) throw Error('Checksum manifest does not match installer scope');
  const expected = target.includes('apple') ? ['.zip', '.dmg'] : target.includes('windows') ? ['.zip', '-setup.exe'] : ['.tar.gz', '.deb'];
  const records = [];
  for (const file of info.installers) {
    safeRelative(file);
    if (path.basename(file) !== file || !file.startsWith(`creditia-gpui-${target}`) || !sums.has(file)) throw Error('Unlisted or unexpected installer');
    const stat = await fs.lstat(path.join(root, file));
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size === 0 || await fileHash(path.join(root, file)) !== sums.get(file)) throw Error(`Installer checksum mismatch: ${file}`);
    records.push({ path: file, bytes: stat.size, sha256: sums.get(file) });
  }
  if (!expected.every(extension => records.some(item => item.path.endsWith(extension)))) throw Error(`Incomplete installer formats for ${target}`);
  return records;
}

function options(argv) {
  const result = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!/^--[a-z][a-z0-9-]*$/.test(argv[i]) || !argv[i + 1] || argv[i + 1].startsWith('--') || Object.hasOwn(result, argv[i].slice(2))) throw Error(`Invalid option ${argv[i]}`);
    result[argv[i].slice(2)] = argv[i + 1];
  }
  return result;
}
async function api(repo, resource, { allow404 = false } = {}) {
  if (!REPOSITORY.test(repo)) throw Error('Invalid repository');
  const response = await fetch(`https://api.github.com/repos/${repo}/${resource}`, { headers: { Accept: 'application/vnd.github+json', Authorization: `Bearer ${process.env.GH_TOKEN}`, 'X-GitHub-Api-Version': '2022-11-28' }, signal: AbortSignal.timeout(60_000) });
  if (allow404 && response.status === 404) return null;
  if (!response.ok) throw Error(`GitHub metadata request failed: ${response.status} ${resource}`);
  return response.json();
}
async function downloadSealedArchive(repo,item,archive) {
  if (item.expired || !/^sha256:[a-f0-9]{64}$/.test(item.digest ?? '')) throw Error(`Artifact ${item.name} is expired or lacks its immutable server digest`);
  const response = await fetch(`https://api.github.com/repos/${repo}/actions/artifacts/${item.id}/zip`, { headers: { Authorization: `Bearer ${process.env.GH_TOKEN}`, Accept: 'application/vnd.github+json' }, signal: AbortSignal.timeout(600_000) });
  if (!response.ok || !response.body) throw Error(`Artifact ${item.id} download failed: ${response.status}`);
  await pipeline(Readable.fromWeb(response.body), createWriteStream(archive));
  if (`sha256:${await fileHash(archive)}` !== item.digest) throw Error(`GitHub artifact archive checksum mismatch: ${item.name}`);
}
async function downloadArtifact(repo, item, root, receipt) {
  const archive = path.join(root, `${item.id}.zip`), destination = path.join(root, 'evidence', item.name);
  if (!/^[A-Za-z0-9_.-]+$/.test(item.name)) throw Error('Unsafe artifact name');
  await fs.mkdir(destination,{recursive:true});
  await downloadSealedArchive(repo,item,archive);
  // Inspect every member before extraction. Artifact ZIPs are never executed.
  execFileSync('python3', ['-c', `import pathlib,stat,sys,zipfile
root=pathlib.Path(sys.argv[2]).resolve()
with zipfile.ZipFile(sys.argv[1]) as archive:
 seen=set()
 for entry in archive.infolist():
  name=entry.filename
  parts=pathlib.PurePosixPath(name).parts
  if not name or name.startswith('/') or '\\\\' in name or ':' in name or '..' in parts or name in seen: raise ValueError('Unsafe/duplicate artifact member: '+name)
  seen.add(name)
  mode=entry.external_attr>>16
  if stat.S_ISLNK(mode): raise ValueError('Artifact symlink rejected')
  if not (root.joinpath(*parts).resolve().is_relative_to(root)): raise ValueError('Artifact member escaped output')
 archive.extractall(root)
`, archive, destination], { stdio: 'inherit' });
  await fs.rm(archive);
  receipt.artifacts.push({ id: item.id, name: item.name, digest: item.digest, size_in_bytes: item.size_in_bytes, workflow_run_id: item.workflow_run?.id });
  return destination;
}
async function artifactList(repo, run) {
  const list = [];
  for (let page = 1; ; page++) {
    const response = await api(repo, `actions/runs/${run}/artifacts?per_page=100&page=${page}`);
    list.push(...response.artifacts);
    if (list.length >= response.total_count) return list;
  }
}
// The downloadable embed bundle contains its own font manifest. Keep the
// runnable build intact, alongside the evidence rather than in its scan root.
export async function retainWebBuild(directory, output, candidate) {
  const embed = path.join(directory, 'packages/embed');
  const browser = await json(path.join(embed,'browser-evidence/report.json'));
  const dist = path.join(embed,'dist');
  if (browser.candidateSha !== candidate || browser.status !== 'passed' || browser.errors?.length !== 0 ||
      !browser.cases?.length || browser.cases.some(c => c.status !== 'passed' || !c.checks?.length || c.checks.some(check => check.passed !== true)) ||
      browser.motion?.controls !== 'passed' || browser.motion?.reducedMotion !== 'passed' || !(browser.motion.frameCount > 1) ||
      !browser.screenshots?.length || browser.bundleSha256 !== await fileHash(path.join(dist,'creditia-card.js'))) {
    throw Error('Browser result does not verify the observed interactions and matching compiled embed bundle');
  }
  for (const name of browser.screenshots) {
    const screenshot=await fs.readFile(path.join(embed,'browser-evidence',safeRelative(name)));
    if (!screenshot.subarray(0,8).equals(Buffer.from('89504e470d0a1a0a','hex'))) throw Error(`Invalid browser screenshot: ${name}`);
  }
  const saved=path.join(output,'web-build/creditia-web-renderer/packages/embed/dist');
  await fs.mkdir(path.dirname(saved),{recursive:true});
  await fs.rename(dist,saved);
  return {candidate_sha:candidate,cases:browser.cases.length,screenshots:browser.screenshots.length,
    bundle_sha256:browser.bundleSha256,retained_bundle:'web-build/creditia-web-renderer/packages/embed/dist'};
}
async function reviewAttachment(repo,commit,reviewPath,description,ext,signature,destination) {
  const name = safeRelative(description.file);
  if (path.posix.basename(name) !== name || !name.endsWith(ext) || !HASH.test(description.sha256)) throw Error('Invalid reviewer attachment reference');
  const file = path.posix.join(path.posix.dirname(reviewPath),name);
  const metadata = await api(repo,`contents/${file.split('/').map(encodeURIComponent).join('/')}?ref=${commit}`);
  if (metadata.type !== 'file' || metadata.size > 4 * 1024 * 1024 || !SHA.test(metadata.sha)) throw Error('Invalid reviewer attachment file');
  const blob = await api(repo,`git/blobs/${metadata.sha}`);
  if (blob.encoding !== 'base64') throw Error('Invalid reviewer attachment encoding');
  const bytes = Buffer.from(blob.content,'base64');
  if (bytes.length !== metadata.size || digest(bytes) !== description.sha256 || !bytes.subarray(0,signature.length).equals(signature)) throw Error(`Reviewer attachment changed: ${file}`);
  await fs.writeFile(path.join(destination,name),bytes,{flag:'wx'});
  return {file,git_blob:metadata.sha,bytes:bytes.length,sha256:description.sha256};
}
async function retainReviewImages(repo, commit, reviewPath, review, output) {
  const images = review.images ?? [];
  if (!Array.isArray(images) || images.length > 10) throw Error('Invalid reviewer image list');
  const root = path.join(output, 'review-images');
  await fs.mkdir(root);
  const records=[];
  for (const description of images) {
    records.push(await reviewAttachment(repo,commit,reviewPath,description,'.jpg',Buffer.from('ffd8ff','hex'),root));
  }
  return records;
}
async function retainRollback(repo, record, assets) {
  if (!record) return null;
  if (!/^[1-9][0-9]*$/.test(record.id) || !/^[1-9][0-9]*$/.test(record.run) || !HASH.test(record.sha256) ||
      !/^[A-Za-z0-9_.-]+\.zip$/.test(record.name) || !record.name.toLowerCase().includes('rollback')) {
    throw Error('Rollback needs an immutable artifact, source run, checksum and ZIP name');
  }
  const item = await api(repo,`actions/artifacts/${record.id}`);
  if (item.workflow_run?.id !== Number(record.run) || item.digest !== `sha256:${record.sha256}`) throw Error('Rollback artifact differs from the reviewed build');
  const file = path.join(assets,record.name);
  await downloadSealedArchive(repo,item,file);
  execFileSync('python3',['-c',`import hashlib,re,sys,zipfile
with zipfile.ZipFile(sys.argv[1]) as archive:
 sums=archive.read("SHA256SUMS").decode().splitlines()
 if not sums: raise ValueError("empty rollback checksum list")
 for line in sums:
  match=re.fullmatch(r"([a-f0-9]{64})  ([A-Za-z0-9_.-]+)",line)
  if not match or hashlib.sha256(archive.read(match[2])).hexdigest()!=match[1]: raise ValueError("rollback payload checksum mismatch")
`,file],{stdio:'inherit'});
  return {...record,bytes:(await fs.stat(file)).size};
}
export function reviewedArtifact(available, name, pinnedId) {
  if (pinnedId !== undefined && !/^[1-9]\d*$/.test(String(pinnedId))) throw Error(`Invalid reviewed artifact ID for ${name}`);
  const matches = available.filter(item => item.name === name && !item.expired &&
    (pinnedId === undefined || String(item.id) === String(pinnedId)));
  if (matches.length !== 1) throw Error(`Expected one immutable artifact ${name}, found ${matches.length}`);
  return matches[0];
}
async function prepare(opts) {
  const candidate = opts['candidate-sha'], repo = process.env.GITHUB_REPOSITORY, output = path.resolve(opts.output ?? 'prepared');
  if (!SHA.test(candidate ?? '') || !SHA.test(opts['review-commit'] ?? '') || !TAG.test(opts.tag ?? '')) throw Error('Use full immutable candidate/review commits and a gpui-vX.Y.Z-prerelease tag');
  if (!process.env.GH_TOKEN || !REPOSITORY.test(repo ?? '')) throw Error('Repository and read-only GitHub token are required');
  safeRelative(opts['review-path']);
  const candidateRoot = path.resolve(opts['candidate-root']);
  if (execFileSync('git', ['-C', candidateRoot, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() !== candidate) throw Error('Candidate checkout is not the tested commit');
  try { await fs.access(output); throw Error('Release output must be a new directory'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  await fs.mkdir(output, { recursive: true });
  const metadata = await api(repo, `commits/${candidate}`);
  if (await api(repo, `git/ref/tags/${encodeURIComponent(opts.tag)}`, { allow404: true })) throw Error('Release tag already exists; immutable candidates are never replaced');
  const provenance = { schema_version: 1, candidate_sha: candidate, repository: repo, tag: opts.tag, prepared_at: new Date().toISOString(), runs: [], artifacts: [] };
  const reviewResponse = await api(repo, `contents/${opts['review-path'].split('/').map(encodeURIComponent).join('/')}?ref=${opts['review-commit']}`);
  if (reviewResponse.type !== 'file' || reviewResponse.encoding !== 'base64') throw Error('Review must be an immutable committed JSON file');
  const reviewBytes = Buffer.from(reviewResponse.content, 'base64'), review = JSON.parse(reviewBytes);
  if (review.candidate_sha !== candidate) throw Error('Recorded review belongs to another candidate');
  const nativeArtifacts = new Map();
  const definitions = [
    ['native-run', 'gpui', [...TARGETS.map(target => `gpui-candidate-${target}`), ...TARGETS.map(target => `gpui-evidence-${target}`)]],
    ['performance-run', 'gpui-performance', ['gpui-production-performance']],
    ['core-run', 'ci', ['creditia-web-renderer', 'creditia-features-core', 'creditia-features-windows-service', 'creditia-features-render']],
  ];
  for (const [input, workflow, required] of definitions) {
    const id = opts[input];
    if (!/^[1-9]\d*$/.test(id ?? '')) throw Error(`Missing numeric ${input}`);
    const run = await api(repo, `actions/runs/${id}`);
    runMatchesCandidate(run, metadata, workflow, repo);
    provenance.runs.push({ id: run.id, path: run.path, head_sha: run.head_sha, event: run.event, attempt: run.run_attempt, html_url: run.html_url, conclusion: run.conclusion });
    const available = await artifactList(repo, id);
    for (const name of required) {
      const artifact = reviewedArtifact(available, name, review.artifact_ids?.[name]);
      const destination = await downloadArtifact(repo, artifact, output, provenance);
      if (name === 'creditia-web-renderer') provenance.browser = await retainWebBuild(destination, output, candidate);
      if (name.startsWith('gpui-candidate-')) nativeArtifacts.set(name.slice('gpui-candidate-'.length), destination);
    }
  }
  await fs.writeFile(path.join(output, 'review.json'), reviewBytes);
  provenance.review = { commit: opts['review-commit'], path: opts['review-path'], git_blob: reviewResponse.sha, sha256: digest(reviewBytes) };
  provenance.review.images = await retainReviewImages(repo,opts['review-commit'],opts['review-path'],review,output);
  await fs.writeFile(path.join(output, 'provenance.json'), JSON.stringify(provenance, null, 2) + '\n');
  execFileSync(process.execPath, [path.join(candidateRoot, 'packaging/gpui-acceptance/report.mjs'), '--evidence', path.join(output, 'evidence'), '--candidate-sha', candidate, '--review', path.join(output, 'review.json'), '--output', path.join(output, 'report'), '--require-complete'], { stdio: 'inherit' });
  const report = await json(path.join(output, 'report/report.json'));
  validateReport(report, candidate);
  const assets = path.join(output, 'assets');
  await fs.mkdir(assets);
  if (review.delivery_note) {
    provenance.review.delivery_note = await reviewAttachment(repo,opts['review-commit'],opts['review-path'],review.delivery_note,
      '.pdf',Buffer.from('%PDF-'),assets);
  }
  provenance.rollback = await retainRollback(repo,review.rollback,assets);
  const installers = [];
  const installedProof = [];
  for (const relative of await files(path.join(output, 'evidence'))) if (relative.endsWith('/manifest.json')) {
    const proof = await json(path.join(output, 'evidence', relative));
    if (proof.kind === 'package' && proof.complete === true && proof.acceptance?.passed === true && proof.provenance?.candidate_sha === candidate && proof.installers?.length) installedProof.push(proof);
  }
  for (const target of TARGETS) {
    const directory = nativeArtifacts.get(target);
    for (const record of await verifyInstallers(directory, candidate, target)) {
      if (!installedProof.some(proof => proof.target === target && proof.installers.some(item => item.name === record.path && item.sha256 === record.sha256 && item.bytes === record.bytes))) throw Error(`Installer bytes differ from the installed/tested artifact: ${record.path}`);
      await fs.copyFile(path.join(directory, record.path), path.join(assets, record.path));
      installers.push({ ...record, target });
    }
  }
  provenance.installers = installers;
  await fs.writeFile(path.join(output, 'provenance.json'), JSON.stringify(provenance, null, 2) + '\n');
  await fs.copyFile(path.join(candidateRoot, 'packaging/gpui/INSTALL.md'), path.join(assets, 'INSTALL.md'));
  for (const file of REPORT_ASSETS) await fs.copyFile(path.join(output, 'report', file), path.join(assets, file));
  // Full raw evidence survives CI retention, including individual native pixels.
  // Installer payloads are release assets already, so omit their duplicate copies.
  for (const [target, directory] of nativeArtifacts) {
    for (const record of installers.filter(item => item.target === target)) await fs.rm(path.join(directory, record.path));
  }
  execFileSync('tar', ['-czf', path.join(assets, 'gpui-acceptance-evidence.tar.gz'), '-C', output, 'evidence', 'web-build', 'review-images', 'review.json', 'report', 'provenance.json'], { stdio: 'inherit' });
  const archivePath = path.join(assets, 'gpui-acceptance-evidence.tar.gz');
  const archiveSize = (await fs.stat(archivePath)).size;
  const archiveRecord = { name: 'gpui-acceptance-evidence.tar.gz', bytes: archiveSize, sha256: await fileHash(archivePath), parts: [] };
  const partSize = 1024 * 1024 * 1024;
  if (archiveSize > partSize) {
    for (let offset = 0, index = 0; offset < archiveSize; offset += partSize, index++) {
      const name = `gpui-acceptance-evidence.tar.gz.part-${String(index).padStart(3, '0')}`;
      const dest = path.join(assets, name), end = Math.min(offset + partSize, archiveSize) - 1;
      await pipeline(createReadStream(archivePath, { start: offset, end }), createWriteStream(dest));
      archiveRecord.parts.push({ path: name, bytes: end - offset + 1, sha256: await fileHash(dest) });
    }
    await fs.rm(archivePath);
  } else archiveRecord.parts.push({ path: archiveRecord.name, bytes: archiveSize, sha256: archiveRecord.sha256 });
  await fs.writeFile(path.join(assets, 'evidence-archive.json'), JSON.stringify(archiveRecord, null, 2) + '\n');
  const checksumLines = [];
  for (const file of await files(assets)) checksumLines.push(`${await fileHash(path.join(assets, file))}  ${file}`);
  await fs.writeFile(path.join(assets, 'SHA256SUMS'), checksumLines.join('\n') + '\n');
  const notes = `# Creditia GPUI ${opts.tag}\n\nTested source: \`${candidate}\`. These prerelease downloads contain the exact installers exercised on all four targets.\n\nAll four native packages, installation checks, the reviewed desktop journey, representative visual checks and same-host process-tree measurements passed the combined acceptance gate. See REPORT.md, ACCEPTANCE.zh.md and report.json. ARTWORK-STATUS.tsv and FEATURE-STATUS.tsv retain the per-item results; the complete source evidence is retained in the archive described by evidence-archive.json (large archives are split into ordered 1 GiB parts). Concatenate parts in manifest order before extracting the tar.gz; its reconstructed SHA-256 is recorded there.\n\nmacOS uses ad-hoc signatures without notarization; Windows packages are unsigned. INSTALL.md explains installation, OS trust prompts, upgrade and rollback. No externally installed Node or downloaded fonts are required.\n\n${provenance.rollback ? `## Rollback\n\nDownload ${provenance.rollback.name} for the previous desktop. Its SHA-256 is \`${provenance.rollback.sha256}\`. Quit the GPUI app and keep the library and project archives in place before opening Tauri.\n\n` : ''}## Verified workflow runs\n\n${provenance.runs.map(run => `- [${run.path} run ${run.id}](${run.html_url})`).join('\n')}\n`;
  await fs.writeFile(path.join(output, 'release-notes.md'), notes);
  const records = [];
  for (const file of [...await files(assets)].map(file => `assets/${file}`).concat(['release-notes.md'])) records.push({ path: file, bytes: (await fs.stat(path.join(output, file))).size, sha256: await fileHash(path.join(output, file)) });
  const bundle = { schema_version: 1, repository: repo, candidate_sha: candidate, tag: opts.tag, draft: true, prerelease: true, release_ready: true, assets: records };
  const bytes = Buffer.from(JSON.stringify(bundle, null, 2) + '\n');
  await fs.writeFile(path.join(output, 'release-bundle.json'), bytes);
  if (process.env.GITHUB_OUTPUT) await fs.appendFile(process.env.GITHUB_OUTPUT, `manifest_sha256=${digest(bytes)}\n`);
  console.log(JSON.stringify({ candidate_sha: candidate, release_ready: true, installers: installers.length, manifest_sha256: digest(bytes) }));
}
export async function verifyBundle(root, expected) {
  const bytes = await fs.readFile(path.join(root, 'release-bundle.json'));
  if (!HASH.test(expected ?? '') || digest(bytes) !== expected) throw Error('Prepared release manifest checksum mismatch');
  const bundle = JSON.parse(bytes);
  if (bundle.schema_version !== 1 || !SHA.test(bundle.candidate_sha ?? '') || !REPOSITORY.test(bundle.repository ?? '') || !TAG.test(bundle.tag ?? '') || bundle.release_ready !== true || bundle.draft !== true || bundle.prerelease !== true) throw Error('Only a verified draft prerelease bundle may be published');
  const names = new Set();
  for (const item of bundle.assets) {
    safeRelative(item.path);
    if (names.has(item.path) || !HASH.test(item.sha256 ?? '')) throw Error('Invalid/duplicate prepared asset');
    names.add(item.path);
    const file = path.join(root, item.path), stat = await fs.lstat(file);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size !== item.bytes || await fileHash(file) !== item.sha256) throw Error(`Prepared release asset changed: ${item.path}`);
  }
  const actual = (await files(path.join(root, 'assets'))).map(file => `assets/${file}`);
  if (actual.some(file => !names.has(file)) || REPORT_ASSETS.some(file => !names.has(`assets/${file}`)) || !names.has('release-notes.md') || !names.has('assets/evidence-archive.json') || !names.has('assets/SHA256SUMS')) throw Error('Prepared release assets are incomplete or injected');
  const archive = await json(path.join(root, 'assets/evidence-archive.json'));
  if (archive.name !== 'gpui-acceptance-evidence.tar.gz' || !HASH.test(archive.sha256 ?? '') || !archive.parts?.length) throw Error('Full evidence archive is missing');
  const reconstructed = createHash('sha256');
  let archiveBytes = 0;
  for (const part of archive.parts) {
    safeRelative(part.path);
    if (!names.has(`assets/${part.path}`) || !HASH.test(part.sha256 ?? '')) throw Error('Evidence archive part is not sealed');
    const file = path.join(root, 'assets', part.path);
    if ((await fs.stat(file)).size !== part.bytes || await fileHash(file) !== part.sha256) throw Error('Evidence archive part changed');
    for await (const chunk of createReadStream(file)) { reconstructed.update(chunk); archiveBytes += chunk.length; }
  }
  if (archiveBytes !== archive.bytes || reconstructed.digest('hex') !== archive.sha256) throw Error('Reconstructed full evidence archive checksum mismatch');
  validateReport(await json(path.join(root, 'assets/report.json')), bundle.candidate_sha);
  for (const target of TARGETS) if (actual.filter(file => file.startsWith(`assets/creditia-gpui-${target}`)).length !== 2) throw Error(`Prepared bundle lacks both installers for ${target}`);
  return bundle;
}
export function verifyUploadedRelease(release, assets, bundle) {
  if (!release.draft || !release.prerelease || release.tag_name !== bundle.tag || release.target_commitish !== bundle.candidate_sha) {
    throw Error('Draft release does not target the reviewed candidate');
  }
  const expected = bundle.assets.filter(a => a.path.startsWith('assets/'));
  if (expected.length !== assets.length) throw Error('Uploaded asset count differs from the verified bundle');
  for (const file of expected) {
    const actual = assets.find(a => a.name === path.basename(file.path));
    if (actual?.state !== 'uploaded' || actual.size !== file.bytes || actual.digest !== `sha256:${file.sha256}`) {
      throw Error(`Uploaded release asset does not match: ${file.path}`);
    }
  }
}

async function main() {
  const command = process.argv[2], opts = options(process.argv.slice(3));
  if (command === 'request') {
    const request = validateRequest(process.env.GITHUB_EVENT_NAME === 'workflow_dispatch'
      ? { ...JSON.parse(process.env.CREDITIA_RELEASE_INPUTS), enabled: true }
      : await json(opts.file));
    if (!process.env.GITHUB_OUTPUT) throw Error('Request selection requires GitHub step outputs');
    await fs.appendFile(process.env.GITHUB_OUTPUT, Object.entries(request).map(([key, value]) => `${key}=${value}`).join('\n') + '\n');
    console.log(request.enabled ? `Requested draft prerelease ${request.tag} of tested commit ${request.candidate_sha}` : 'Release request is disabled; no release will be created');
    return;
  }
  if (command === 'prepare') return prepare(opts);
  if (command === 'publish') {
    const root = path.resolve(opts.root), bundle = await verifyBundle(root, opts['manifest-sha256']);
    if (process.env.GITHUB_REPOSITORY !== bundle.repository) throw Error('Publish repository differs from preparation');
    if (await api(bundle.repository, `git/ref/tags/${encodeURIComponent(bundle.tag)}`, { allow404: true })) throw Error('Tag already exists; refusing to replace a previous candidate');
    execFileSync('gh', ['release', 'create', bundle.tag, '--repo', bundle.repository, '--target', bundle.candidate_sha, '--draft', '--prerelease', '--title', `Creditia GPUI ${bundle.tag}`, '--notes-file', path.join(root, 'release-notes.md'), ...bundle.assets.filter(item => item.path.startsWith('assets/')).map(item => path.join(root, item.path))], { stdio: 'inherit' });
    if (opts['publish-downloads'] === 'true') {
      const drafts = await api(bundle.repository, 'releases?per_page=100');
      const draft = drafts.find(item => item.draft && item.tag_name === bundle.tag);
      if (!draft) throw Error('Uploaded draft release is missing');
      const assets = await api(bundle.repository, `releases/${draft.id}/assets?per_page=100`);
      verifyUploadedRelease(draft,assets,bundle);
      execFileSync('gh',['release','edit',bundle.tag,'--repo',bundle.repository,'--draft=false','--prerelease','--latest=false'],{stdio:'inherit'});
      const published = await api(bundle.repository, `releases/tags/${encodeURIComponent(bundle.tag)}`);
      if (published.draft || !published.prerelease || published.id !== draft.id) throw Error('Prerelease publication did not complete');
    }
    return;
  }
  throw Error('Use prepare or publish');
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) await main();
