import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const notice = /^(license|ofl|fontlog|copying|notice)([._-]|$)/i;
const faceFile = /\.(ttf|otf)$/i;

export async function runtimeFontPackages(render) {
  const [{ FONTS }, { CLASSIC_FONTS }] = await Promise.all([
    import(pathToFileURL(path.join(render, 'src/fonts.ts'))),
    import(pathToFileURL(path.join(render, 'src/classic/fonts.ts'))),
  ]);
  const packages = new Map();
  for (const { pkg, dir } of [...FONTS, ...CLASSIC_FONTS]) {
    if (!packages.has(pkg)) packages.set(pkg, new Set());
    packages.get(pkg).add(dir);
  }
  return packages;
}

// Copy whole faces and their licenses. Dynamic project/author names and Git
// text can use any glyph in these fonts, even when the UI language is English.
export async function copyRuntimeFonts(render, destination, include = () => true) {
  const source = path.join(render, 'node_modules/@expo-google-fonts');
  const packages = await runtimeFontPackages(render);
  const fontFiles = [];
  for (const pkg of [...packages.keys()].sort()) {
    if (!include(pkg)) continue;
    const pkgSource = path.join(source, pkg);
    const pkgDest = path.join(destination, pkg);
    const names = await fs.readdir(pkgSource).catch(error => {
      if (error.code !== 'ENOENT') throw error;
      throw new Error(`Required font package or face is missing: ${pkg}`, { cause: error });
    });
    await fs.mkdir(pkgDest, { recursive: true });
    for (const name of names.filter(name => name === 'package.json' || notice.test(name))) {
      await fs.copyFile(path.join(pkgSource, name), path.join(pkgDest, name));
    }
    for (const dir of packages.get(pkg)) {
      const faceSource = path.join(pkgSource, dir);
      const faceDest = path.join(pkgDest, dir);
      const files = await fs.readdir(faceSource).catch(error => {
        if (error.code !== 'ENOENT') throw error;
        throw new Error(`Required font package or face is missing: ${pkg}/${dir}`, { cause: error });
      });
      const ttf = files.find(name => name.endsWith('.ttf'));
      if (!ttf) throw new Error(`Required font is missing: ${pkg}/${dir}`);
      await fs.mkdir(faceDest, { recursive: true });
      for (const name of files.filter(name => faceFile.test(name) || notice.test(name))) {
        await fs.copyFile(path.join(faceSource, name), path.join(faceDest, name));
      }
      fontFiles.push(path.join(faceDest, ttf));
    }
  }
  return fontFiles;
}
