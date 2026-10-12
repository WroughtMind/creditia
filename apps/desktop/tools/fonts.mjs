// App 用的字体子集：清单来自 packages/render/src/classic/fonts.ts 和 packages/render/src/fonts.ts。拉丁字体留常用区段；
// 中日文字体留 GB2312 的 6,763 个汉字 + 假名 + 常用符号。产物在 src/fonts/（不入库，npm run fonts 生成）。
// 需要 python3 + fontTools（pip install fonttools）。
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, statSync, readdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { FONTS as V2 } from '../../../packages/render/src/fonts.ts';
import { CLASSIC_FONTS } from '../../../packages/render/src/classic/fonts.ts';

// 两套渲染器的字体合在一起，按 family + 字重 + 斜体去重
const FONTS = [...CLASSIC_FONTS];
for (const f of V2) if (!FONTS.some((x) => x.family === f.family && x.weight === f.weight && !!x.italic === !!f.italic)) FONTS.push(f);

const HERE = import.meta.dirname;
const RENDER = path.resolve(HERE, '../../../packages/render');
const require = createRequire(path.join(RENDER, 'package.json'));
const OUT = path.resolve(HERE, '../src/fonts');
mkdirSync(OUT, { recursive: true });
const LATIN = 'U+0020-007E,U+00A0-00FF,U+2000-206F,U+2190-21FF,U+2500-25FF,U+3000-303F,U+FF00-FFEF,U+30A0-30FF,U+3040-309F';
const dec = new TextDecoder('gb2312');
const chars = [];
for (let a = 0xb0; a <= 0xf7; a++) for (let b = 0xa1; b <= 0xfe; b++) { const c = dec.decode(new Uint8Array([a, b])); if (c && c !== '�') chars.push(c); }
const textFile = path.join(OUT, '.gb2312.txt');
writeFileSync(textFile, chars.join('') + '點〇');

const manifest = [];
for (const f of FONTS) {
  const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${f.pkg}/package.json`)), f.dir);
  const ttf = readdirSync(dir).find((x) => x.endsWith('.ttf') && (f.italic || !x.includes('Italic')));
  const name = `${f.family.replace(/\s+/g, '')}-${f.weight}${f.italic ? 'i' : ''}.ttf`;
  const out = path.join(OUT, name);
  if (!existsSync(out)) {
    const args = ['-m', 'fontTools.subset', path.join(dir, ttf), `--output-file=${out}`, `--unicodes=${LATIN}`, '--layout-features=*', '--no-hinting'];
    if (f.cjk) args.push(`--text-file=${textFile}`);
    execFileSync('python3', args, { stdio: 'inherit' });
  }
  manifest.push({ family: f.family, weight: f.weight, italic: !!f.italic, file: name });
  console.log(name.padEnd(34), `${(statSync(out).size / 1024).toFixed(0)} KB`);
}
writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 1));
