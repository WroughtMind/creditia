import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fontKey, fontHasGlyph, portableTextFonts, portableFontBindings } from '../src/portable-text.ts';
import { nativeFonts, matchingFace } from '../src/fonts.ts';
import { raster } from '../src/index.ts';
const require = createRequire(import.meta.url);
const cache = new Map<string, Uint8Array>();
function load(svg: string) {
  for (const face of nativeFonts(svg)) {
    const key = fontKey(face);
    if (cache.has(key)) continue;
    const dir = path.join(path.dirname(require.resolve(`@expo-google-fonts/${face.pkg}/package.json`)), face.dir);
    cache.set(key, readFileSync(path.join(dir, readdirSync(dir).find(f => f.endsWith('.ttf'))!)));
  }
  return cache;
}
const cases = [
  '<text x="10" y="30" font-family="ZCOOL QingKe HuangYou, Noto Sans SC" font-size="18" font-weight="700">734 commits · 50,190 lines · 141 days</text>',
  '<text x="10" y="30" font-family="ZCOOL QingKe HuangYou, Noto Sans SC" font-size="18">中文 · credits</text>',
  '<text x="10" y="30" font-family="Press Start 2P, Noto Sans SC" font-size="18" font-weight="700">catnap</text>',
  '<text x="10" y="30" font-family="Menlo, Consolas, monospace" font-size="18">❯ creditia  </text>',
  '<text x="240" y="30" text-anchor="middle" font-family="Inter, Noto Sans SC" font-size="18"><tspan x="180">creditia ·</tspan><tspan x="300" font-family="Noto Sans SC">中文项目</tspan></text>',
];
test('portable text uses exact native fallback, actual font weight and inherited tspan faces', () => {
  for (const body of cases) {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="600" height="60">${body}</svg>`;
    const portable = portableTextFonts(svg, load(svg));
    for (const width of [600, 1200]) assert.equal(Buffer.compare(raster.png(portable, width), raster.png(svg, width)), 0, `${body} @ ${width}`);
  }
});
test('pinned Unicode cmaps identify missing symbols and CJK glyphs without platform fonts', () => {
  const svg = '<svg><text font-family="ZCOOL QingKe HuangYou, Noto Sans SC">中 · x</text></svg>';
  const bytes = load(svg);
  const zcool = fontHasGlyph(bytes.get(fontKey(matchingFace('ZCOOL QingKe HuangYou', 400, false)!))!);
  assert.equal(zcool('中'.codePointAt(0)!), true);
  assert.equal(zcool('x'.codePointAt(0)!), true);
  assert.equal(zcool('·'.codePointAt(0)!), false);
  const mono = fontHasGlyph(bytes.get(fontKey(matchingFace('JetBrains Mono', 400, false)!))!);
  assert.equal(mono('❯'.codePointAt(0)!), true);
});

test('portable family stacks quote numeric names and bind fallback aliases to their actual face', () => {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="600" height="60">${cases[1]}${cases[2]}</svg>`;
  const portable = portableTextFonts(svg, load(svg));
  assert.ok(portable.includes('font-family="&apos;Press Start 2P&apos;"'));
  assert.ok(portable.includes('&apos;Creditia inter 400Regular&apos;'));
  assert.ok(portable.includes('font-synthesis:none'));
  const alias = portableFontBindings(portable).find(binding => binding.family === 'Creditia inter 400Regular');
  assert.equal(alias?.face.family, 'Inter');
  assert.equal(alias?.face.weight, 400);
});
