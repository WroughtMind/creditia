#!/usr/bin/env node
// Review the exact SVGs used by the native shell. Blur is a diagnostic, not an
// application effect. Native window captures remain separate from this sheet.
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const require = createRequire(path.join(repo, 'packages/render/package.json'));
const { Resvg } = require('@resvg/resvg-js');
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(name);
  if (index < 0) return fallback;
  if (!args[index + 1] || args[index + 1].startsWith('--')) throw new Error(`${name} needs a value`);
  return args[index + 1];
};
const baseline = option('--baseline', 'origin/main');
const output = path.resolve(option('--output', path.join(repo, 'evidence/optical-review')));
const sourcePath = 'apps/gpui/src/ui_icons.rs';
const before = execFileSync('git', ['show', `${baseline}:${sourcePath}`], { cwd: repo, encoding: 'utf8' });
const after = await fs.readFile(path.join(repo, sourcePath), 'utf8');
const icons = source => new Map([...source.matchAll(/"([^"]+)" => br#"(<svg[^\n]+?<\/svg>)"#/g)].map(match => [match[1], match[2]]));
const previous = icons(before), current = icons(after);
if (!previous.size || !current.size) throw new Error('No native SVG definitions found');
await fs.mkdir(output, { recursive: true });

const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const svgBody = svg => svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>$/, '');
function metrics(svg) {
  const image = new Resvg(svg.replaceAll('currentColor', '#000'), { fitTo: { mode: 'width', value: 256 } }).render();
  const pixels = image.pixels;
  let mass = 0, x = 0, y = 0, edge = 0;
  for (let row = 0; row < 256; row++) for (let col = 0; col < 256; col++) {
    const alpha = pixels[(row * 256 + col) * 4 + 3] / 255;
    mass += alpha;
    x += (col + 0.5) * alpha;
    y += (row + 0.5) * alpha;
    if (row === 0 || col === 0 || row === 255 || col === 255) edge += alpha;
  }
  if (!mass) throw new Error('An icon is blank');
  if (edge > 0.01) throw new Error('An icon touches its viewport edge; inspect clipping');
  const cx = x / mass / 16, cy = y / mass / 16;
  return { centroid: [cx, cy].map(n => +n.toFixed(3)), centerDistance: +Math.hypot(cx - 8, cy - 8).toFixed(3), inkArea: +(mass / 256).toFixed(2), clipped: false };
}

const rows = [];
const report = [];
const sheet = [];
const cell = (svg, blur, foreground, background) => `<svg viewBox="-10 -10 36 36" aria-hidden="true"><defs><filter id="blur" x="-100%" y="-100%" width="300%" height="300%"><feGaussianBlur stdDeviation="1.3"/></filter></defs><circle cx="8" cy="8" r="16" fill="${background}"/><path class="guide" d="M-8 8H24M8-8V24" stroke="#4a8bba" stroke-width=".25" stroke-dasharray="1.2 1.2"/><g color="${foreground}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"${blur ? ' filter="url(#blur)"' : ''}>${svgBody(svg)}</g></svg>`;
for (const [name, svg] of current) {
  const old = previous.get(name) ?? svg;
  const beforeMetrics = metrics(old), afterMetrics = metrics(svg);
  report.push({ name, added: !previous.has(name), before: previous.has(name) ? beforeMetrics : null, after: afterMetrics });
  const originalCell = (blur, fg, bg) => previous.has(name) ? cell(old, blur, fg, bg) : '<svg viewBox="0 0 36 36"><text x="18" y="20" text-anchor="middle" font-size="5" fill="#888">n/a</text></svg>';
  rows.push(`<tr><th scope="row">${escape(name)}${previous.has(name) ? '' : ' <small>new</small>'}</th>${[['#232323', '#eeeeec'], ['#f2f2f0', '#292a2c']].map(([fg, bg]) => `<td>${originalCell(false, fg, bg)}</td><td>${cell(svg, false, fg, bg)}</td><td>${originalCell(true, fg, bg)}</td><td>${cell(svg, true, fg, bg)}</td>`).join('')}</tr>`);
  const y = 98 + sheet.length * 82;
  const samples = [['#232323', '#eeeeec'], ['#f2f2f0', '#292a2c']].flatMap(([fg, bg]) => [originalCell(false, fg, bg), cell(svg, false, fg, bg), originalCell(true, fg, bg), cell(svg, true, fg, bg)]);
  sheet.push(`<text x="24" y="${y + 42}" font-size="14">${escape(name)}</text>${samples.map((sample, i) => sample.replace('<svg ', `<svg x="${142 + i * 96}" y="${y}" width="72" height="72" `)).join('')}`);
}

