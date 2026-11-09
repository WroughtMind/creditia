#!/usr/bin/env node
// Compile the layered icon on macOS 26, then stage the same verified catalog
// on the macOS 15 runners. actool's catalog deployment target does not change
// LSMinimumSystemVersion: older systems keep the original branded .icns.
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const icons = 'apps/desktop/src-tauri/icons';
const partialName = 'assetcatalog_generated_info.plist';
const payload = ['Assets.car', partialName, 'Creditia.icns', 'assets-info.json'];
const sha = data => createHash('sha256').update(data).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';
const python = process.platform === 'win32' ? 'python' : 'python3';
const readPlist = file => JSON.parse(execFileSync(python, ['-c', 'import json, plistlib, sys; print(json.dumps(plistlib.loads(open(sys.argv[1], "rb").read())))', file], {encoding:'utf8'}));
const writePlist = (file, value) => fs.writeFile(file, execFileSync(python, ['-c', 'import json, plistlib, sys; sys.stdout.buffer.write(plistlib.dumps(json.load(sys.stdin), sort_keys=False))'], {input: JSON.stringify(value)}));

async function fileHash(file) {
  if (!(await fs.lstat(file)).isFile()) throw new Error(`Expected an icon file: ${file}`);
  const data = await fs.readFile(file);
  if (!data.length) throw new Error(`Empty icon file: ${file}`);
  return sha(data);
}

// Hash the actual Icon Composer document (including each vector layer) and
// the original fallback; a different checkout cannot silently consume it.
export async function iconSourceHashes(source) {
  const root = path.join(source, icons);
  const result = {};
  async function visit(name) {
    const file = path.join(root, name);
    if ((await fs.lstat(file)).isDirectory()) {
      for (const child of (await fs.readdir(file)).sort()) await visit(`${name}/${child}`);
    } else result[name] = await fileHash(file);
  }
  await visit('Creditia.icon');
  await visit('icon.icns');
  if (!result['Creditia.icon/icon.json']) throw new Error('Missing Icon Composer document');
  return result;
}

function iconMetadata(partial, catalog) {
  // Some actool versions omit CFBundleIconName in the partial plist. In that
  // case derive it from the compiled stack, as the Tauri bundler does.
  const stackNames = new Set(catalog.filter(r => r.AssetType === 'IconImageStack').map(r => r.Name));
  const name = partial.CFBundleIconName ?? (stackNames.size === 1 ? [...stackNames][0] : null);
  if (typeof name !== 'string' || !name || !stackNames.has(name)) throw new Error('No matching primary icon stack for partial plist');
  for (const appearance of ['NSAppearanceNameAqua','NSAppearanceNameDarkAqua','ISAppearanceTintable']) {
    const rows = catalog.filter(r => r.Appearance === appearance);
    const group = rows.find(r => r.AssetType === 'IconGroup' && r.Name === `${name}/Five petals`);
    if (!rows.some(r => r.AssetType === 'IconImageStack' && r.Name === name) || group?.LayerCount !== 5 ||
        group.Layers?.length !== 5 || !group.Layers.every(layer => layer.LayerHasLightingEffects))
      throw new Error(`Missing five lit petals in compiled icon appearance: ${appearance}`);
  }
  // Do not import platform/SDK/identity fields from actool. Its generated
  // static file is intentionally replaced by Creditia's original icon.icns.
  const keys = Object.fromEntries(Object.entries(partial).filter(([k]) => /^CFBundleIcons(?:~mac)?$/.test(k)));
  return {...keys, CFBundleIconName: name, CFBundleIconFile: 'Creditia.icns'};
}

async function installResources(app, assets, metadata) {
  const contents = path.join(app,'Contents'), resources = path.join(contents,'Resources');
  const plist = path.join(contents,'Info.plist'), info = readPlist(plist);
  await fs.mkdir(resources,{recursive:true});
  for (const name of ['Assets.car','Creditia.icns']) await fs.copyFile(path.join(assets,name),path.join(resources,name));
  await writePlist(plist,{...info,...metadata});
}

