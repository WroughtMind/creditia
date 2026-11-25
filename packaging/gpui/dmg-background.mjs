// Regenerate with npm ci --prefix packages/render, then node packaging/gpui/dmg-background.mjs.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { dmgWindow, dmgIconPositions } from './dmg-layout.mjs';
import { firstLaunchCommand } from './installers.mjs';

const require = createRequire(new URL('../../packages/render/package.json', import.meta.url));
const { Resvg } = require('@resvg/resvg-js');
const noto = path.dirname(require.resolve('@expo-google-fonts/noto-sans-sc/package.json'));
const mono = path.dirname(require.resolve('@expo-google-fonts/jetbrains-mono/package.json'));
const fontFiles = [
  path.join(noto, '400Regular/NotoSansSC_400Regular.ttf'),
  path.join(noto, '700Bold/NotoSansSC_700Bold.ttf'),
  path.join(mono, '400Regular/JetBrainsMono_400Regular.ttf'),
];

const { width, height } = dmgWindow;
const [appX, appY] = dmgIconPositions['Creditia.app'];
const [applicationsX] = dmgIconPositions.Applications;
const arrowX = (appX + applicationsX) / 2;
const svg = `
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <rect width="720" height="480" fill="#edf3f3"/>
  <path d="M${arrowX - 48} ${appY}h96m-12 -12 12 12-12 12"
    fill="none" stroke="#6b8887" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
  <rect x="24" y="217" width="672" height="225" rx="14" fill="#fff" stroke="#d4e0e0"/>
  <g font-family="Noto Sans SC" fill="#142b2b">
    <g font-size="17" font-weight="700">
      <text x="46" y="245">1. 拖入安装 · 将 Creditia.app 拖入「应用程序」</text>
      <text x="46" y="305">2. 首次打开受阻（未经公证）</text>
    </g>
    <g font-size="15" fill="#354c4b">
      <text x="46" y="267">Drag Creditia.app into Applications.</text>
      <text x="46" y="327">若提示无法验证或已损坏，先尝试打开一次，再前往「系统设置 → 隐私与安全性」。</text>
      <text x="46" y="349">点击「仍要打开」，或在终端运行下方命令。</text>
      <text x="46" y="371">If macOS blocks first launch, try opening once. Then go to System Settings →</text>
      <text x="46" y="391">Privacy &amp; Security → Open Anyway, or run in Terminal:</text>
    </g>
  </g>
  <path d="M46 281h628" stroke="#d4e0e0"/>
  <rect x="46" y="401" width="628" height="30" rx="6" fill="#eaf2f1"/>
  <text x="57" y="421" font-family="JetBrains Mono" font-size="14" fill="#143d39">${firstLaunchCommand}</text>
</svg>
`;
const png = new Resvg(svg, { font: { fontFiles, loadSystemFonts: false } }).render().asPng();
await fs.writeFile(new URL('./dmg-background.png', import.meta.url), png);
