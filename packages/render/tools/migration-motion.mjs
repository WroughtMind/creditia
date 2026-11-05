#!/usr/bin/env node
// Reproducible material-motion evidence from the production renderer.
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { nativeBackend } from '../src/native-protocol.ts';
import { createRenderSession } from '../src/protocol.ts';
import { motionProfile } from '../src/motion-profiles.ts';
import { raster } from '../src/index.ts';
const out = path.resolve(process.argv[2] ?? 'motion-evidence');
mkdirSync(out, { recursive: true });
const fixture = readFileSync(new URL('../fixtures/catnap.json', import.meta.url));
const raw = JSON.parse(fixture);
const sourceRoot = path.resolve(import.meta.dirname, '../src');
const sourceFiles = directory => readdirSync(directory, { withFileTypes: true }).flatMap(entry => entry.isDirectory()
  ? sourceFiles(path.join(directory, entry.name)) : [path.join(directory, entry.name)]);
const sourceHash = createHash('sha256');
for (const file of sourceFiles(sourceRoot).sort()) sourceHash.update(path.relative(sourceRoot, file) + '\0').update(readFileSync(file));
const rendererSourceSha256 = sourceHash.digest('hex');
const cases = [
  ['minimal-light', 'tier', 'original'], ['minimal-dark', 'asterisk'], ['glass', 'donut', 'original'],
  ['film', 'roll', 'crawl'], ['cinema', 'roll', 'scroll'], ['editorial', 'receipt', 'folio'],
  ['broadsheet', 'tier', 'original'], ['playbill', 'roll', 'program'], ['gala', 'asterisk'],
  ['mono', 'waffle', 'original'], ['poster', 'mosaic'], ['terminal', 'fetch'],
  ['crt', 'donut', 'original'], ['neon', 'asterisk'], ['arcade', 'waffle', 'original'],
  ['ink', 'asterisk'], ['ukiyoe', 'roll', 'vertical'], ['woodblock', 'mosaic'],
  ['baroque', 'roll', 'billing'], ['grimoire', 'tier', 'rows'], ['manuscript', 'roll', 'vertical'],
  ['pastel', 'share'], ['cutout', 'waffle', 'grid'], ['meadow', 'asterisk'], ['manga', 'codemap'], ['cyber', 'tier', 'hud'],
  ['ink', 'tier', 'grouped'], ['glass', 'asterisk'], ['film', 'asterisk'], ['terminal', 'asterisk'],
];
const times = [0, .2, .45, .7, 1], width = 220, gap = 12, rowH = 300, rowsPerPage = 8;
const pages = [], records = [];
const W = gap + times.length * (width + gap), contacts = [];
const savePage = page => {
  const H = Math.min(rowsPerPage, cases.length - page * rowsPerPage) * rowH;
  const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="#efede8"/>${pages[page].join('')}</svg>`;
  const filename = page === 0 ? 'material-motion-contact.png' : `material-motion-contact-${page + 1}.png`;
  writeFileSync(path.join(out, filename), raster.png(sheet, W)); contacts[page] = filename;
};
const decode = response => { if (!response.ok) throw new Error(JSON.stringify(response.error)); return response.result; };
const session = createRenderSession(nativeBackend);
const renderRequest = input => session.dispatch(input);
decode(await renderRequest({ version: 1, id: 'fixture', op: 'archive', archiveId: 'motion-fixture', raw }));
for (let row = 0; row < cases.length; row++) {
  const [style, piece, variant] = cases[row];
  const stem = `${style}-${piece}${variant ? '-' + variant : ''}`;
  const base = { version: 1, id: `${style}-${piece}`, archiveId: 'motion-fixture', style, piece, lang: 'zh', group: 'model', named: false, ...(variant ? { variant } : {}) };
  const page = Math.floor(row / rowsPerPage), y = row % rowsPerPage;
  const tiles = pages[page] ??= [];
  const samples = [];
  for (let col = 0; col < times.length; col++) {
    const t = times[col];
    const a = decode(await renderRequest({ ...base, op: 'render', t })).art;
    if (!a) throw new Error(`Missing motion fixture: ${style}/${piece}`);
    const png = raster.png(a.svg, width);
    const label = `${style} · ${piece} · t=${t}`;
    const displayH = Math.min(260, width * a.h / a.w), displayW = displayH * a.w / a.h;
    tiles.push(`<text x="${gap + col * (width + gap)}" y="${24 + y * rowH}" font-family="Inter" font-size="12" fill="#272522">${label}</text><image x="${gap + col * (width + gap)}" y="${36 + y * rowH}" width="${displayW}" height="${displayH}" href="data:image/png;base64,${Buffer.from(png).toString('base64')}"/>`);
    const file = `${stem}-t${t}.png`; writeFileSync(path.join(out, file), png);
    samples.push({ t, file, sha256: createHash('sha256').update(png).digest('hex'), w: a.w, h: a.h });
  }
  const gif = decode(await renderRequest({ ...base, op: 'export', format: 'gif', frameCount: 40, width: 400 }));
  const filename = `${stem}.gif`;
  writeFileSync(path.join(out, filename), Buffer.from(gif.bytes, 'base64'));
  records.push({ style, piece, variant, profile: motionProfile(style), file: filename, samples });
  if ((row + 1) % rowsPerPage === 0 || row === cases.length - 1) savePage(page);
  console.log(`${row + 1}/${cases.length} ${style}/${piece}/${variant ?? 'default'}: ${filename}`);
}
writeFileSync(path.join(out, 'motion-report.json'), JSON.stringify({ fixtureSha256: createHash('sha256').update(fixture).digest('hex'), rendererSourceSha256, renderer: 'production protocol', times, contacts, cases: records }, null, 2) + '\n');
console.log(`Material motion evidence: ${out}`);
