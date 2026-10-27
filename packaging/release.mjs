// Release filenames are shared by the packager and website.
import { readFileSync, writeFileSync, readdirSync, copyFileSync, mkdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
export const ROOT = path.resolve(import.meta.dirname, '..');
export const version = JSON.parse(readFileSync(path.join(ROOT, 'packages/cli/package.json'))).version;
export const apps = [
  { os: 'mac', arch: 'aarch64', target: 'aarch64-apple-darwin', bundle: 'dmg', file: `creditia_${version}_aarch64.dmg` },
  { os: 'mac', arch: 'x64', target: 'x86_64-apple-darwin', bundle: 'dmg', file: `creditia_${version}_x64.dmg` },
  { os: 'win', arch: 'x64', target: 'x86_64-pc-windows-msvc', bundle: 'nsis', file: `creditia_${version}_x64-setup.exe` },
  { os: 'linux', arch: 'x64', target: 'x86_64-unknown-linux-gnu', bundle: 'appimage', file: `creditia_${version}_amd64.AppImage` },
  { os: 'linux', arch: 'x64', target: 'x86_64-unknown-linux-gnu', bundle: 'deb', file: `creditia_${version}_amd64.deb` },
];
export const cores = ['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'x86_64-pc-windows-msvc'];
export const coreFile = target => `creditia-core-${target}${target.includes('windows') ? '.exe' : ''}`;
export function checkVersion(tag) {
  for (const file of ['apps/desktop/package.json', 'apps/desktop/src-tauri/tauri.conf.json', 'packages/render/package.json']) {
    if (JSON.parse(readFileSync(path.join(ROOT, file))).version !== version) throw Error(`Version mismatch: ${file}`);
  }
  for (const file of ['Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml']) {
    if (/^version = "([^"]+)"/m.exec(readFileSync(path.join(ROOT, file), 'utf8'))?.[1] !== version) throw Error(`Version mismatch: ${file}`);
  }
  if (tag && tag !== `v${version}`) throw Error(`Tag ${tag} does not match v${version}`);
}
export function assemble(dir) {
  const files = [...cores.map(coreFile), ...apps.map(a => a.file)];
  const hashes = new Map(files.map(file => [file, createHash('sha256').update(readFileSync(path.join(dir, file))).digest('hex')]));
  for (const [file, hash] of hashes) writeFileSync(path.join(dir, `${file}.sha256`), `${hash}  ${file}\n`);
  writeFileSync(path.join(dir, 'SHA256SUMS'), [...hashes].map(([file, hash]) => `${hash}  ${file}\n`).join(''));
  const template = readFileSync(path.join(ROOT, 'packaging/homebrew/Casks/creditia.rb.in'), 'utf8');
  writeFileSync(path.join(dir, 'creditia.rb'), template.replaceAll('@VERSION@', version)
    .replace('@ARM_SHA256@', hashes.get(apps[0].file)).replace('@INTEL_SHA256@', hashes.get(apps[1].file)));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, arg, dest] = process.argv.slice(2);
  checkVersion(process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : undefined);
  if (command === 'collect') {
    mkdirSync(dest, { recursive: true });
    const built = path.join(ROOT, 'apps/desktop/src-tauri/target', arg, 'release/bundle');
    for (const app of apps.filter(a => a.target === arg)) {
      const folder = path.join(built, app.bundle);
      // Tauri's filename must agree with the website; do not silently rename a different architecture.
      if (!readdirSync(folder).includes(app.file)) throw Error(`Missing installer: ${app.file}`);
      copyFileSync(path.join(folder, app.file), path.join(dest, app.file));
    }
  } else if (command === 'assemble') assemble(arg);
  else if (command !== 'check') throw Error('Usage: release.mjs check | collect <target> <dir> | assemble <dir>');
}
