import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { installMacIcons } from './mac-icons.mjs';

const partial = 'assetcatalog_generated_info.plist';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const doc = entries => `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>${Object.entries(entries).map(([k,v])=>`<key>${k}</key><string>${v}</string>`).join('')}</dict></plist>`;
const shaFile = async file => digest(await fs.readFile(file));
const appearances = ['NSAppearanceNameAqua','NSAppearanceNameDarkAqua','ISAppearanceTintable'];

// Explicit synthetic artifact; only the macOS 26 build exercises real actool,
// assetutil, and the editor-authored materials. These tests cover the handoff.
async function fixture(t) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(),'creditia-mac-icon-test-'));
  t.after(() => fs.rm(dir,{recursive:true,force:true}));
  const app = path.join(dir,'Creditia.app'), source = path.join(dir,'source'), assets = path.join(dir,'compiled');
  const icons = path.join(source,'apps/desktop/src-tauri/icons');
  await fs.mkdir(path.join(app,'Contents'),{recursive:true});
  await fs.mkdir(path.join(icons,'Creditia.icon/Assets'),{recursive:true});
  await fs.mkdir(assets);
  const plist = path.join(app,'Contents/Info.plist');
  await fs.writeFile(plist,doc({CFBundleExecutable:'creditia-gpui',CFBundleIdentifier:'dev.creditia.gpui',LSMinimumSystemVersion:'15.0'}));
  const sources = {'Creditia.icon/icon.json':'synthetic document','Creditia.icon/Assets/petal.svg':'synthetic petal','icon.icns':'synthetic branded fallback'};
  for (const [file,bytes] of Object.entries(sources)) await fs.writeFile(path.join(icons,file),bytes);
  const catalog = appearances.flatMap(Appearance=>[
    {AssetType:'IconImageStack',Name:'Layered',Appearance},
    {AssetType:'IconGroup',Name:'Layered/Five petals',Appearance,LayerCount:5,Layers:Array.from({length:5},()=>({LayerHasLightingEffects:true}))},
  ]);
  await fs.writeFile(path.join(assets,'Assets.car'),'synthetic compiled catalog');
  await fs.writeFile(path.join(assets,'Creditia.icns'),sources['icon.icns']);
  await fs.writeFile(path.join(assets,partial),doc({CFBundleIconName:'Layered',CFBundleIconFile:'compiler-generated',LSMinimumSystemVersion:'26.0',CFBundleIdentifier:'do.not.import'}));
  await fs.writeFile(path.join(assets,'assets-info.json'),JSON.stringify(catalog));
  const seal = async (candidate_sha=process.env.GITHUB_SHA??null) => {
    const files = {};
    for (const name of ['Assets.car','Creditia.icns',partial,'assets-info.json']) files[name] = await shaFile(path.join(assets,name));
    await fs.writeFile(path.join(assets,'manifest.json'),JSON.stringify({schema:1,candidate_sha,sources:Object.fromEntries(Object.entries(sources).map(([s,b])=>[s,digest(b)])),files}));
  };
  await seal();
  return {app,assets,source,icons,plist,catalog,seal};
}

function readPlist(file) {
  return JSON.parse(execFileSync(process.platform==='win32'?'python':'python3',['-c','import json, plistlib, sys; print(json.dumps(plistlib.loads(open(sys.argv[1], "rb").read())))',file],{encoding:'utf8'}));
}

test('staging keeps the catalog, original static icon and identity while merging the icon metadata', async t => {
  const f = await fixture(t);
  await installMacIcons(f);
  assert.deepEqual(readPlist(f.plist),{CFBundleExecutable:'creditia-gpui',CFBundleIdentifier:'dev.creditia.gpui',LSMinimumSystemVersion:'15.0',CFBundleIconName:'Layered',CFBundleIconFile:'Creditia.icns'});
  for (const name of ['Assets.car','Creditia.icns']) assert.equal(await shaFile(path.join(f.app,'Contents/Resources',name)), await shaFile(path.join(f.assets,name)));
  await fs.writeFile(path.join(f.assets,partial),doc({CFBundleIconFile:'compiler-generated'}));
  await f.seal();
  await installMacIcons(f);
  assert.equal(readPlist(f.plist).CFBundleIconName,'Layered');
});

test('staging rejects a different candidate, changed vector layer, missing asset or substituted fallback', async t => {
  const f = await fixture(t);
  await f.seal('another candidate');
  await assert.rejects(installMacIcons(f),/another candidate/);
  await f.seal();
  const vector = path.join(f.icons,'Creditia.icon/Assets/petal.svg');
  await fs.writeFile(vector,'changed');
  await assert.rejects(installMacIcons(f),/Icon Composer source/);
  await fs.writeFile(vector,'synthetic petal');
  await fs.writeFile(path.join(f.assets,'Assets.car'),'different catalog');
  await assert.rejects(installMacIcons(f),/checksum mismatch: Assets.car/);
  await fs.rm(path.join(f.assets,'Assets.car'));
  await assert.rejects(installMacIcons(f),/ENOENT/);
  await fs.writeFile(path.join(f.assets,'Assets.car'),'synthetic compiled catalog');
  await fs.writeFile(path.join(f.assets,'Creditia.icns'),'not the original fallback');
  await f.seal();
  await assert.rejects(installMacIcons(f),/original Creditia.icns/);
});

test('a mismatched primary name or a missing tinted material fails before changing the staged app', async t => {
  const f = await fixture(t);
  await fs.writeFile(path.join(f.assets,partial),doc({CFBundleIconName:'Unrelated'}));
  await f.seal();
  await assert.rejects(installMacIcons(f),/No matching primary icon stack/);
  await fs.writeFile(path.join(f.assets,partial),doc({CFBundleIconName:'Layered'}));
  f.catalog.at(-1).Layers[3].LayerHasLightingEffects = false;
  await fs.writeFile(path.join(f.assets,'assets-info.json'),JSON.stringify(f.catalog));
  await f.seal();
  await assert.rejects(installMacIcons(f),/ISAppearanceTintable/);
  assert.equal(readPlist(f.plist).CFBundleIconName,undefined);
  await assert.rejects(fs.access(path.join(f.app,'Contents/Resources/Assets.car')),/ENOENT/);
});
