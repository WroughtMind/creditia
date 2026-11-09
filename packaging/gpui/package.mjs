#!/usr/bin/env node
// The installed desktop owns its Node runtime and fonts. Nothing is fetched by
// the installed application; this build tool copies an already resolved tree.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { installMacIcons } from './mac-icons.mjs';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const executable = process.platform === 'win32' ? '.exe' : '';
const safe = value => /^[a-zA-Z0-9_.-]+$/.test(value);

export async function files(root, prefix = '') {
  const out = [];
  for (const entry of (await fs.readdir(path.join(root, prefix), { withFileTypes: true })).sort((a,b) => a.name.localeCompare(b.name))) {
    const relative = path.posix.join(prefix, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Package must not contain symlinks: ${relative}`);
    if (entry.isDirectory()) out.push(...await files(root, relative));
    else if (entry.isFile()) out.push(relative);
  }
  return out;
}

function xml(value) { return value.replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c])); }
async function copyFile(from, to, mode) {
  await fs.mkdir(path.dirname(to), { recursive: true });
  await fs.copyFile(from, to);
  if (mode) await fs.chmod(to, mode);
}

export function layout(root, platform) {
  const mac = platform === 'darwin';
  const app = mac ? path.join(root, 'Creditia.app') : root;
  const bin = mac ? path.join(app, 'Contents', 'MacOS') : root;
  const resources = mac ? path.join(app, 'Contents', 'Resources') : path.join(root, 'resources');
  return { app, bin, resources, gui: path.join(bin, `creditia-gpui${platform === 'win32' ? '.exe' : ''}`), core: path.join(bin, `creditia-core${platform === 'win32' ? '.exe' : ''}`), node: path.join(resources, `node${platform === 'win32' ? '.exe' : ''}`), bridge: path.join(resources, 'render', 'tools', 'bridge.mjs') };
}

export async function stage({ target, output, appBin, coreBin, nodeBin = process.execPath, nodeLicense, nodeProvenance, migrationBin, macIcons, source = repo, platform = process.platform }) {
  if (!safe(target)) throw new Error('Invalid Rust target');
  if(platform==='darwin'&&!macIcons)throw new Error('Mac staging requires --mac-icons with the verified Xcode 26 Icon Composer artifact');
  const root = path.resolve(output, `creditia-gpui-${target}`);
  const paths = layout(root, platform);
  await fs.mkdir(root, { recursive: true });
  if ((await fs.readdir(root)).length) throw new Error(`Refusing to overwrite a nonempty package directory: ${root}`);
  const manifest = await fs.readFile(path.join(source, 'apps/gpui/Cargo.toml'), 'utf8');
  const version = /^version\s*=\s*"([^"]+)"/m.exec(manifest)?.[1];
  if (!version) throw new Error('GPUI version is missing');
  await copyFile(appBin, paths.gui, 0o755);
  await copyFile(coreBin, paths.core, 0o755);
  await copyFile(nodeBin, paths.node, 0o755);
  if (!nodeLicense) throw new Error('Pass --node-license with the license for the bundled Node version');
  await copyFile(nodeLicense, path.join(paths.resources, 'licenses/Node-LICENSE.txt'));
  if (nodeProvenance) await copyFile(nodeProvenance, path.join(paths.resources, 'licenses/Node-provenance.json'));
  if (migrationBin) {
    if (platform === 'darwin') {
      const migrationApp=path.join(paths.resources,'legacy/Creditia Migration.app');
      await copyFile(migrationBin,path.join(migrationApp,'Contents/MacOS/creditia'),0o755);
      await fs.writeFile(path.join(migrationApp,'Contents/Info.plist'),`<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>creditia</string><key>CFBundleIdentifier</key><string>dev.creditia.app</string><key>CFBundleName</key><string>Creditia Migration</string><key>CFBundleVersion</key><string>${xml(version)}</string><key>CFBundlePackageType</key><string>APPL</string><key>LSUIElement</key><true/><key>NSHighResolutionCapable</key><true/><key>NSPrincipalClass</key><string>NSApplication</string></dict></plist>\n`);
    } else await copyFile(migrationBin,path.join(paths.resources,`legacy/creditia-migrate${platform==='win32'?'.exe':''}`),0o755);
  }

  const render = path.join(source, 'packages/render');
  const destination = path.join(paths.resources, 'render');
  await fs.mkdir(destination, { recursive: true });
  for (const name of ['src', 'tools']) await fs.cp(path.join(render, name), path.join(destination, name), { recursive: true });
  for (const name of ['package.json', 'package-lock.json']) await copyFile(path.join(render, name), path.join(destination, name));
  // Shared imports intentionally retain the monorepo's render/schema sibling
  // relationship (for example src/tool-names.ts → ../../schema/agents.json).
  await fs.cp(path.join(source, 'packages/schema'), path.join(paths.resources, 'schema'), { recursive: true });
  // Dependencies are copied from npm ci on the target platform, including its
  // matching resvg native addon. Keep all runtime/license files; avoid test data.
  await fs.cp(path.join(render, 'node_modules'), path.join(destination, 'node_modules'), {
    recursive: true, dereference: true,
    filter: from => !from.split(path.sep).some(part => ['.bin', '.cache', '.git'].includes(part)),
  });
  // Google-font packages contain many unused weights. Retain exactly the faces
  // named by both renderer registries, plus package metadata and font licenses.
  const { FONTS } = await import(pathToFileURL(path.join(render, 'src/fonts.ts')));
  const { CLASSIC_FONTS } = await import(pathToFileURL(path.join(render, 'src/classic/fonts.ts')));
  const required = new Map();
  for (const font of [...FONTS, ...CLASSIC_FONTS]) {
    if (!required.has(font.pkg)) required.set(font.pkg, new Set());
    required.get(font.pkg).add(font.dir);
  }
  const fontsRoot = path.join(destination, 'node_modules/@expo-google-fonts');
  const fontFiles = [];
  const retainedFaces = new Set();
  for (const entry of await fs.readdir(fontsRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const pkgRoot = path.join(fontsRoot, entry.name), faces = required.get(entry.name);
    if (!faces) { await fs.rm(pkgRoot, { recursive: true }); continue; }
    for (const child of await fs.readdir(pkgRoot, { withFileTypes: true })) {
      if (child.isDirectory() && !faces.has(child.name)) await fs.rm(path.join(pkgRoot, child.name), { recursive: true });
      else if (child.isFile() && child.name!=='package.json' && !/^(license|ofl|fontlog|copying|notice)([._-]|$)/i.test(child.name)) await fs.rm(path.join(pkgRoot,child.name));
    }
    for (const face of faces) {
      const names = await fs.readdir(path.join(pkgRoot, face));
      const ttf = names.find(name => name.endsWith('.ttf'));
      if (!ttf) throw new Error(`Required font is missing: ${entry.name}/${face}`);
      for(const name of names)if(!/\.(ttf|otf)$/i.test(name)&&!/^(license|ofl|fontlog|copying|notice)([._-]|$)/i.test(name))await fs.rm(path.join(pkgRoot,face,name),{recursive:true,force:true});
      fontFiles.push(path.relative(root, path.join(pkgRoot, face, ttf)).split(path.sep).join('/'));
      retainedFaces.add(`${entry.name}/${face}`);
    }
  }
  for(const [pkg,faces] of required)for(const face of faces)if(!retainedFaces.has(`${pkg}/${face}`))throw new Error(`Required font package or face is missing: ${pkg}/${face}`);
  const licenseCandidates = ['LICENSE', 'LICENSE.md'];
  for (const name of licenseCandidates) {
    try { await copyFile(path.join(source, name), path.join(paths.resources, `licenses/Creditia-${name}`)); break; }
    catch (e) { if (e.code !== 'ENOENT') throw e; }
  }
  try { await copyFile(path.join(render,'assets/logos/SOURCES.md'),path.join(paths.resources,'licenses/Logo-SOURCES.md')); }
  catch (error) { if(error.code!=='ENOENT')throw error; }
  await copyFile(path.join(source, 'packaging/gpui/INSTALL.md'), path.join(root, 'INSTALL.md'));
  if (platform === 'darwin') {
    const plist = `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict><key>CFBundleExecutable</key><string>creditia-gpui</string><key>CFBundleIdentifier</key><string>dev.creditia.gpui</string><key>CFBundleName</key><string>Creditia</string><key>CFBundleDisplayName</key><string>Creditia</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${xml(version)}</string><key>CFBundleVersion</key><string>${xml(version)}</string><key>LSMinimumSystemVersion</key><string>15.0</string><key>NSHighResolutionCapable</key><true/><key>NSPrincipalClass</key><string>NSApplication</string></dict></plist>\n`;
    await fs.writeFile(path.join(paths.app, 'Contents/Info.plist'), plist);
    await fs.writeFile(path.join(paths.app, 'Contents/PkgInfo'), 'APPL????');
    await installMacIcons({app:paths.app,assets:macIcons,source});
  } else if (platform === 'linux') {
    await fs.writeFile(path.join(root, 'creditia.desktop'), '[Desktop Entry]\nType=Application\nName=Creditia\nComment=Your coding agents, in credits\nExec=creditia-gpui\nTerminal=false\nCategories=Development;Graphics;\n');
  }
  const nodeVersion = execFileSync(nodeBin, ['--version'], { encoding: 'utf8' }).trim();
  const receipt = { schema: 1, product: 'creditia-gpui', version, target, platform, nodeVersion, commit: process.env.GITHUB_SHA ?? null, sourceDateEpoch: process.env.SOURCE_DATE_EPOCH ?? null, legacyMigrationHelper: !!migrationBin, fontFiles, files: [] };
  await fs.writeFile(path.join(root, 'package-info.json'), JSON.stringify(receipt, null, 2) + '\n');
  return { root, ...paths };
}

export async function seal(root) {
  const info = JSON.parse(await fs.readFile(path.join(root, 'package-info.json'), 'utf8'));
  info.files = [];
  for(const p of (await files(root)).filter(p => p !== 'package-info.json')) {
    const bytes = await fs.readFile(path.join(root, p));
    info.files.push({ path: p, size: bytes.length, sha256: digest(bytes) });
  }
  await fs.writeFile(path.join(root, 'package-info.json'), JSON.stringify(info, null, 2) + '\n');
  return info;
}

export async function verify(root) {
  const info = JSON.parse(await fs.readFile(path.join(root, 'package-info.json'), 'utf8'));
  if (info.schema !== 1 || !info.files?.length) throw new Error('Package has not been sealed');
  const expected = new Set(info.files.map(x => x.path));
  for (const item of info.files) {
    if (path.posix.isAbsolute(item.path) || item.path.split('/').includes('..')) throw new Error('Unsafe manifest path');
    const bytes = await fs.readFile(path.join(root, item.path));
    if (bytes.length !== item.size || digest(bytes) !== item.sha256) throw new Error(`Package checksum mismatch: ${item.path}`);
  }
  for (const name of await files(root)) if (name !== 'package-info.json' && !expected.has(name)) throw new Error(`Unexpected package file: ${name}`);
  for (const name of info.fontFiles) if (!expected.has(name)) throw new Error(`Font is not in package manifest: ${name}`);
  return info;
}

export function bridgeRequest(node, bridge, request, cwd, timeout = 90_000) {
  return new Promise((resolve, reject) => {
    const env={...process.env};
    for(const key of ['NODE_PATH','NODE_OPTIONS','CREDITIA_CORE','CREDITIA_NODE','CREDITIA_RENDER_BRIDGE'])delete env[key];
    const child = spawn(node, [bridge], { cwd, windowsHide: true, env, stdio: ['pipe','pipe','pipe'] });
    let output = '', error = '', settled = false;
    const timer = setTimeout(() => { child.kill(); finish(new Error('Packaged renderer timed out')); }, timeout);
    function finish(failure, value) { if (settled) return; settled = true; clearTimeout(timer); failure ? reject(failure) : resolve(value); }
    child.on('error', finish);
    child.stdout.on('data', data => { output += data; if (output.length > 64 * 1024 * 1024) { child.kill(); finish(new Error('Oversize renderer output')); } });
    child.stderr.on('data', data => { error = (error + data).slice(-16_384); });
    child.stdin.on('error', finish);
    child.on('close', code => {
      if (code !== 0) return finish(new Error(`Packaged renderer exited ${code}: ${error}`));
      try { const response = JSON.parse(output.trim()); if (!response.ok || response.id !== request.id) throw new Error(JSON.stringify(response)); finish(null, response.result); }
      catch (e) { finish(e); }
    });
    child.stdin.end(JSON.stringify(request) + '\n');
  });
}

export async function smoke(root, fixture, output) {
  const info = await verify(root), paths = layout(root, info.platform);
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'creditia-packaged-'));
  const report = { schema_version: 1, kind: 'package', provenance:{candidate_sha: info.commit,environment:{platform:process.platform,arch:process.arch,os_release:os.release(),os_version:os.version()},fixture:{path:path.basename(fixture),sha256:digest(await fs.readFile(fixture)),kind:'synthetic_render_fixture'}}, platform:`${info.platform}-${process.arch}`,target:info.target,nodeVersion: info.nodeVersion, tests: [],acceptance:{passed:false},complete:false,artifacts:[] };
  const artifact=async(name,bytes)=>{await fs.mkdir(path.dirname(path.join(path.dirname(output),name)),{recursive:true});await fs.writeFile(path.join(path.dirname(output),name),bytes);report.artifacts.push({path:name,bytes:Buffer.byteLength(bytes),sha256:digest(bytes)});};
  try {
    const env = { ...process.env, CREDITIA_HOME: path.join(temporary, 'library') };
    for(const key of ['CREDITIA_CORE','CREDITIA_NODE','CREDITIA_RENDER_BRIDGE','NODE_PATH','NODE_OPTIONS'])delete env[key];
    // Run outside the source checkout to detect accidental development paths.
    await artifact('analysis-core-help.txt',execFileSync(paths.core, ['help'], { cwd: temporary, env, timeout: 30_000, stdio: 'pipe' }));
    report.tests.push({ name: 'bundled-analysis-core-launches', status: 'passed' });
    const catalog = await bridgeRequest(paths.node, paths.bridge, { version: 1, id: 'catalog', op: 'catalog' }, temporary);
    if (catalog.styles.length !== 26) throw new Error(`Expected 26 desktop styles, got ${catalog.styles.length}`);
    report.tests.push({ name: 'bundled-renderer-catalog', status: 'passed', styles: catalog.styles.length });
    const raw = JSON.parse(await fs.readFile(fixture, 'utf8'));
    const common = { version: 1, raw, style: 'minimal-light', piece: 'asterisk', lang: 'zh', group: 'auto', named: false };
    const image = await bridgeRequest(paths.node, paths.bridge, { ...common, id: 'preview', op: 'render', flatten: true }, temporary);
    if (!image.art?.svg?.includes('<svg')) throw new Error('Packaged preview is missing');
    await artifact('preview.svg',image.art.svg);
    report.tests.push({ name: 'bundled-fonts-and-preview', status: 'passed' });
    for (const format of ['png','svg','pdf','gif','html']) {
      const result = await bridgeRequest(paths.node, paths.bridge, { ...common, id: format, op: 'export', format, size: 'native', frameCount: 2 }, temporary);
      const bytes = Buffer.from(result.bytes, 'base64');
      if (!bytes.length || result.ext !== format) throw new Error(`Invalid ${format} export`);
      await artifact(`exports/asterisk.${format}`,bytes);
      report.tests.push({ name: `bundled-export-${format}`, status: 'passed', bytes: bytes.length, sha256: digest(bytes) });
    }
    await artifact('native-resource-check.txt',execFileSync(paths.gui, ['--check-resources'], { cwd: temporary, env, timeout: 90_000, stdio: 'pipe' }));
    await artifact('package-info.json',await fs.readFile(path.join(root,'package-info.json')));
    report.tests.push({ name: 'native-executable-resource-discovery', status: 'passed' });
    if(info.legacyMigrationHelper){
      const helper=info.platform==='darwin'?path.join(paths.resources,'legacy/Creditia Migration.app/Contents/MacOS/creditia'):path.join(paths.resources,`legacy/creditia-migrate${info.platform==='win32'?'.exe':''}`);
      const command=info.platform==='linux'?'xvfb-run':helper;
      const args=info.platform==='linux'?['-a','dbus-run-session','--',helper,'--migrate-ui-settings']:['--migrate-ui-settings'];
      await artifact('legacy-helper-startup.txt',execFileSync(command,args,{cwd:temporary,env,timeout:65_000,stdio:'pipe'}));
      const migrated=await fs.readFile(path.join(env.CREDITIA_HOME,'app.json'));
      if(JSON.parse(migrated).ui?.legacyImported!==true)throw new Error('Legacy helper did not complete preference import');
      await artifact('legacy-helper-imported-settings.json',migrated);
      report.tests.push({name:'bundled-legacy-helper-embedded-page-and-import',status:'passed'});
    }
    report.status = 'passed';
    report.acceptance.passed=true;report.complete=true;
  } catch (error) { report.status = 'failed'; report.error = error.message; throw error; }
  finally {
    await fs.rm(temporary, { recursive: true, force: true });
    await fs.mkdir(path.dirname(output), { recursive: true });
    await fs.writeFile(output, JSON.stringify(report, null, 2) + '\n');
  }
  return report;
}

async function main(args) {
  const command = args.shift(), opts = {};
  for (let i = 0; i < args.length; i += 2) { if (!args[i].startsWith('--') || args[i + 1] === undefined) throw new Error('Expected --key value'); opts[args[i].slice(2)] = args[i + 1]; }
  if (command === 'stage') {
    const target = opts.target;
    if (!target || !opts.output) throw new Error('stage requires --target and --output');
    const result = await stage({ target, output: opts.output, appBin: opts['app-bin'] ?? path.join(repo, `apps/gpui/target/${target}/release/creditia-gpui${executable}`), coreBin: opts['core-bin'] ?? path.join(repo, `target/${target}/release/creditia-core${executable}`), nodeBin: opts['node-bin'], nodeLicense: opts['node-license'], nodeProvenance: opts['node-provenance'],migrationBin:opts['migration-bin'],macIcons:opts['mac-icons'] });
    console.log(JSON.stringify(result));
  } else if (command === 'seal') console.log(JSON.stringify(await seal(path.resolve(opts.root))));
  else if (command === 'verify') { const r = await verify(path.resolve(opts.root)); console.log(`verified ${r.files.length} files for ${r.target}`); }
  else if (command === 'smoke') console.log(JSON.stringify(await smoke(path.resolve(opts.root), path.resolve(opts.fixture), path.resolve(opts.output))));
  else throw new Error('Usage: package.mjs stage|seal|verify|smoke --key value');
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main(process.argv.slice(2)).catch(error => { console.error(error.message); process.exitCode = 1; });
