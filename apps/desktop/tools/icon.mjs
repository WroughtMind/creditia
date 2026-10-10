// 应用图标：纸底 + 朱红印章 + 白文「名」。临时方案，正式 logo 等 App 风格定稿后做。
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const HERE = path.dirname(new URL(import.meta.url).pathname);
const RENDER = path.resolve(HERE, '../../../packages/render');
const require = createRequire(path.join(RENDER, 'package.json'));
const { Resvg } = require('@resvg/resvg-js');
const font = require.resolve('@expo-google-fonts/noto-serif-sc/500Medium/NotoSerifSC_500Medium.ttf');
const S = 1024, M = 100, R = 185; // macOS 图标：824 的内容区，圆角 185
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}">
<defs>
  <linearGradient id="p" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#F7F4EE"/><stop offset="1" stop-color="#EAE4D8"/></linearGradient>
</defs>
<rect x="${M}" y="${M}" width="${S - 2 * M}" height="${S - 2 * M}" rx="${R}" fill="url(#p)"/>
<g transform="rotate(-4 512 512)">
  <rect x="296" y="310" width="460" height="460" rx="44" fill="#5A1E12" opacity="0.18"/>
  <rect x="282" y="282" width="460" height="460" rx="44" fill="#C8442E"/>
  <rect x="312" y="312" width="400" height="400" rx="28" fill="none" stroke="#F7F4EE" stroke-width="9" opacity="0.9"/>
  <text x="512" y="634" text-anchor="middle" font-family="Noto Serif SC" font-weight="500" font-size="330" fill="#F7F4EE">名</text>
</g>
</svg>`;
const png = new Resvg(svg, { font: { fontFiles: [font], loadSystemFonts: false, defaultFontFamily: 'Noto Serif SC' } }).render().asPng();
writeFileSync(path.join(HERE, 'icon.png'), png);
console.log('→ tools/icon.png');
