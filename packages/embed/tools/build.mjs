import { build } from 'esbuild';
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, readdirSync, copyFileSync, writeFileSync, rmSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { createHash } from 'node:crypto';
import { FONTS } from '../../render/src/fonts.ts';
import { CLASSIC_FONTS } from '../../render/src/classic/fonts.ts';

const root = resolve(import.meta.dirname, '..');
const render = resolve(root, '../render');
const require = createRequire(join(render, 'package.json'));
const out = join(root, 'dist');
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'fonts'), { recursive: true });
await build({ entryPoints: [join(root, 'src/creditia-card.ts')], bundle: true,
  format: 'esm', platform: 'browser', target: 'es2022', minify: true,
  nodePaths: [join(render, 'node_modules')], outfile: join(out, 'creditia-card.js') });
copyFileSync(require.resolve('satori/layout.wasm'), join(out, 'layout.wasm'));
const specs = new Map([...CLASSIC_FONTS, ...FONTS].map(f => [`${f.family}|${f.weight}|${!!f.italic}`, f]));
const manifest = [];
const packages = new Set();
for (const face of specs.values()) {
  const pkg = dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`));
  const dir = join(pkg, face.dir);
  const file = readdirSync(dir).find(f => f.endsWith('.ttf') && (!!face.italic === f.includes('Italic')));
  if (!file) throw new Error(`Missing font ${face.family} ${face.weight}`);
  const data = readFileSync(join(dir, file));
  const hash = createHash('sha256').update(data).digest('hex');
  const name = `${hash.slice(0, 24)}.ttf`;
  writeFileSync(join(out, 'fonts', name), data);
  manifest.push({ family: face.family, weight: face.weight, italic: !!face.italic, file: name, sha256: hash });
  if (!packages.has(face.pkg)) {
    packages.add(face.pkg);
    for (const license of readdirSync(pkg).filter(f => /^(OFL|LICENSE|COPYING)/i.test(f))) {
      copyFileSync(join(pkg, license), join(out, 'fonts', `${face.pkg}-${license}`));
    }
  }
}
writeFileSync(join(out, 'fonts', 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`Built web component with ${manifest.length} pinned font faces (loaded on demand).`);