const labels = ['Before', 'After', 'Before blur', 'After blur', 'Before', 'After', 'Before blur', 'After blur'];
const sheetSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="930" height="${112 + sheet.length * 82}" font-family="Inter" fill="#252525"><rect width="100%" height="100%" fill="#f7f7f5"/><text x="24" y="38" font-size="22">Creditia · Optical alignment</text>${labels.map((label, i) => `<text x="${178 + i * 96}" y="84" text-anchor="middle" font-size="11">${label}</text>`).join('')}${sheet.join('')}</svg>`;
await fs.writeFile(path.join(output, 'sheet.svg'), sheetSvg);
await fs.writeFile(path.join(output, 'sheet.png'), new Resvg(sheetSvg, { font: { fontFiles: [path.join(repo, 'packages/render/node_modules/@expo-google-fonts/inter/400Regular/Inter_400Regular.ttf')] } }).render().asPng());

const nativeBefore = option('--native-before'), nativeAfter = option('--native-after');
let native = '';
if (nativeBefore || nativeAfter) {
  if (!nativeBefore || !nativeAfter) throw new Error('Provide both native capture directories');
  const names = (await fs.readdir(nativeAfter)).filter(name => name.endsWith('.png'));
  for (const name of names) {
    const source = path.join(nativeBefore, name);
    try { await fs.access(source); } catch { continue; }
    const original = (await fs.readFile(source)).toString('base64');
    const updated = (await fs.readFile(path.join(nativeAfter, name))).toString('base64');
    native += `<details><summary>${escape(name)}</summary><div class="native"><figure><figcaption>Before</figcaption><img src="data:image/png;base64,${original}" alt="Original native window"></figure><figure><figcaption>After</figcaption><img src="data:image/png;base64,${updated}" alt="Updated native window"></figure></div></details>`;
  }
}
await fs.writeFile(path.join(output, 'metrics.json'), JSON.stringify({ baseline, source: sourcePath, method: 'Alpha-weighted centroid at 16x resolution. A diagnostic, not an automatic alignment rule.', icons: report }, null, 2) + '\n');
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Creditia · Optical alignment</title><style>
*{box-sizing:border-box}body{margin:0 auto;padding:36px 24px;max-width:1160px;background:#f7f7f5;color:#252525;font:14px/1.55 system-ui,sans-serif}h1{font-size:26px;font-weight:600;margin:0 0 8px}p{max-width:780px;color:#60605e}table{border-collapse:collapse;width:100%;table-layout:fixed;margin-top:24px}th,td{padding:8px 2px;text-align:center;border-bottom:1px solid #dededb}th{font-weight:500}th:first-child{width:130px;text-align:left}td svg{width:72px;height:72px;vertical-align:middle}small{font-size:10px;color:#777}label{display:inline-flex;align-items:center;gap:7px;margin-right:22px}.guides-off .guide{visibility:hidden}.native{display:grid;grid-template-columns:1fr 1fr;gap:12px}.native figure{margin:0}.native img{width:100%;display:block}figcaption,summary{padding:12px 0}details{border-bottom:1px solid #dededb}
</style><h1>Creditia · Optical alignment</h1><p>Blur the painted shape, inspect its visual weight inside the control, adjust, then restore sharp detail. Clear and blurred versions below use the exact SVG geometry from the native shell at 2× display scale. The crosshair marks the unchanged layout center.</p><p>Alpha centroids help locate imbalance; they do not replace visual review of the native window, text, spacing, hover and focus states.</p><label><input type="checkbox" id="guides" checked>Center guides</label><table><thead><tr><th rowspan="2">Icon</th><th colspan="4">Light surface</th><th colspan="4">Dark surface</th></tr><tr>${['Before','After','Before · blur','After · blur','Before','After','Before · blur','After · blur'].map(label => `<th>${label}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table>${native ? `<h2>Actual native captures</h2>${native}` : '<p>No native window captures were supplied. The sheet above checks vector geometry only.</p>'}<script>document.querySelector('#guides').addEventListener('change',event=>document.body.classList.toggle('guides-off',!event.target.checked));</script></html>`;
await fs.writeFile(path.join(output, 'index.html'), html);
console.log(JSON.stringify({ output, icons: current.size, nativeCaptures: !!native, metrics: report.filter(row => row.before && row.before.centerDistance !== row.after.centerDistance) }, null, 2));