export async function installMacIcons({app, assets, source = repo}) {
  if (!assets) throw new Error('macOS staging requires --mac-icons from mac-icons.mjs compile');
  const manifest = JSON.parse(await fs.readFile(path.join(assets,'manifest.json'),'utf8'));
  if (manifest.schema !== 1 || manifest.candidate_sha !== (process.env.GITHUB_SHA ?? null))
    throw new Error('Mac icon artifact belongs to another candidate');
  const sourceFiles = await iconSourceHashes(source);
  if (Object.keys(sourceFiles).length !== Object.keys(manifest.sources ?? {}).length ||
      Object.entries(sourceFiles).some(([file, hash]) => manifest.sources?.[file] !== hash))
    throw new Error('Mac icon artifact does not match the Icon Composer source or fallback');
  for (const name of payload) {
    if (manifest.files?.[name] !== await fileHash(path.join(assets,name))) throw new Error(`Mac icon artifact checksum mismatch: ${name}`);
  }
  if (manifest.files['Creditia.icns'] !== sourceFiles['icon.icns']) throw new Error('Static icon is not the original Creditia.icns');
  const metadata = iconMetadata(readPlist(path.join(assets,partialName)), JSON.parse(await fs.readFile(path.join(assets,'assets-info.json'),'utf8')));
  await installResources(app, assets, metadata);
  return metadata;
}

export async function compileMacIcons({output, source = repo}) {
  if (process.platform !== 'darwin') throw new Error('Compile the icon on macOS 26 with Xcode 26.6');
  const macos = execFileSync('sw_vers',['-productVersion'],{encoding:'utf8'}).trim();
  const xcode = execFileSync('xcodebuild',['-version'],{encoding:'utf8'}).trim();
  if (!/^26(?:\.|$)/.test(macos) || !/^Xcode 26\.6(?:[.\n]|$)/.test(xcode)) throw new Error(`Expected macOS 26 and Xcode 26.6; got ${macos} / ${xcode}`);
  const sourceFiles = await iconSourceHashes(source);
  await fs.mkdir(output,{recursive:true});
  if ((await fs.readdir(output)).length) throw new Error(`Refusing to overwrite nonempty icon output: ${output}`);
  const tmp = await fs.mkdtemp(path.join(os.tmpdir(),'creditia-mac-icon-'));
  try {
    const input = path.join(tmp,'Icon.icon'), work = path.join(tmp,'out');
    await fs.cp(path.join(source,icons,'Creditia.icon'),input,{recursive:true});
    await fs.mkdir(work);
    // Same input basename and compiler options as Creditia's pinned Tauri CLI:
    // https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle/macos/icon.rs
    execFileSync('/bin/sh',[path.join(source,'apps/desktop/tools/macos/actool'), input,
      '--compile',work,'--output-format','human-readable-text','--notices','--warnings',
      '--output-partial-info-plist',path.join(work,partialName),'--app-icon','Icon',
      '--include-all-app-icons','--accent-color','AccentColor','--enable-on-demand-resources','NO',
      '--development-region','en','--target-device','mac','--minimum-deployment-target','26.0','--platform','macosx'], {stdio:'inherit'});
    const catalog = execFileSync('assetutil',['--info',path.join(work,'Assets.car')],{maxBuffer:16*1024*1024});
    await fs.writeFile(path.join(work,'assets-info.json'),catalog);
    await fs.copyFile(path.join(source,icons,'icon.icns'),path.join(work,'Creditia.icns'));
    const files = {};
    for (const name of payload) files[name] = await fileHash(path.join(work,name));
    await fs.writeFile(path.join(work,'manifest.json'),json({schema:1,candidate_sha:process.env.GITHUB_SHA ?? null,macos,xcode,sources:sourceFiles,files}));
    // The macOS 15 packagers only need to verify and copy these exact bytes.
    // Inspect the assembled resources on a host that understands Tahoe catalogs.
    const check = path.join(tmp,'Icon Verification.app');
    await fs.mkdir(path.join(check,'Contents'),{recursive:true});
    await writePlist(path.join(check,'Contents/Info.plist'),{});
    await installMacIcons({app:check,assets:work,source});
    execFileSync(process.execPath,[path.join(source,'apps/desktop/tools/verify-macos-icon.mjs'),check],{stdio:'inherit'});
    for (const name of [...payload,'manifest.json']) await fs.copyFile(path.join(work,name),path.join(output,name));
  } finally { await fs.rm(tmp,{recursive:true,force:true}); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [command, flag, output, ...rest] = process.argv.slice(2);
  if (command !== 'compile' || flag !== '--output' || !output || rest.length) {
    console.error('Usage: mac-icons.mjs compile --output <artifact-directory>');process.exitCode = 1;
  } else compileMacIcons({output:path.resolve(output)}).catch(error => {console.error(error.message);process.exitCode = 1;});
}
