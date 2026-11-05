#!/usr/bin/env node
// Reproducible material-motion evidence from the production renderer.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
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
const decode = response => { if (!response.ok) throw new Error(JSON.stringify(response.error)); return response.result; };
const session = createRenderSession(nativeBackend);
const renderRequest = input => session.dispatch(input);
decode(await renderRequest({ version: 1, id: 'fixture', op: 'archive', archiveId: 'motion-fixture', raw }));
for (let row = 0; row < cases.length; row++) {
  const [style, piece, variant] = cases[row];
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
    samples.push({ t, sha256: createHash('sha256').update(png).digest('hex'), w: a.w, h: a.h });
  }
  const gif = decode(await renderRequest({ ...base, op: 'export', format: 'gif', frameCount: 40, width: 400 }));
  const filename = `${style}-${piece}${variant ? '-' + variant : ''}.gif`;
  writeFileSync(path.join(out, filename), Buffer.from(gif.bytes, 'base64'));
  records.push({ style, piece, variant, profile: motionProfile(style), file: filename, samples });
  console.log(`${row + 1}/${cases.length} ${style}/${piece}/${variant ?? 'default'}: ${filename}`);
}
const W = gap + times.length * (width + gap), contacts = [];
for (let i = 0; i < pages.length; i++) {
  const H = Math.min(rowsPerPage, cases.length - i * rowsPerPage) * rowH;
  const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="#efede8"/>${pages[i].join('')}</svg>`;
  const filename = i === 0 ? 'material-motion-contact.png' : `material-motion-contact-${i + 1}.png`;
  writeFileSync(path.join(out, filename), raster.png(sheet, W)); contacts.push(filename);
}
writeFileSync(path.join(out, 'motion-report.json'), JSON.stringify({ fixtureSha256: createHash('sha256').update(fixture).digest('hex'), renderer: 'production protocol', times, contacts, cases: records }, null, 2) + '\n');
console.log(`Material motion evidence: ${out}`);
