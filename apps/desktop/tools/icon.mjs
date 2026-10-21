// 应用图标：黑色圆角方块 + 标志（工具颜色的横纹，白色四角星），带一点玻璃高光。产物 tools/icon.png，再交给 tauri icon。
import { writeFileSync, readFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const HERE = import.meta.dirname;
const require = createRequire(path.resolve(HERE, '../../../packages/render/package.json'));
const { Resvg } = require('@resvg/resvg-js');
const mark = (file) => readFileSync(path.resolve(HERE, '../../../packages/render/assets/creditia', file), 'utf8').replace(/^<svg[^>]*>|<\/svg>\s*$/g, '').replaceAll('#0E0E0E', '#F4F4F2');
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
// 安装包图标用默认的条纹圆；设置里能换成的其他标志，另出一张 512 的给程序坞（src-tauri/icons/dock/）
writeFileSync(path.join(HERE, 'icon.png'), new Resvg(tile(mark('logo-orb.svg'))).render().asPng());
console.log('→ tools/icon.png');
const DOCK = path.resolve(HERE, '../src-tauri/icons/dock');
mkdirSync(DOCK, { recursive: true });
for (const [id, file] of [['ring', 'logo-ring.svg'], ['color', 'logo-color.svg'], ['mono', 'logo.svg']]) {
  writeFileSync(path.join(DOCK, `${id}.png`), new Resvg(tile(mark(file)), { fitTo: { mode: 'width', value: 512 } }).render().asPng());
  console.log(`→ src-tauri/icons/dock/${id}.png`);
}
