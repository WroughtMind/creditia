import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Resvg } from '@resvg/resvg-js';
import { raster, fontFiles } from '../src/index.ts';
import { flattenSvg } from '../src/flatten.ts';

test('多处不同渐变的完整文字转路径后保持全部像素，包含祖先和文字自身变换', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="700" height="260"><defs><linearGradient id="gold" x2=".3" y2="1"><stop stop-color="#ffe588"/><stop offset="1" stop-color="#654321"/></linearGradient><linearGradient id="blue"><stop stop-color="blue"/><stop offset="1" stop-color="cyan"/></linearGradient></defs><g transform="translate(10 4)"><text x="25" y="90" font-family="Inter" font-size="70" fill="url(#gold)">CATNAP</text><text x="300" y="190" font-family="Inter" font-size="38" fill="url(#blue)" transform="rotate(-8 300 190)">Different title</text></g></svg>';
  const flat = raster.flat(svg);
  assert.doesNotMatch(flat, /url\(#\)|\bid=""|<text\b/);
  assert.deepEqual(raster.rgba(flat, 700), raster.rgba(svg, 700));
});

test('同一渐变的多个不同尺寸词组各保留自己的实际渐变坐标', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="600" height="240"><defs><radialGradient id="ink"><stop stop-color="#ff5533"/><stop offset="1" stop-color="#2233aa"/></radialGradient></defs><text x="30" y="90" font-family="Inter" font-size="62" fill="url(#ink)">Wide title</text><text x="300" y="180" font-family="Inter" font-size="22" fill="url(#ink)">Small</text></svg>';
  const flat = raster.flat(svg);
  assert.doesNotMatch(flat, /url\(#\)|\bid=""|<text\b/);
  assert.deepEqual(raster.rgba(flat, 600), raster.rgba(svg, 600));
});

test('普通文字转路径保持原有一次解析，不增加栅格化', () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><text x="10" y="50" font-family="Inter" font-size="30">Creditia</text></svg>';
  let calls = 0;
  const flat = flattenSvg(svg, source => {
    calls++;
    return new Resvg(source, { font: { fontFiles: fontFiles(), loadSystemFonts: false } }).toString();
  });
  assert.equal(calls, 1);
  assert.doesNotMatch(flat, /<text\b/);
});

test('渐变解析保留祖先字体继承和百分比位置，等同显式排版环境', () => {
  const wrap = (text: string) => `<svg xmlns="http://www.w3.org/2000/svg" width="500" height="150"><defs><linearGradient id="gold"><stop stop-color="#fff488"/><stop offset="1" stop-color="#604020"/></linearGradient></defs>${text}</svg>`;
  const inherited = wrap('<g font-family="Inter" font-size="64" font-weight="700"><text x="30" y="95" fill="url(#gold)">Creditia</text></g>');
  const explicit = wrap('<g><text x="30" y="95" font-family="Inter" font-size="64" font-weight="700" fill="url(#gold)">Creditia</text></g>');
  const percentage = wrap('<text x="10%" y="65%" font-family="Inter" font-size="64" fill="url(#gold)">Creditia</text>');
  const absolute = wrap('<text x="50" y="97.5" font-family="Inter" font-size="64" fill="url(#gold)">Creditia</text>');
  for (const [source, reference] of [[inherited, explicit], [percentage, absolute]]) {
    const flat = raster.flat(source);
    assert.doesNotMatch(flat, /url\(#\)|\bid=""/);
    assert.deepEqual(raster.rgba(flat, 500), raster.rgba(raster.flat(reference), 500));
  }
});
