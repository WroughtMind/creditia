// 从共享标志生成 App 和网页使用的 PNG，以及 Icon Composer 的矢量图层。
import { writeFileSync, readFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const HERE = import.meta.dirname;
const require = createRequire(path.resolve(HERE, '../../packages/render/package.json'));
const { Resvg } = require('@resvg/resvg-js');
const mark = (file) => readFileSync(path.resolve(HERE, '../../packages/render/assets/creditia', file), 'utf8').replace(/^<svg[^>]*>|<\/svg>\s*$/g, '').replaceAll('#0E0E0E', '#F4F4F2');
// Icon Composer keeps each original petal as a vector layer; materials live in icon.json.
const petals = mark('logo-star.svg').match(/<path\b[^>]*\/>/g);
if (petals?.length !== 5) throw new Error('The star mark must contain five petals');
const LAYERS = path.join(HERE, 'Creditia.icon/Assets');
mkdirSync(LAYERS, { recursive: true });
for (const [i, petal] of petals.entries()) {
  writeFileSync(path.join(LAYERS, `petal-${i + 1}.svg`), `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><g transform="translate(164 164) scale(29)">${petal}</g></svg>\n`);
}
const S = 1024, M = 100, R = 185;
const tile = (logo) => `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}">
<defs>
  <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2A2A2A"/><stop offset="1" stop-color="#050505"/></linearGradient>
  <linearGradient id="sheen" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFFFFF" stop-opacity=".22"/><stop offset=".5" stop-color="#FFFFFF" stop-opacity="0"/></linearGradient>
</defs>
<rect x="${M}" y="${M}" width="${S - 2 * M}" height="${S - 2 * M}" rx="${R}" fill="url(#bg)"/>
<rect x="${M}" y="${M}" width="${S - 2 * M}" height="${(S - 2 * M) / 2}" rx="${R}" fill="url(#sheen)"/>
<rect x="${M + 2}" y="${M + 2}" width="${S - 2 * M - 4}" height="${S - 2 * M - 4}" rx="${R - 2}" fill="none" stroke="#FFFFFF" stroke-opacity=".16" stroke-width="4"/>
<g transform="translate(232 232) scale(23.33)">${logo}</g>
</svg>`;
function writeIcon(output, file, width) {
  const image = new Resvg(tile(mark(file)), { fitTo: { mode: 'width', value: width } });
  writeFileSync(path.join(HERE, output), image.render().asPng());
  console.log(`→ packaging/icons/${output}`);
}

writeIcon('icon.png', 'logo-star.svg', 512);
writeIcon('128x128@2x.png', 'logo-star.svg', 256);
const DOCK = path.join(HERE, 'dock');
mkdirSync(DOCK, { recursive: true });
for (const [id, file] of [['star-mono', 'logo-star-mono.svg'], ['orb', 'logo-orb.svg'], ['ring', 'logo-ring.svg'], ['color', 'logo-color.svg'], ['mono', 'logo.svg']]) {
  writeIcon(`dock/${id}.png`, file, 512);
}
