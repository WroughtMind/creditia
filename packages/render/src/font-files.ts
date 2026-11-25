// Node-only face resolution. The shared manifest also runs in browsers.
import { existsSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import type { FontFace } from './fonts.ts';

// The CLI build supplies the location of its bundled font packages.
declare const CREDITIA_BUNDLED_FONTS: string;
const require = createRequire(import.meta.url);
const paths = new Map<string, string>();

export function fontPath(face: FontFace): string {
  const key = `${face.pkg}/${face.dir}`;
  const cached = paths.get(key);
  if (cached) return cached;
  const bundled = typeof CREDITIA_BUNDLED_FONTS === 'string'
    ? path.join(import.meta.dirname, CREDITIA_BUNDLED_FONTS, face.pkg) : undefined;
  const root = bundled && existsSync(bundled) ? bundled
    : path.dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`));
  const dir = path.join(root, face.dir);
  // Match the installed filename (case-sensitive filesystems included).
  const names = readdirSync(dir);
  const name = names.find(name => name.endsWith('.ttf')) ?? names.find(name => name.endsWith('.otf'));
  if (!name) throw new Error(`Missing font: ${face.family} ${face.weight} (${key})`);
  const file = path.join(dir, name);
  paths.set(key, file);
  return file;
}
