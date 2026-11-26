// Regenerate with npm ci --prefix packages/render, then node packaging/gpui/dmg-background.mjs.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { dmgWindow, dmgIconPositions } from './dmg-layout.mjs';
import { firstLaunchCommand } from './installers.mjs';

const require = createRequire(new URL('../../packages/render/package.json', import.meta.url));
const { Resvg } = require('@resvg/resvg-js');
const inter = path.dirname(require.resolve('@expo-google-fonts/inter/package.json'));
const noto = path.dirname(require.resolve('@expo-google-fonts/noto-sans-sc/package.json'));
const mono = path.dirname(require.resolve('@expo-google-fonts/jetbrains-mono/package.json'));
const fontFiles = [
  path.join(inter, '400Regular/Inter_400Regular.ttf'),
  path.join(inter, '500Medium/Inter_500Medium.ttf'),
  path.join(inter, '600SemiBold/Inter_600SemiBold.ttf'),
  path.join(noto, '400Regular/NotoSansSC_400Regular.ttf'),
  path.join(noto, '500Medium/NotoSansSC_500Medium.ttf'),
  path.join(noto, '700Bold/NotoSansSC_700Bold.ttf'),
  path.join(mono, '400Regular/JetBrainsMono_400Regular.ttf'),
  path.join(mono, '500Medium/JetBrainsMono_500Medium.ttf'),
];

const logo = await fs.readFile(new URL('../../.github/readme/logo-light.svg', import.meta.url), 'utf8');
const petals = logo.match(/<path\b[^>]*\/>/g);
if (petals?.length !== 5) throw new Error('The Creditia mark must contain five petals');

const { width, height } = dmgWindow;
const [appX, appY] = dmgIconPositions['Creditia.app'];
const [applicationsX, applicationsY] = dmgIconPositions.Applications;
const arrowX = (appX + applicationsX) / 2;
const svg = `
<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"
  font-family="Inter" fill="#161512">
  <defs>
    <linearGradient id="paper" x1="0" y1="0" x2="0" y2="1">
      <stop stop-color="#F6F4EF"/>
      <stop offset="1" stop-color="#EFECE5"/>
    </linearGradient>
    <radialGradient id="stage-glow">
      <stop stop-color="#FFFCF4" stop-opacity=".7"/>
      <stop offset="1" stop-color="#FFFCF4" stop-opacity="0"/>
    </radialGradient>
    <g id="pedestal">
      <rect x="-2" y="4" width="152" height="126" rx="24" fill="#161512" fill-opacity=".03"/>
      <rect y="2" width="148" height="126" rx="22" fill="#161512" fill-opacity=".06"/>
      <rect width="148" height="124" rx="22" fill="#FBFAF7" stroke="#E2DDD3"/>
    </g>
  </defs>
  <rect width="${width}" height="${height}" fill="url(#paper)"/>
  <rect x="40" width="680" height="216" fill="url(#stage-glow)"/>

  <svg x="28" y="13" width="24" height="24" viewBox="0 0 24 24">
    ${petals.join('\n    ')}
  </svg>
  <text x="58" y="31" font-weight="600" font-size="15">creditia</text>
  <text x="732" y="31" text-anchor="end" font-family="Noto Sans SC" font-weight="500" font-size="11.5" fill="#857F76"><tspan font-family="Inter">macOS · </tspan>拖入安装</text>

  <use href="#pedestal" x="${appX - 74}" y="${appY - 50}"/>
  <g transform="translate(${applicationsX - 74} ${applicationsY - 50})">
    <use href="#pedestal"/>
    <rect x="8" y="8" width="132" height="108" rx="16" fill="none" stroke="#D5CFC4" stroke-dasharray="4 4"/>
  </g>
  <g transform="translate(${arrowX} ${appY})">
    <path d="M-88 0h176" stroke="#C9C3B8" stroke-dasharray="4 5" stroke-linecap="round"/>
    <circle r="17" fill="#FBFAF7" stroke="#DCD6CA"/>
    <path d="M-8 0H8M2 -6l6 6-6 6" fill="none" stroke="#161512" stroke-width="1.5"
      stroke-linecap="round" stroke-linejoin="round"/>
  </g>
  <g text-anchor="middle">
    <text x="${arrowX}" y="142" font-family="Noto Sans SC" font-weight="500" font-size="12" fill="#4A463F">拖入「应用程序」</text>
    <text x="${arrowX}" y="158" font-size="11" fill="#8C877E">Drag to Applications</text>
  </g>

  <path d="M28 192H732" stroke="#E5E0D7"/>

  <rect x="26" y="308" width="708" height="168" rx="18" fill="#161512" fill-opacity=".03"/>
  <rect x="28" y="306" width="704" height="167" rx="16" fill="#161512" fill-opacity=".06"/>
  <rect x="28" y="304" width="704" height="166" rx="16" fill="#FBFAF7" stroke="#E1DBD1"/>
  <circle cx="50" cy="325" r="3.5" fill="#D97757"/>
  <text x="62" y="330" font-family="Noto Sans SC" font-weight="700" font-size="13.5">首次打开若提示无法验证或已损坏</text>
  <text x="710" y="330" text-anchor="end" font-weight="500" font-size="11" fill="#8C877E">Unnotarized · First Launch</text>
  <path d="M46 343H714" stroke="#ECE7DE"/>

  <circle cx="56" cy="363" r="9" fill="#EFECE5"/>
  <circle cx="56" cy="404" r="9" fill="#EFECE5"/>
  <g text-anchor="middle" font-family="Noto Sans SC" font-weight="700" font-size="11">
    <text x="56" y="367">1</text>
    <text x="56" y="408">2</text>
  </g>
  <g font-family="Noto Sans SC" font-weight="500" font-size="12.5">
    <text x="74" y="367">先尝试打开一次，再到「系统设置 → 隐私与安全性」点「仍要打开」</text>
    <text x="74" y="408">或在「终端」运行下方命令<tspan dx="6" font-weight="400" font-size="11" fill="#8C877E">（可打开上方说明文件复制 · <tspan font-family="Inter">or copy from guide above</tspan>）</tspan></text>
  </g>
  <text x="74" y="384" font-size="11.5" fill="#78736A">Try opening once, then System Settings → Privacy &amp; Security → Open Anyway</text>

  <rect x="46" y="421" width="668" height="33" rx="9" fill="#181715"/>
  <g font-family="JetBrains Mono" font-size="12.5">
    <text x="60" y="442" font-weight="500" fill="#D97757">$</text>
    <text x="76" y="442" fill="#F3EFE7">${firstLaunchCommand}</text>
  </g>
  <text x="700" y="441" text-anchor="end" font-weight="500" font-size="10.5" fill="#706C64">Terminal</text>
</svg>
`;
const png = new Resvg(svg, { font: { fontFiles, loadSystemFonts: false } }).render().asPng();
await fs.writeFile(new URL('./dmg-background.png', import.meta.url), png);
