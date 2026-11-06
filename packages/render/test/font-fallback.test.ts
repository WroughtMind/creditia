import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Resvg } from '@resvg/resvg-js';
import { fontFiles, raster } from '../src/index.ts';
import { portableFontFamilies } from '../src/fonts.ts';

const fullDatabase = (svg: string, width: number) => new Resvg(svg, {
  font: { fontFiles: fontFiles(), loadSystemFonts: false, defaultFontFamily: 'Noto Sans SC' },
  fitTo: { mode: 'width', value: width }, shapeRendering: 2, textRendering: 1, imageRendering: 0,
}).render().asPng();

test('selective loading preserves whole-run glyph fallback and original database order at native and double scale', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="520" height="90"><text x="10" y="25" font-family="Press Start 2P, Noto Sans SC" font-size="16">catnap</text><text x="10" y="55" font-family="ZCOOL QingKe HuangYou, Noto Sans SC" font-size="14">734 commits · 50,190 lines · 141 days</text><text x="10" y="80" font-family="ZCOOL QingKe HuangYou, Noto Sans SC" font-size="14">中文 · credits</text></svg>';
  for (const width of [320, 520, 1040]) assert.deepEqual(Buffer.from(raster.png(svg, width)), fullDatabase(svg, width), `width ${width}`);
});

test('portable legacy terminal prompt keeps the original chevron glyph and whole-run spacing', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="240" height="24"><text x="12" y="17" font-family="Menlo, Consolas, monospace" font-size="11">❯ creditia  </text><text x="90" y="17" font-family="Menlo, Consolas, monospace" font-size="11">中文 32% · GPT 29%</text></svg>';
  const portable = portableFontFamilies(svg);
  for (const width of [240, 480]) assert.deepEqual(Buffer.from(raster.png(portable, width)), fullDatabase(svg, width), `width ${width}`);
});
