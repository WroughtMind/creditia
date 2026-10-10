// App 预览用的字体子集：拉丁字体只留常用区段，中文字体留 GB2312 的 6,763 个汉字。CLI 仍用完整字体。
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const HERE = path.dirname(new URL(import.meta.url).pathname);
const require = createRequire(path.resolve(HERE, '../../../packages/render/package.json'));
const OUT = path.resolve(HERE, '../src/fonts');
mkdirSync(OUT, { recursive: true });
const LATIN = 'U+0020-007E,U+00A0-00FF,U+2000-206F,U+2190-21FF,U+2500-25FF,U+3000-303F,U+FF00-FFEF';
// GB2312 一、二级汉字：解码全部码位得到字符集
const dec = new TextDecoder('gb2312');
const chars = [];
for (let a = 0xb0; a <= 0xf7; a++) for (let b = 0xa1; b <= 0xfe; b++) { const c = dec.decode(new Uint8Array([a, b])); if (c && c !== '�') chars.push(c); }
const textFile = path.join(OUT, '.gb2312.txt');
writeFileSync(textFile, chars.join(''));
const FILES = [
  ['inter/400Regular/Inter_400Regular.ttf', 'Inter-400.ttf'],
  ['inter/500Medium/Inter_500Medium.ttf', 'Inter-500.ttf'],
  ['inter/600SemiBold/Inter_600SemiBold.ttf', 'Inter-600.ttf'],
  ['inter/700Bold/Inter_700Bold.ttf', 'Inter-700.ttf'],
  ['jetbrains-mono/400Regular/JetBrainsMono_400Regular.ttf', 'JetBrainsMono-400.ttf'],
  ['jetbrains-mono/700Bold/JetBrainsMono_700Bold.ttf', 'JetBrainsMono-700.ttf'],
  ['instrument-serif/400Regular/InstrumentSerif_400Regular.ttf', 'InstrumentSerif-400.ttf'],
  ['newsreader/400Regular/Newsreader_400Regular.ttf', 'Newsreader-400.ttf'],
  ['newsreader/500Medium/Newsreader_500Medium.ttf', 'Newsreader-500.ttf'],
  ['noto-sans-sc/400Regular/NotoSansSC_400Regular.ttf', 'NotoSansSC-400.ttf', true],
  ['noto-sans-sc/700Bold/NotoSansSC_700Bold.ttf', 'NotoSansSC-700.ttf', true],
  ['noto-serif-sc/400Regular/NotoSerifSC_400Regular.ttf', 'NotoSerifSC-400.ttf', true],
];
for (const [src, name, cjk] of FILES) {
  const input = require.resolve(`@expo-google-fonts/${src}`);
  const out = path.join(OUT, name);
  const args = ['-m', 'fontTools.subset', input, `--output-file=${out}`, `--unicodes=${LATIN}`, '--layout-features=*', '--no-hinting'];
  if (cjk) args.push(`--text-file=${textFile}`);
  execFileSync('python3', args, { stdio: 'inherit' });
  console.log(name.padEnd(24), `${(statSync(out).size / 1024).toFixed(0)} KB`);
}
