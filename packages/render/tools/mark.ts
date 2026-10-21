// 从 src/mark.ts 写出标志文件：assets/creditia/logo-star.svg（五色）和 logo-star-mono.svg（黑白，#0E0E0E 是墨色，用的地方换成 currentColor）
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { markSvg } from '../src/mark.ts';
const dir = path.join(import.meta.dirname, '../assets/creditia');
writeFileSync(path.join(dir, 'logo-star.svg'), markSvg() + '\n');
writeFileSync(path.join(dir, 'logo-star-mono.svg'), markSvg({ ink: '#0E0E0E' }) + '\n');
console.log('✓ logo-star.svg, logo-star-mono.svg');
