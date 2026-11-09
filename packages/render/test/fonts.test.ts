import { test } from 'node:test';
import assert from 'node:assert/strict';
import { referencedFonts, portableFontFamilies, familiesIn } from '../src/fonts.ts';

test('font selection includes actual fallback faces and inherited tspan weights without loading unrelated families', () => {
  const svg = '<svg><g font-family="Inter, Noto Sans SC" font-weight="700"><text>字<tspan font-family="Noto Serif SC" font-weight="500">中</tspan></text></g><text font-family="Courier Prime" font-style="italic">words</text></svg>';
  assert.deepEqual(referencedFonts(svg).map(f => [f.family, f.weight, !!f.italic]), [
    ['Inter', 700, false], ['Noto Sans SC', 700, false], ['Noto Serif SC', 500, false], ['Courier Prime', 400, true],
  ]);
  assert.deepEqual(referencedFonts('<svg><path d="M0 0"/></svg>'), []);
});

test('legacy badges and unspecified font families use the bundled native fallback', () => {
  const svg = '<svg><text font-family="Georgia, serif" font-weight="700">name</text><text>字</text></svg>';
  const portable = portableFontFamilies(svg);
  assert.ok(portable.includes('font-family="Noto Sans SC"'));
  assert.deepEqual(referencedFonts(svg).map(f => [f.family, f.weight]), [['Noto Sans SC', 700], ['Noto Sans SC', 400]]);
  assert.deepEqual(referencedFonts(portable), referencedFonts(svg));
  const pinned = '<svg><text font-family="Inter, Noto Sans SC">name</text></svg>';
  assert.equal(portableFontFamilies(pinned), pinned);
  assert.deepEqual(referencedFonts('<svg><text font-family="noto sans sc" font-weight="600">name</text></svg>').map(f => f.weight), [700]);
});


test('SVG entity-quoted families resolve the same bundled faces as CSS stacks', () => {
  assert.deepEqual(familiesIn('&quot;Press Start 2P&quot;, &#39;Noto Sans SC&#39;'), ['Press Start 2P', 'Noto Sans SC']);
  assert.deepEqual(referencedFonts('<svg><text font-family="&quot;Press Start 2P&quot;, &apos;Noto Sans SC&apos;">字</text></svg>').map(f => f.family), ['Press Start 2P', 'Noto Sans SC']);
});
