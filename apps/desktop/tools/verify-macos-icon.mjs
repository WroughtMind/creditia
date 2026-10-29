// Inspect the actual bundle: an old Xcode can make Tauri skip .icon compilation.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { statSync } from 'node:fs';
import path from 'node:path';

const bundle = process.argv[2];
assert(bundle, 'Usage: node tools/verify-macos-icon.mjs <creditia.app>');
const contents = path.resolve(bundle, 'Contents');
const info = JSON.parse(execFileSync('plutil', ['-convert', 'json', '-o', '-', path.join(contents, 'Info.plist')], { encoding: 'utf8' }));
assert(info.CFBundleIconName, 'The bundle must name its compiled layered icon');
const assets = JSON.parse(execFileSync('assetutil', ['--info', path.join(contents, 'Resources/Assets.car')], { encoding: 'utf8' }));
for (const appearance of ['NSAppearanceNameAqua', 'NSAppearanceNameDarkAqua', 'ISAppearanceTintable']) {
  assert(assets.some(a => a.AssetType === 'IconImageStack' && a.Name === info.CFBundleIconName && a.Appearance === appearance), `Missing icon stack: ${appearance}`);
  const group = assets.find(a => a.AssetType === 'IconGroup' && a.Name === `${info.CFBundleIconName}/Five petals` && a.Appearance === appearance);
  assert.equal(group?.LayerCount, 5, `Missing five independent petals: ${appearance}`);
  assert(group.Layers.every(layer => layer.LayerHasLightingEffects), `Missing material effects: ${appearance}`);
}
assert(info.CFBundleIconFile, 'The bundle must also provide its static icon for older macOS');
const staticIcon = info.CFBundleIconFile.endsWith('.icns') ? info.CFBundleIconFile : `${info.CFBundleIconFile}.icns`;
assert(statSync(path.join(contents, 'Resources', staticIcon)).size > 0);
console.log('macOS icon: five layers, light/dark/tinted appearances and static icon verified');
