import fs from 'node:fs/promises';
import path from 'node:path';
import { build } from 'esbuild';
import { copyRuntimeFonts } from '../../render/tools/font-assets.mjs';
import manifest from '../package.json' with { type: 'json' };

const cli = path.resolve(import.meta.dirname, '..');
const render = path.resolve(cli, '../render');
const dist = path.join(cli, 'dist');
const staging = await fs.mkdtemp(path.join(cli, '.dist-'));
try {
  // Bundle the used weights of the largest font families from the renderer's
  // pinned npm install. Remaining families resolve as regular dependencies.
  await copyRuntimeFonts(render, path.join(staging, 'fonts'), pkg => !manifest.dependencies[`@expo-google-fonts/${pkg}`]);
  await build({
    absWorkingDir: cli,
    entryPoints: ['src/cli.ts'],
    bundle: true,
    splitting: true,
    platform: 'node',
    format: 'esm',
    target: 'node22',
    outdir: staging,
    outExtension: { '.js': '.mjs' },
    packages: 'external',
    define: { CREDITIA_BUNDLED_FONTS: JSON.stringify('fonts') },
    banner: { js: "import { createRequire as __cr } from 'node:module'; const require = __cr(import.meta.url);" },
  });
  await fs.rm(dist, { recursive: true, force: true });
  await fs.rename(staging, dist);
} finally {
  await fs.rm(staging, { recursive: true, force: true });
}
