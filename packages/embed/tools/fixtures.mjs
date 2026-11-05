import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { renderRequest } from '../../render/src/native-protocol.ts';
const root = resolve(import.meta.dirname, '..');
const input = readFileSync(resolve(root, '../render/fixtures/catnap.json'));
const raw = JSON.parse(input);
const catalog = await renderRequest({ version: 1, id: 'catalog', op: 'catalog' });
if (!catalog.ok) throw new Error(catalog.error.message);
const cases = [];
for (const style of catalog.result.styles) {
  for (const lang of ['zh', 'en']) {
    const request = { version: 1, id: `${style.id}-${lang}`, op: 'render', style: style.id,
      renderer: style.renderer, piece: 'tier', lang, group: 'model', named: false, sign: '', raw };
    const result = await renderRequest(request);
    if (!result.ok || !result.result.art) throw new Error(`${style.id}: ${JSON.stringify(result)}`);
    const { raw: _, ...selection } = request;
    cases.push({ selection, art: result.result.art });
  }
}
mkdirSync(resolve(root, 'dist/acceptance'), { recursive: true });
writeFileSync(resolve(root, 'dist/acceptance/fixture.json'), input);
writeFileSync(resolve(root, 'dist/acceptance/cases.json'), JSON.stringify({
  fixtureKind: 'synthetic_render_sample', fixtureHash: createHash('sha256').update(input).digest('hex'), cases,
}));
console.log(`Prepared ${cases.length} native/web parity cases.`);
