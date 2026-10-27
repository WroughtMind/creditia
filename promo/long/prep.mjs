// 长片要用的真数据：终端里 creditia fetch 的原样输出（同一个函数生成）、份额、样例项目 catnap 的文件与日历
// 颜色用标志的工具色（mark.ts），和短片、星号一致
//   node promo/long/prep.mjs promo/.build/long/data.json
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import * as V2 from '../../packages/render/src/index.ts';
import { fetchSegs } from '../../packages/render/src/text.ts';
import { toolColor, MARK_GREY } from '../../packages/render/src/mark.ts';

const out = process.argv[2] ?? 'promo/.build/long/data.json';
const raw = JSON.parse(readFileSync(new URL('../../packages/render/fixtures/catnap.json', import.meta.url)));
const m = V2.buildModel(raw, { lang: 'en', named: false, sign: '', group: 'auto' });
const fetch = fetchSegs(m).map((ln) => ln.map((s) => ({ text: s.text, color: s.color ? toolColor(s.color) : null, dim: !!s.dim, bold: !!s.bold })));
const agents = m.rows.map((a) => ({ key: a.key, name: a.name, kind: a.kind, pct: a.pct, color: a.kind === 'agent' ? toolColor(a.key) : MARK_GREY, model: a.models?.[0]?.name ?? '' }));
// creditia analyze 在终端里的三行（crates/cli/src/main.rs 的 summary 格式；tier 行只算 AI 和未确认）
const t = raw.totals;
const tier = agents.map((a) => `${a.kind === 'agent' ? a.key : 'unattributed'} ${a.pct.toFixed(1)}%`).join(' · ');
const analyze = [`${raw.project.name}: ${t.commits} commits (${t.commits} new), ${t.units} items, ${t.linesAdded} lines; 2.8s`, tier, 'wrote .creditia/credits.json'];
// 人的提交不进任何画面（只留工具、模型、数字）
const ai = (by) => Object.fromEntries(Object.entries(by).filter(([k]) => !k.startsWith('h:')));
const data = {
  project: raw.project, versions: raw.versions, totals: t, agents, fetch, analyze,
  files: raw.files.map((f) => ({ path: f.path, lines: f.lines, by: f.by })),
  days: raw.days.map((d) => ({ date: d.date, by: ai(d.by) })),
  evidence: raw.breakdown.filter((b) => !b.contributor.startsWith('h:')).map((b) => ({ id: b.contributor, ...Object.fromEntries(Object.entries(b.byEvidence ?? {}).map(([k, v]) => [k, v.linesAdded])) })),
};
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(data));
console.log('data →', out);
