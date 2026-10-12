// 构建分析核心 creditia-core，放到 Tauri 打包要的位置（src-tauri/binaries/creditia-core-<目标三元组>）。
// App 里的「重新分析」「添加项目」「数据源体检」都调用它。
import { execFileSync } from 'node:child_process';
import { mkdirSync, copyFileSync } from 'node:fs';
import path from 'node:path';
const ROOT = path.resolve(import.meta.dirname, '../../..');
const triple = /host: (\S+)/.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))[1];
execFileSync('cargo', ['build', '--release', '-p', 'creditia-cli'], { cwd: ROOT, stdio: 'inherit' });
const ext = process.platform === 'win32' ? '.exe' : '';
const out = path.join(import.meta.dirname, '../src-tauri/binaries');
mkdirSync(out, { recursive: true });
copyFileSync(path.join(ROOT, 'target/release', `creditia-core${ext}`), path.join(out, `creditia-core-${triple}${ext}`));
console.log(`→ src-tauri/binaries/creditia-core-${triple}${ext}`);
