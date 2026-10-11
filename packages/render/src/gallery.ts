// 样张：全部作品 × 全部风格（加全部版式），出 PNG 和一张总览 HTML，用来做视觉验收。
// node src/gallery.ts [credits.json] [输出目录] [风格,风格…]
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import path from 'node:path';
import { buildModel, STYLES, PIECES, art, exporter } from './index.ts';

const [, , dataFile = path.join(import.meta.dirname, '../fixtures/lumen.json'), outDir = 'gallery', only] = process.argv;
const m = buildModel(JSON.parse(readFileSync(dataFile, 'utf8')), { lang: (process.env.LANG_CT as 'zh' | 'en') ?? 'zh', named: process.env.NAMED === '1', sign: process.env.SIGN ?? '' });
mkdirSync(outDir, { recursive: true });
const styles = STYLES.filter((s) => !only || only.split(',').includes(s.id));
const made: { style: string; piece: string; variant: string; file: string }[] = [];
for (const st of styles) {
  for (const [id, p] of Object.entries(PIECES)) {
    for (const v of p.variants ?? ['']) {
      const a = art(id, m, st, { variant: v || undefined });
      if (!a) continue;
      const file = `${st.id}-${id}${v ? '-' + v : ''}.png`;
      writeFileSync(path.join(outDir, file), exporter.png(a, Math.min(1, 900 / a.w)));
      made.push({ style: st.id, piece: id, variant: v, file });
    }
  }
  console.log('✓', st.id);
}
// 总览：按风格分节；已有的 index 追加
const idx = path.join(outDir, 'index.json');
const all = [...(existsSync(idx) ? JSON.parse(readFileSync(idx, 'utf8')) : []).filter((x: any) => !styles.some((s) => s.id === x.style)), ...made];
writeFileSync(idx, JSON.stringify(all));
writeFileSync(path.join(outDir, 'index.html'), `<!doctype html><meta charset="utf-8"><title>creditia gallery</title>
<style>body{margin:0;background:#111;color:#ddd;font:13px Inter,sans-serif;padding:24px}h2{margin:40px 0 12px;font-weight:600}.g{display:flex;flex-wrap:wrap;gap:16px;align-items:flex-start}figure{margin:0;width:300px}img{width:100%;border-radius:4px}figcaption{color:#888;margin-top:4px}</style>
${STYLES.filter((s) => all.some((x: any) => x.style === s.id)).map((s) => `<h2>${s.name} · ${s.en}<small style="color:#777;font-weight:400"> — ${s.note}</small></h2><div class="g">${all.filter((x: any) => x.style === s.id).map((x: any) => `<figure><img loading="lazy" src="${x.file}"><figcaption>${x.piece}${x.variant ? ' · ' + x.variant : ''}</figcaption></figure>`).join('')}</div>`).join('')}`);
console.log(`→ ${outDir}/index.html（${all.length} 张）`);
