import { readFileSync, writeFileSync, mkdirSync, createWriteStream } from 'node:fs';
import { once } from 'node:events';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { renderRequest } from '../../render/src/native-protocol.ts';
import { parseMatrix } from '../../../packaging/gpui-acceptance/common.mjs';
import { contexts, preferences } from '../../../packaging/gpui-acceptance/fixtures.mjs';
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
for (const style of catalog.result.styles.filter(style => ['minimal-light','ink'].includes(style.id))) {
  for (const lang of ['zh','en']) {
    const request = { version:1,id:`${style.id}-${lang}-named-tool`,op:'render',style:style.id,
      renderer:style.renderer,piece:'tier',lang,group:'tool',named:true,sign:'Creditia · 共同创作',raw };
    const result = await renderRequest(request);
    if (!result.ok || !result.result.art) throw new Error(`Preference case failed: ${request.id}`);
    const { raw: _, ...selection } = request;
    cases.push({ selection, art: result.result.art });
  }
}
for (const selection of [
  {renderer:'classic',style:'minimal-light',piece:'tier',variant:'original'},
  {renderer:'classic',style:'minimal-light',piece:'receipt',variant:'themed'},
  {renderer:'classic',style:'minimal-light',piece:'roll',variant:'scroll'},
  {renderer:'svg',style:'ink',piece:'asterisk'},
]) {
  const request = {...selection,version:1,id:`transparent-${selection.piece}`,op:'render',lang:'zh',group:'model',named:false,transparent:true,raw};
  const result = await renderRequest(request);
  if (!result.ok || !result.result.art) throw new Error(`Transparent browser fixture: ${JSON.stringify(result)}`);
  const {raw:_,...options} = request;
  cases.push({selection:options,art:result.result.art});
}
mkdirSync(resolve(root, 'dist/acceptance'), { recursive: true });
writeFileSync(resolve(root, 'dist/acceptance/fixture.json'), input);
writeFileSync(resolve(root, 'dist/acceptance/cases.json'), JSON.stringify({
  fixtureKind: 'synthetic_render_sample', fixtureHash: createHash('sha256').update(input).digest('hex'), cases,
}));
console.log(`Prepared ${cases.length} native/web parity cases.`);

if (process.argv.includes('--full')) {
  const context = contexts(raw);
  writeFileSync(resolve(root, 'dist/acceptance/source.json'), JSON.stringify({ raw, ...context }));
  const rows = parseMatrix(readFileSync(resolve(root, '../../docs/gpui-migration/matrix.tsv'), 'utf8'));
  const stream = createWriteStream(resolve(root, 'dist/acceptance/catalog.jsonl'));
  const digest = createHash('sha256');
  for (const [index, row] of rows.entries()) {
    const preference = preferences[index % preferences.length];
    const request = { version:1, id:[row.renderer,row.style,row.piece,row.variant,preference.id].join('-'),
      op:'render', renderer:row.renderer, style:row.style, piece:row.piece,
      ...(row.variant !== 'default' ? { variant:row.variant } : {}),
      lang:preference.lang, group:preference.group, named:preference.named, raw, ...context };
    const result = await renderRequest(request);
    if (!result.ok) throw new Error(`Catalog render failed: ${request.id}: ${result.error.message}`);
    const { raw: _, projects: __, blame: ___, person: ____, ...selection } = request;
    const line = JSON.stringify({ selection, art:result.result.art, context:true }) + '\n';
    digest.update(line);
    if (!stream.write(line)) await once(stream,'drain');
  }
  stream.end(); await once(stream,'finish');
  writeFileSync(resolve(root,'dist/acceptance/catalog-meta.json'),JSON.stringify({ rows:rows.length,
    sha256:digest.digest('hex'), sourceSha256:createHash('sha256').update(readFileSync(resolve(root,'dist/acceptance/source.json'))).digest('hex') }));
  console.log(`Prepared ${rows.length} complete catalog cases with projects and evidence context.`);
}
