// 文本形式：终端名片、逐行溯源。CREDITS.md、版本说明、PR 评论和命令行共用 ../text.ts（同一套口径）
import type { Model } from './data.ts';
import { fmt, pct, compact, nameOf } from './data.ts';
import type { Seg } from './forms.ts';

const clean = (s: string) => s.replace(/\s*\(#\d+\)\s*$/, '');

/** creditia fetch：agent 排百分比 */
export function fetchSegs(m: Model): Seg[][] {
  const w = 12;
  const kv = (k: string, v: string): Seg[] => [{ text: k.padEnd(8, ' '), dim: true }, { text: v }];
  const blocks = (p: number) => '■'.repeat(Math.max(1, Math.round(p / 3.5)));
  const ranked = m.rows.filter((r) => r.key !== 'human');
  return [
    [{ text: m.project, bold: true }, ...(m.director ? [{ text: `  ${m.director}`, dim: true }] : [])],
    [{ text: '' }],
    kv('工期', `${m.spanDays} 天   ${m.range.from} → ${m.range.to}`),
    kv('提交', `${fmt(m.commits)}`),
    kv('代码', `${fmt(m.linesAdded)} 行，留下 ${fmt(m.linesSurviving)}`),
    ...(m.tokens ? [kv('Token', compact(m.tokens))] : []),
    [{ text: '' }],
    ...ranked.map((r, i) => [
      { text: r.name.padEnd(w + (r.key === 'unattributed' ? -1 : 0), ' '), dim: r.key === 'unattributed' },
      { text: blocks(r.pct), color: r.key === 'unattributed' ? undefined : r.key, rank: i, dim: r.key === 'unattributed' },
      { text: `  ${pct(r.pct)}`, dim: true },
    ]),
    // 贡献者（别人的提交）：和上面同一个分母
    ...m.humans.map((r) => [{ text: r.name.padEnd(w, ' ') }, { text: blocks(r.pct), dim: true }, { text: `  ${pct(r.pct)}`, dim: true }]),
  ];
}

/** creditia blame */
export function blameSegs(b: any): Seg[][] {
  const who = b.harness ? nameOf(b.harness) : b.name ?? nameOf('unattributed');
  const t = b.sessionTime ? new Date((b.sessionTime + 8 * 3600) * 1000).toISOString().replace('T', ' ').slice(0, 16) : '';
  const ev: Record<string, string> = { verified: '日志里的编辑与这一行逐字对上', declared: '提交信息或会话时间吻合', inferred: '按分支名推断', unattributed: '没有证据' };
  const kv = (k: string, v: Seg[]): Seg[] => [{ text: k.padEnd(6, ' '), dim: true }, ...v];
  return [
    kv('作者', [{ text: who, color: b.harness, bold: true }, ...(b.model ? [{ text: `  ${b.model}`, dim: true }] : [])]),
    ...(b.session ? [kv('会话', [{ text: `${b.session.slice(0, 8)}  ` }, { text: t, dim: true }])] : []),
    kv('提交', [{ text: `${b.commit}  ` }, { text: b.date ?? '', dim: true }]),
    kv('', [{ text: b.subject ?? '' }]),
    ...(b.unit ? [kv('PR', [{ text: `#${b.unit.replace('pr:', '')}  ` }, { text: clean(b.unitTitle ?? ''), dim: true }])] : []),
    kv('证据', [{ text: ev[b.evidence] ?? '' }]),
  ];
}
