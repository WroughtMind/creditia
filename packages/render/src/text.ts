// 文本形式：CREDITS.md、Release notes、PR 评论、终端名片、逐行溯源。都读同一份 credits.json。
import type { Model } from './data.ts';
import { fmt, pct, compact, nameOf } from './data.ts';
import type { Seg } from './forms.ts';

const harnessOf = (raw: any, id: string) => (raw.contributors ?? []).find((x: any) => x.id === id)?.harness ?? (id === 'unattributed' ? 'unattributed' : 'human');
const labelOf = (raw: any, id: string) => {
  const c = (raw.contributors ?? []).find((x: any) => x.id === id);
  if (!c || id === 'unattributed') return nameOf('unattributed');
  if (c.kind === 'human') return c.name ?? id;
  const n = nameOf(c.harness);
  return c.model ? `${n}（${c.model === 'auto' ? 'Auto' : c.model}）` : n;
};
const clean = (s: string) => s.replace(/\s*\(#\d+\)\s*$/, '');

/** 一项的主要贡献方：先按工具合并取最大，再取该工具下占比最大的模型 */
function lead(raw: any, u: any) {
  const by = new Map<string, number>();
  for (const s of u.shares ?? []) by.set(harnessOf(raw, s.contributor), (by.get(harnessOf(raw, s.contributor)) ?? 0) + s.share);
  const top = [...by.entries()].sort((a, b) => b[1] - a[1])[0];
  if (!top || top[0] === 'unattributed') return nameOf('unattributed');
  const best = (u.shares ?? []).filter((s: any) => harnessOf(raw, s.contributor) === top[0]).sort((a: any, b: any) => b.share - a.share)[0];
  return labelOf(raw, best.contributor);
}

/** 某版本内各工具占比 */
function versionLine(raw: any, tag: string) {
  const by = new Map<string, number>();
  for (const b of raw.breakdown ?? []) {
    const k = harnessOf(raw, b.contributor);
    by.set(k === 'human' ? 'other' : k, (by.get(k === 'human' ? 'other' : k) ?? 0) + (b.byVersion?.[tag] ?? 0));
  }
  const tot = [...by.values()].reduce((a, b) => a + b, 0);
  if (!tot) return '';
  return [...by.entries()].filter(([, v]) => v > 0)
    .sort((a, b) => (a[0] === 'unattributed' ? 1 : b[0] === 'unattributed' ? -1 : b[1] - a[1]))
    .map(([k, v]) => `${nameOf(k)} ${pct((v / tot) * 100)}`).join(' · ');
}

/** 给 App 的档案页用：某一项的主要贡献方 */
export const unitLead = lead;

export const MARK_START = '<!-- creditia:start -->';
export const MARK_END = '<!-- creditia:end -->';

/** CREDITS.md 标记区间里的内容。人按名字逐条列出，不和 agent 并排算百分比排名 */
export function creditsBlock(m: Model): string {
  const raw = m.raw;
  const out: string[] = [];
  const head = [m.project, m.signature && m.signature, `${fmt(m.commits)} 次提交`, `${fmt(m.units)} 个 PR / 直接提交`, `+${fmt(m.linesAdded)} 行`].filter(Boolean).join(' · ');
  out.push(head, '');
  out.push('| 谁写的 | 占比 | 写了多少行 | 现在还在的代码 |', '|---|---:|---:|---:|');
  const surv = new Map<string, number>();
  for (const b of raw.breakdown ?? []) { const k = harnessOf(raw, b.contributor); surv.set(k, (surv.get(k) ?? 0) + (b.linesSurviving ?? 0)); }
  for (const r of m.rows) {
    if (r.key === 'human') {
      out.push(`| 人的代码记录 | ${pct(r.pct)} | +${fmt(r.lines)} | ${fmt(surv.get('human') ?? 0)} |`);
      continue;
    }
    out.push(`| ${r.name} | ${pct(r.pct)} | +${fmt(r.lines)} | ${r.kind === 'other' ? '—' : fmt(surv.get(r.key) ?? 0)} |`);
  }
  out.push('', '> 占比按历史新增代码行计算，不代表劳动时间、项目价值或人的贡献份额。', '');
  if (m.people.length) {
    out.push('## 制作人员', '');
    for (const p of m.people) out.push(`- ${p.name}${p.roles.length ? ` · ${p.roles.join('、')}` : ''}${p.note ? ` — ${p.note}` : ''}`);
    out.push('');
  }
  const weak = (raw.coverage ?? []).filter((c: any) => c.verifiedShare === undefined ? c.best !== 'verified' : c.verifiedShare < 1);
  if (weak.length) out.push(`> ${weak[0].from} – ${weak[weak.length - 1].to} 有部分代码没有逐行日志。提交声明单列，证据不足或冲突的部分保留为未确认；不按分支名或相近时间猜测作者。`, '');
  const units: any[] = raw.units ?? [];
  const groups: [string, string, string, any[]][] = [];
  const unrel = units.filter((u) => !u.version);
  if (unrel.length) groups.push(['未发布', '', 'unreleased', unrel]);
  for (const v of [...(raw.versions ?? [])].reverse()) groups.push([v.tag, v.date, v.tag, units.filter((u) => u.version === v.tag)]);
  for (const [title, date, key, us] of groups) {
    if (!us.length) continue;
    out.push(`## ${title}${date ? ` · ${date}` : ''}`, '');
    const vl = versionLine(raw, key);
    if (vl) out.push(vl, '');
    const prs = us.filter((u) => u.id.startsWith('pr:')).sort((a, b) => b.date.localeCompare(a.date));
    for (const u of prs) out.push(`- #${u.id.slice(3)} ${clean(u.title)} — ${lead(raw, u)}`);
    const direct = us.length - prs.length;
    if (direct) out.push(`- ${direct} 个直接提交`);
    out.push('');
  }
  out.push(`<sub>由 creditia 生成 · ${String(raw.generatedAt ?? '').slice(0, 10)}</sub>`);
  return out.join('\n');
}

/** 合并进已有 CREDITS.md：只改写标记区间，区间外的手写内容原样保留 */
export function mergeCreditsMd(existing: string | null, block: string): string {
  const body = `${MARK_START}\n\n${block}\n\n${MARK_END}`;
  if (!existing) return `# Credits\n\n${body}\n`;
  const a = existing.indexOf(MARK_START), b = existing.indexOf(MARK_END);
  if (a >= 0 && b > a) return existing.slice(0, a) + body + existing.slice(b + MARK_END.length);
  return existing.replace(/\s*$/, '\n\n') + body + '\n';
}

export function releaseNotes(m: Model, tag: string): string {
  const raw = m.raw;
  const us = (raw.units ?? []).filter((u: any) => (tag === 'unreleased' ? !u.version : u.version === tag) && u.id.startsWith('pr:'))
    .sort((a: any, b: any) => b.date.localeCompare(a.date));
  const lines = ['### Credits', ''];
  const vl = versionLine(raw, tag);
  if (vl) lines.push(vl, '');
  for (const u of us) lines.push(`- #${u.id.slice(3)} ${clean(u.title)} — ${lead(raw, u)}`);
  return lines.join('\n') + '\n';
}

export function prComment(m: Model, id: string): string {
  const raw = m.raw;
  const u = (raw.units ?? []).find((x: any) => x.id === id);
  if (!u) return '';
  const by = new Map<string, { share: number; models: string[] }>();
  for (const s of u.shares) {
    const k = harnessOf(raw, s.contributor);
    const e = by.get(k) ?? { share: 0, models: [] };
    e.share += s.share;
    const c = (raw.contributors ?? []).find((x: any) => x.id === s.contributor);
    if (c?.model && s.share > 0) e.models.push(c.model === 'auto' ? 'Auto' : c.model);
    by.set(k, e);
  }
  const parts = [...by.entries()].filter(([, v]) => v.share > 0).sort((a, b) => b[1].share - a[1].share)
    .map(([k, v]) => `**${k === 'human' ? '人' : nameOf(k)}** ${pct(v.share * 100)}${v.models.length ? `（${v.models.join('、')}）` : ''}`);
  return `这个 PR 的 ${fmt(u.lines)} 行新增代码：${parts.join('，')}。\n`;
}

/** creditia fetch：agent 排百分比，人单独一行 */
export function fetchSegs(m: Model): Seg[][] {
  const w = 12;
  const kv = (k: string, v: string): Seg[] => [{ text: k.padEnd(8, ' '), dim: true }, { text: v }];
  const blocks = (p: number) => '■'.repeat(Math.max(1, Math.round(p / 3.5)));
  const ranked = m.rows.filter((r) => r.key !== 'human');
  return [
    [{ text: m.project, bold: true }, ...(m.signature ? [{ text: `  ${m.signature}`, dim: true }] : [])],
    [{ text: '' }],
    kv('工期', `${m.spanDays} 天   ${m.range.from} → ${m.range.to}`),
    kv('提交', `${fmt(m.commits)}`),
    kv('代码', `写了 ${fmt(m.linesAdded)} 行，现在还在 ${fmt(m.linesSurviving)} 行`),
    ...(m.tokens ? [kv('Token', compact(m.tokens))] : []),
    [{ text: '' }],
    ...ranked.map((r, i) => [
      { text: r.name.padEnd(w + (r.key === 'unattributed' ? -1 : 0), ' '), dim: r.key === 'unattributed' },
      { text: blocks(r.pct), color: r.key === 'unattributed' ? undefined : r.key, rank: i, dim: r.key === 'unattributed' },
      { text: `  ${pct(r.pct)}`, dim: true },
    ]),
    ...(m.humans.length ? [[{ text: '人'.padEnd(w, ' '), dim: true }, { text: m.humans.map((x) => `${x.name} ${pct(x.pct)}`).join('　'), dim: true }] as Seg[]] : []),
  ];
}

/** 秒级时间戳 → 本机时区的「YYYY-MM-DD HH:mm」（以前写死了 UTC+8） */
export function localTime(sec: number, offsetMin = -new Date(sec * 1000).getTimezoneOffset()): string {
  return new Date((sec + offsetMin * 60) * 1000).toISOString().replace('T', ' ').slice(0, 16);
}

/** creditia blame */
export function blameSegs(b: any): Seg[][] {
  const who = b.harness ? nameOf(b.harness) : b.name ?? nameOf('unattributed');
  const t = b.sessionTime ? localTime(b.sessionTime) : '';
  const ev: Record<string, string> = { verified: '编辑成功记录的路径、行哈希和时间匹配', declared: '提交盖章或明确的工具署名' , unattributed: '认不出来' };
  const kv = (k: string, v: Seg[]): Seg[] => [{ text: k.padEnd(6, ' '), dim: true }, ...v];
  return [
    kv('作者', [{ text: who, color: b.harness, bold: true }, ...(b.model ? [{ text: `  ${b.model}`, dim: true }] : [])]),
    ...(b.session ? [kv('会话', [{ text: `${b.session.slice(0, 8)}  ` }, { text: t, dim: true }])] : []),
    kv('提交', [{ text: `${b.commit}  ` }, { text: b.date ?? '', dim: true }]),
    kv('', [{ text: b.subject ?? '' }]),
    ...(b.unit ? [kv('PR', [{ text: `#${b.unit.replace('pr:', '')}  ` }, { text: clean(b.unitTitle ?? ''), dim: true }])] : []),
    kv('怎么认的', [{ text: ev[b.evidence] ?? '' }]),
  ];
}
