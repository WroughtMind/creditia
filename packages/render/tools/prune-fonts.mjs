// This private package owns its local npm tree. Leave hoisted or linked
// installs alone because another package may use those font faces.
import fs from 'node:fs/promises';
import path from 'node:path';
import { runtimeFontPackages } from './font-assets.mjs';

const render = path.resolve(import.meta.dirname, '..');
const root = path.join(render, 'node_modules/@expo-google-fonts');
if (await fs.realpath(root).catch(error => {
  if (error.code !== 'ENOENT') throw error;
}) === root) {
  const required = await runtimeFontPackages(render);
  // The landing page and repository video previews additionally load this face.
  required.get('jetbrains-mono').add('500Medium');
  for (const pkg of await fs.readdir(root, { withFileTypes: true })) {
    const retained = required.get(pkg.name);
    if (!retained || !pkg.isDirectory()) continue;
    const dir = path.join(root, pkg.name);
    for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      if (entry.isDirectory() && /^[1-9]00[A-Za-z]+(?:_Italic)?$/.test(entry.name)) {
        if (!retained.has(entry.name)) await fs.rm(file, { recursive: true, force: true });
        else {
          for (const preview of await fs.readdir(file, { withFileTypes: true })) {
            if (preview.isFile() && preview.name.endsWith('.png')) await fs.rm(path.join(file, preview.name), { force: true });
          }
        }
      } else if (entry.isFile() && entry.name === 'font-family.png') await fs.rm(file, { force: true });
    }
  }
}
