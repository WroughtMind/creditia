#!/usr/bin/env node
// Reproducible material-motion evidence from the production renderer.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { renderRequest } from '../src/native-protocol.ts';
import { raster } from '../src/index.ts';
const out = path.resolve(process.argv[2] ?? 'motion-evidence');
mkdirSync(out, { recursive: true });
const fixture = readFileSync(new URL('../fixtures/catnap.json', import.meta.url));
const raw = JSON.parse(fixture);
const cases = [
  ['ink', 'asterisk'], ['meadow', 'asterisk'], ['glass', 'asterisk'], ['film', 'asterisk'], ['terminal', 'asterisk'],
  ['ink', 'tier', 'grouped'], ['woodblock', 'mosaic'], ['film', 'roll', 'crawl'], ['terminal', 'tier', 'original'],
];
const times = [0, .2, .45, .7, 1], width = 220, gap = 12, rowH = 300;
const tiles = [], records = [];
const decode = response => { if (!response.ok) throw new Error(JSON.stringify(response.error)); return response.result; };
for (let row = 0; row < cases.length; row++) {
  const [style, piece, variant] = cases[row];
  const base = { version: 1, id: `${style}-${piece}`, raw, style, piece, lang: 'zh', group: 'model', named: false, ...(variant ? { variant } : {}) };
  const samples = [];
  for (let col = 0; col < times.length; col++) {
    const t = times[col];
    const a = decode(await renderRequest({ ...base, op: 'render', t })).art;
    if (!a) throw new Error(`Missing motion fixture: ${style}/${piece}`);
    const png = raster.png(a.svg, width);
    const label = `${style} · ${piece} · t=${t}`;
    const displayH = Math.min(260, width * a.h / a.w), displayW = displayH * a.w / a.h;
    tiles.push(`<text x="${gap + col * (width + gap)}" y="${24 + row * rowH}" font-family="Inter" font-size="12" fill="#272522">${label}</text><image x="${gap + col * (width + gap)}" y="${36 + row * rowH}" width="${displayW}" height="${displayH}" href="data:image/png;base64,${Buffer.from(png).toString('base64')}"/>`);
    samples.push({ t, sha256: createHash('sha256').update(png).digest('hex'), w: a.w, h: a.h });
  }
  const gif = decode(await renderRequest({ ...base, op: 'export', format: 'gif', frameCount: 48, width: 480 }));
  const filename = `${style}-${piece}${variant ? '-' + variant : ''}.gif`;
  writeFileSync(path.join(out, filename), Buffer.from(gif.bytes, 'base64'));
  records.push({ style, piece, variant, file: filename, samples });
}
const W = gap + times.length * (width + gap), H = cases.length * rowH;
const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="#efede8"/>${tiles.join('')}</svg>`;
writeFileSync(path.join(out, 'material-motion-contact.png'), raster.png(sheet, W));
writeFileSync(path.join(out, 'motion-report.json'), JSON.stringify({ fixtureSha256: createHash('sha256').update(fixture).digest('hex'), renderer: 'production protocol', times, cases: records }, null, 2) + '\n');
console.log(`Material motion evidence: ${out}`);
