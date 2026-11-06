import test from 'node:test';
import assert from 'node:assert/strict';
import { portableBadgeReference, terminalRasterReference } from './baseline-equivalence.mjs';
test('font equivalence only changes reviewed badge stacks and preserves prompt fallback', () => {
  const svg = `<text font-family="Menlo, Consolas, monospace">❯ creditia</text><text font-family="Menlo, Consolas, monospace">name</text><text font-family="Other">name</text>`;
  const result = portableBadgeReference(svg, { renderer: 'classic', piece: 'badge' });
  assert.equal(result.changes, 2);
  assert.equal(result.svg, `<text font-family="JetBrains Mono">❯ creditia</text><text font-family="Noto Sans SC">name</text><text font-family="Other">name</text>`);
  assert.equal(portableBadgeReference(svg, { renderer: 'svg', piece: 'badge' }).svg, svg);
});
test('terminal accommodation removes only off-canvas code rain', () => {
  const text = (y, fill = '#7CFF9E') => `<text y="${y}" font-family="DotGothic16, JetBrains Mono" font-size="16" fill="${fill}" opacity="0.1">A</text>`;
  const row = { renderer: 'svg', style: 'terminal' }, svg = text(140) + text(141) + text(160, '#fff');
  assert.deepEqual(terminalRasterReference(svg, row, 120), { svg: text(140) + text(160, '#fff'), changes: 1 });
  assert.equal(terminalRasterReference(svg, { ...row, style: 'ink' }, 120).svg, svg);
});
