// 把 assets/logos/*.svg 收成 src/logos.ts（只留 path 数据），这样 Node 与浏览器都不用读文件。
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
const dir = path.join(import.meta.dirname, '../assets/logos');
const out = {};
for (const f of readdirSync(dir).filter((x) => x.endsWith('.svg')).sort()) {
  const s = readFileSync(path.join(dir, f), 'utf8');
  const ds = [...s.matchAll(/<path[^>]*\sd="([^"]+)"/g)].map((m) => m[1]);
  const vb = /viewBox="([^"]+)"/.exec(s)?.[1] ?? '0 0 24 24';
  if (ds.length) out[f.replace('.svg', '')] = { vb, d: ds.join(' ') };
}
writeFileSync(path.join(import.meta.dirname, '../src/logos.ts'), '// 由 tools/logos.mjs 从 assets/logos 生成，勿手改。来源见 assets/logos/SOURCES.md\nexport const LOGOS: Record<string, { vb: string; d: string }> = ' + JSON.stringify(out, null, 0).replace(/},"/g, '},\n  "') + ';\n');
console.log(Object.keys(out).join(' '));
