import { test } from 'node:test';
import assert from 'node:assert/strict';
import { referencedFonts } from '../src/fonts.ts';

test('font selection includes actual fallback faces and inherited tspan weights without loading unrelated families', () => {
  const svg = '<svg><g font-family="Inter, Noto Sans SC" font-weight="700"><text>字<tspan font-family="Noto Serif SC" font-weight="500">中</tspan></text></g><text font-family="Courier Prime" font-style="italic">words</text></svg>';
  assert.deepEqual(referencedFonts(svg).map(f => [f.family, f.weight, !!f.italic]), [
    ['Inter', 700, false], ['Noto Sans SC', 700, false], ['Noto Serif SC', 500, false], ['Courier Prime', 400, true],
  ]);
  assert.deepEqual(referencedFonts('<svg><path d="M0 0"/></svg>'), []);
});
