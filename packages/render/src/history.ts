// 贡献柱状图的共同口径：有版本节点按版本，没有则按月；旧档案只有周明细时保留周粒度。
// 时间分组不写入 versions，也不冒充发布版本。人的署名不参与代码占比。
export type HistoryBasis = 'versions' | 'months' | 'weeks';
export interface HistoryBucket { tag: string; date: string; by: Record<string, number>; total: number }
export interface ContributionHistory { basis: HistoryBasis; buckets: HistoryBucket[]; missing: boolean }

const positive = (v: unknown): number => typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : 0;
const tagsOf = (raw: any): { tag: string; date: string }[] => [...new Map<string, { tag: string; date: string }>(
  (raw?.versions ?? []).filter((v: any) => typeof v?.tag === 'string' && v.tag && v.tag !== 'unreleased').map((v: any) => [v.tag, { tag: v.tag, date: v.date ?? '' }]),
).values()];
const datedDays = (raw: any): { date: string; by: Record<string, number> }[] => (raw?.days ?? []).filter((d: any) =>
  typeof d?.date === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(d.date) && Number.isFinite(Date.parse(d.date)) && new Date(d.date).toISOString().slice(0, 10) === d.date);
const datedWeeks = (raw: any): { week: string; byContributor: Record<string, number> }[] => (raw?.timeline ?? []).filter((w: any) => typeof w?.week === 'string' && /^\d{4}-W(0[1-9]|[1-4]\d|5[0-3])$/.test(w.week));

export function historyBasis(raw: any): HistoryBasis {
  if (tagsOf(raw).length) return 'versions';
  return !datedDays(raw).length && datedWeeks(raw).length ? 'weeks' : 'months';
}

function aggregate(raw: any, keyOf: (id: string) => string) {
  const known = new Set<string>((raw?.contributors ?? []).map((c: any) => c.id));
  const people = new Set<string>((raw?.contributors ?? []).filter((c: any) => c.kind === 'human').map((c: any) => c.id));
  for (const d of raw?.directors ?? []) people.add(d.id);
  return (into: Record<string, number>, id: string, value: unknown) => {
    const v = positive(value);
    if (!v || people.has(id) || id.startsWith('h:')) return;
    const key = known.has(id) || id.includes(':') ? keyOf(id) : id;
    if (key === 'human' || key.startsWith('h:')) return;
    into[key] = (into[key] ?? 0) + v;
  };
}
const finish = (tag: string, date: string, by: Record<string, number>): HistoryBucket => ({ tag, date, by, total: Object.values(by).reduce((sum, v) => sum + v, 0) });

/** 发布说明等仍只读取版本统计，不能把按月的柱子当成发布版本。 */
export function versionBuckets(raw: any, keyOf: (id: string) => string): HistoryBucket[] {
  const add = aggregate(raw, keyOf);
  return [...tagsOf(raw), { tag: 'unreleased', date: '' }].map(({ tag, date }) => {
    const by: Record<string, number> = {};
    for (const b of raw?.breakdown ?? []) add(by, b.contributor, b.byVersion?.[tag]);
    return finish(tag, date, by);
  }).filter((b) => b.total > 0);
}

export function contributionHistory(raw: any, keyOf: (id: string) => string): ContributionHistory {
  const basis = historyBasis(raw);
  if (basis === 'versions') return {
    basis, buckets: versionBuckets(raw, keyOf),
    missing: !(raw?.breakdown ?? []).some((b: any) => b.byVersion && Object.keys(b.byVersion).length),
  };
  const add = aggregate(raw, keyOf), groups = new Map<string, Record<string, number>>();
  const points = basis === 'months' ? datedDays(raw).map((d) => ({ tag: d.date.slice(0, 7), by: d.by })) : datedWeeks(raw).map((w) => ({ tag: w.week, by: w.byContributor }));
  for (const { tag, by } of points) {
    const into = groups.get(tag) ?? {};
    for (const [id, value] of Object.entries(by ?? {})) add(into, id, value);
    groups.set(tag, into);
  }
  return { basis, buckets: [...groups].sort(([a], [b]) => a.localeCompare(b)).map(([tag, by]) => finish(tag, '', by)).filter((b) => b.total > 0), missing: !points.length };
}

export function historyLabels(basis: HistoryBasis, lang: 'zh' | 'en' = 'zh') {
  const zh = lang === 'zh';
  return {
    name: basis === 'versions' ? (zh ? '版本柱状图' : 'Versions') : (zh ? '开发历程' : 'Development history'),
    kicker: basis === 'versions' ? (zh ? '按版本' : 'BY VERSION') : basis === 'months' ? (zh ? '按月' : 'BY MONTH') : (zh ? '按周' : 'BY WEEK'),
    caption: basis === 'versions' ? (zh ? '各版本新增代码' : 'Lines added per version') : basis === 'months' ? (zh ? '每月新增代码' : 'Lines added per month') : (zh ? '每周新增代码' : 'Lines added per week'),
  };
}
export function historyEmpty(history: ContributionHistory, lang: 'zh' | 'en' = 'zh'): string {
  if (!history.missing) return lang === 'zh' ? '暂无可展示的代码贡献' : 'No code contributions to display yet';
  return history.basis === 'versions'
    ? (lang === 'zh' ? '档案缺少版本明细，请重新分析项目' : 'Version details are missing. Reanalyze the project.')
    : (lang === 'zh' ? '档案缺少时间明细，请重新分析项目' : 'Time details are missing. Reanalyze the project.');
}
