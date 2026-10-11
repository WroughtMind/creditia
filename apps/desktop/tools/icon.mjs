import { copyFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
copyFileSync(fileURLToPath(new URL('../../../packages/render/assets/creditia/icon.png', import.meta.url)), fileURLToPath(new URL('./icon.png', import.meta.url)));
console.log('已准备新应用图标');
