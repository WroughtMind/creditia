// 样张：每种形式 × 每种风格，片尾字幕和小票的全部版式。node src/gallery.ts <spike out 目录> <输出目录>
import { readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { FORMS, renderSvg, svgToPng, buildModel, THEMES, type Ctx } from './index.ts';
import { badge, ROLL_VARIANTS, RECEIPT_VARIANTS } from './forms.ts';
import { creditsBlock, mergeCreditsMd, releaseNotes, prComment } from './text.ts';

const [, , dataDir = '../../spike/out', outDir = 'gallery'] = process.argv;
const load = (p: string) => JSON.parse(readFileSync(path.join(dataDir, p), 'utf8'));
const opt = { director: process.env.DIRECTOR === '1', lang: 'zh' as const };
const m = buildModel(load('weibei/credits.json'), opt);
const projects = [m, ...['claude-code-zh-cn', 'lifecycle-site'].map((n) => buildModel(load(`${n}/credits.json`), opt))];
const ctx: Ctx = { lang: 'zh', projects, person: load('weibei/credits.json').directors?.[0]?.name, blame: load('weibei/blame-sample.json') };

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const onlyForms = process.env.FORMS?.split(','), onlyThemes = process.env.THEMES?.split(',');
const made: Record<string, string[]> = {};
for (const [id, f] of Object.entries(FORMS)) {
  if (onlyForms && !onlyForms.includes(id)) continue;
  for (const t of THEMES) {
    if (onlyThemes && !onlyThemes.includes(t.id)) continue;
    const svg = await renderSvg(id, m, t, ctx);
    if (!svg) continue;
    writeFileSync(path.join(outDir, `${id}-${t.id}.png`), svgToPng(svg));
    (made[id] ??= []).push(t.id);
  }
  console.log('✓', id);
}

// 版式变体：片尾字幕五种版式都用 Film Credits；小票各回自己的风格
// 每种版式在哪个主题下最像自己
const HOME: Record<string, string> = { slip: 'minimal-light', thermal: 'terminal', folio: 'editorial', stub: 'film', ticket: 'cinema', ledger: 'broadsheet', scroll: 'crt' };
const ROLL_HOME: Record<string, string> = { cast: 'cinema', scroll: 'crt', cards: 'arcade', end: 'gala', block: 'poster', program: 'playbill' };
const variants: [string, string, string][] = [
  ...ROLL_VARIANTS.map((v): [string, string, string] => ['roll', ROLL_HOME[v] ?? 'film', v]),
  ...RECEIPT_VARIANTS.map((v): [string, string, string] => ['receipt', HOME[v], v]),
];
for (const [id, th, v] of variants) {
  const t = THEMES.find((x) => x.id === th)!;
  const svg = await renderSvg(id, m, t, { ...ctx, variant: v });
  if (svg) writeFileSync(path.join(outDir, `${id}-${th}-${v}.png`), svgToPng(svg));
}
console.log('✓ variants');

for (const t of THEMES) writeFileSync(path.join(outDir, `badge-${t.id}.svg`), badge(m, t));
writeFileSync(path.join(outDir, 'CREDITS.md'), mergeCreditsMd(null, creditsBlock(m)));
writeFileSync(path.join(outDir, 'release-notes.md'), releaseNotes(m, 'v1.0.0'));
writeFileSync(path.join(outDir, 'pr-comment.md'), prComment(m, 'pr:519'));

// 总览页：上方切换风格，下方按清单顺序排列
const esc = (s: string) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!);
const groups: [string, string[]][] = [
  ['一眼看懂', ['tier', 'share', 'byline', 'badge']],
  ['有情绪、适合传播', ['roll', 'receipt', 'ribbon', 'mosaic', 'profile', 'fetch']],
  ['版式变体', ['roll-variants', 'receipt-variants']],
  ['融进工作流', ['credits-md', 'pr', 'release', 'blame', 'app']],
  ['统计图', ['waffle', 'donut', 'area', 'lanes', 'clock', 'versions', 'settle']],
];
const width: Record<string, number> = { tier: 360, app: 300, receipt: 420, roll: 420, byline: 720, share: 720, ribbon: 620, mosaic: 620, settle: 620 };
const text = (f: string) => `<pre>${esc(readFileSync(path.join(outDir, f), 'utf8'))}</pre>`;
const VNAME: Record<string, string> = { classic: '居中堆叠', columns: '双栏', billing: '海报演职员表', title: '标题卡', crawl: '爬行定格', slip: '纸质清单', thermal: '热敏纸', folio: '财务单', stub: '电影票', cast: '卡司表', scroll: '双栏滚屏 / 长卷纸', cards: '卡串', end: '终卡', block: 'billing block', program: '节目单', ticket: '电影票', ledger: '账簿', };
const section = (id: string) => {
  if (id === 'badge') return `<section><h3>徽章<span>README 顶部；浅色/深色底各看一遍</span></h3><div class="badges">${THEMES.map((t) => `<img data-t="${t.id}" src="badge-${t.id}.svg">`).join('')}</div><div class="badges dark">${THEMES.map((t) => `<img data-t="${t.id}" src="badge-${t.id}.svg">`).join('')}</div></section>`;
  if (id === 'roll-variants') return `<section><h3>片尾字幕 · 版式<span>classic / columns / billing / title / crawl</span></h3><div class="imgs">${ROLL_VARIANTS.map((v) => `<a data-t="${ROLL_HOME[v] ?? 'film'}" href="roll-${ROLL_HOME[v] ?? 'film'}-${v}.png" target="_blank"><figure><img loading="lazy" src="roll-${ROLL_HOME[v] ?? 'film'}-${v}.png" style="width:340px"><figcaption>${v} · ${VNAME[v]}</figcaption></figure></a>`).join('')}</div></section>`;
  if (id === 'receipt-variants') return `<section><h3>精力小票 · 版式<span>${RECEIPT_VARIANTS.join(' / ')}</span></h3><div class="imgs">${RECEIPT_VARIANTS.map((v) => `<a data-t="${HOME[v]}" href="receipt-${HOME[v]}-${v}.png" target="_blank"><figure><img loading="lazy" src="receipt-${HOME[v]}-${v}.png" style="width:340px"><figcaption>${v} · ${VNAME[v]}</figcaption></figure></a>`).join('')}</div></section>`;
  if (id === 'credits-md') return `<section class="txt"><h3>CREDITS.md<span>按版本、按 PR</span></h3>${text('CREDITS.md')}</section>`;
  if (id === 'pr') return `<section class="txt"><h3>PR 评论<span>这个 PR 谁做的</span></h3>${text('pr-comment.md')}</section>`;
  if (id === 'release') return `<section class="txt"><h3>Release notes 段落<span>这个版本谁做的</span></h3>${text('release-notes.md')}</section>`;
  const f = FORMS[id];
  if (!made[id]) return '';
  return `<section><h3>${f.name}<span>${f.q}</span></h3><div class="imgs">${made[id].map((t) => `<a data-t="${t}" href="${id}-${t}.png" target="_blank"><img loading="lazy" src="${id}-${t}.png" style="width:${width[id] ?? 560}px"></a>`).join('')}</div></section>`;
};
writeFileSync(path.join(outDir, 'index.html'), `<!doctype html><meta charset="utf-8"><title>creditia · 展示形式</title>
<style>
:root{color-scheme:light}
body{margin:0;background:#EEEDEA;font:15px/1.6 -apple-system,"PingFang SC",sans-serif;color:#161514}
header{position:sticky;top:0;z-index:2;background:rgba(238,237,234,.86);backdrop-filter:blur(20px);-webkit-backdrop-filter:blur(20px);padding:18px 64px;display:flex;gap:28px;align-items:center;border-bottom:1px solid rgba(0,0,0,.06)}
header b{font-size:15px;font-weight:600;margin-right:12px}
.seg{display:flex;gap:2px;background:rgba(0,0,0,.05);padding:3px;border-radius:10px}
.seg button{border:0;background:none;padding:6px 14px;border-radius:7px;font:inherit;font-size:13px;color:#76726B;cursor:pointer}
.seg button.on{background:#fff;color:#161514;box-shadow:0 1px 2px rgba(0,0,0,.08)}
main{padding:24px 64px 160px}
h2{font-size:12px;letter-spacing:.16em;color:#9A958D;font-weight:500;margin:80px 0 8px;text-transform:uppercase}
section{margin-top:44px}h3{font-size:16px;font-weight:600;margin:0 0 16px}h3 span{font-weight:400;color:#8F8B84;margin-left:12px;font-size:13px}
.imgs{display:flex;gap:24px;flex-wrap:wrap;align-items:flex-start}.imgs img{display:block;border-radius:10px}
figure{margin:0}figcaption{margin-top:8px;font-size:12.5px;color:#76726B}
.badges{display:flex;gap:16px;flex-wrap:wrap;margin-bottom:14px}.badges img{height:22px}
.badges.dark{background:#12110F;padding:16px;border-radius:10px}
pre{background:#fff;border-radius:12px;padding:22px 26px;font:13px/1.75 ui-monospace,Menlo,monospace;white-space:pre-wrap;max-width:860px;margin:0}
body:not(.all) [data-t]:not(.show){display:none}
</style>
<header><b>creditia</b><div class="seg" id="s"><button data-v="all">全部</button>${THEMES.map((t) => `<button data-v="${t.id}">${esc(t.name)}</button>`).join('')}</div></header>
<main>${groups.map(([g, ids]) => `<h2>${g}</h2>${ids.map(section).join('')}`).join('')}</main>
<script>
const s=document.getElementById('s');
function pick(v){document.body.classList.toggle('all',v==='all');document.querySelectorAll('[data-t]').forEach(e=>e.classList.toggle('show',e.dataset.t===v));s.querySelectorAll('button').forEach(b=>b.classList.toggle('on',b.dataset.v===v));localStorage.t=v}
s.onclick=e=>e.target.dataset.v&&pick(e.target.dataset.v);pick(localStorage.t||'minimal-light');
</script>`);
console.log('→', path.join(outDir, 'index.html'));
