// credits.json → 展示用模型。所有形式只读这一份模型。
import { resolveGroup, modelGroups, ROW_SLOTS, type Group, type GroupOpt } from '../data.ts';
import { TOOL_NAMES } from '../tool-names.ts';
import { contributionHistory, type ContributionHistory } from '../history.ts';

export interface Contributor {
  key: string;
  name: string;
  /** 按模型分组时这一行的模型 id（key 是「工具/模型」） */
  model?: string;
  pct: number;
  lines: number;
  /** 该工具下按新增行排序的模型 */
  models: string[];
  kind: 'agent' | 'human' | 'other' | 'unknown';
}

export interface Options {
  /** 在各形式里写出主导者（默认 false） */
  director: boolean;
  lang: 'zh' | 'en';
  /** 写出贡献者的名字（档案里有才有；默认写）。false 时合成「其他贡献者」 */
  named?: boolean;
  /** 分组方式，默认 auto（和新风格同一套规则） */
  group?: GroupOpt;
}
export const DEFAULTS: Options = { director: false, lang: 'zh', group: 'auto' };

export interface Model {
  project: string;
  named: boolean;
  lang: Options['lang'];
  history: ContributionHistory;
  director: string;
  group: Group;
  /** 贡献者 id（或旧档案里的工具名）→ 当前分组 key */
  keyOf: (id: string) => string;
  range: { from: string; to: string };
  /** 排行榜：agent（版面放得下的具名项，其余并成「其他」）+「未确认」 */
  rows: Contributor[];
  /** 贡献者（别人的提交）：和 agent 同一个分母，各版式在榜单下面单独写一行 */
  humans: Contributor[];
  agents: Contributor[];
  unknown: Contributor | null;
  commits: number;
  units: number;
  linesAdded: number;
  linesSurviving: number;
  tokens: number;
  sessions: number;
  activeDays: number;
  spanDays: number;
  reviewers: { key: string; name: string; model: string }[];
  raw: any;
}

export const NAMES: Record<string, string> = {
  ...TOOL_NAMES, other: '其他', unattributed: '未确认', people: '其他贡献者',
};
const EN: Record<string, string> = { other: 'Others', unattributed: 'Unattributed', people: 'Other contributors' };
export const nameOf = (k: string, lang: 'zh' | 'en' = 'zh') => (lang === 'en' && EN[k]) || NAMES[k] || k;

const n = (x: unknown) => (typeof x === 'number' && isFinite(x) && x > 0 ? x : 0);

export function buildModel(c: any, opt: Options = DEFAULTS): Model {
  const contribs: any[] = c.contributors ?? [];
  const byId = new Map(contribs.map((x) => [x.id, x]));
  const group = resolveGroup(c, opt.group);
  const mg = group === 'model' ? modelGroups(c, opt.lang) : null;
  const groupOf = (id: string) => {
    const x = byId.get(id);
    if (x?.harness) return { key: (mg?.keyOf(id) ?? x.harness) as string, kind: 'agent' as const };
    if (id === 'unattributed') return { key: 'unattributed', kind: 'unknown' as const };
    // 贡献者按名字归并：同一个人用了多个邮箱时只出一行；没有名字、或选了不写名字，合成「其他贡献者」
    const name = opt.named !== false ? x?.name : undefined;
    return name ? { key: `h:${name.toLowerCase().trim()}`, kind: 'human' as const, name } : { key: 'people', kind: 'human' as const, name: nameOf('people', opt.lang) };
  };
  const directors = new Set((c.directors ?? []).map((d: any) => d.id));
  const acc = new Map<string, Contributor>();
  const modelLines = new Map<string, Map<string, number>>();
  for (const b of c.breakdown ?? []) {
    if (directors.has(b.contributor)) continue;
    const g = groupOf(b.contributor);
    const e = acc.get(g.key) ?? { key: g.key, name: (g as any).name ?? (mg && g.kind === 'agent' ? mg.name(g.key) : nameOf(g.key, opt.lang)), model: g.key.includes('/') ? g.key.slice(g.key.indexOf('/') + 1) : undefined, pct: 0, lines: 0, models: [], kind: g.kind };
    e.lines += n(b.linesAdded);
    acc.set(g.key, e);
    const model = byId.get(b.contributor)?.model;
    if (g.kind === 'agent' && model) {
      const mm = modelLines.get(g.key) ?? new Map();
      mm.set(model, (mm.get(model) ?? 0) + n(b.linesAdded));
      modelLines.set(g.key, mm);
    }
  }
  // 分母是全部新增行：工具、贡献者、未确认加起来 100%（和新风格同一口径）
  const total = [...acc.values()].reduce((s, x) => s + x.lines, 0);
  for (const e of acc.values()) {
    e.pct = total ? (e.lines / total) * 100 : 0;
    // 按模型分组时一行就是一个模型：模型那一栏写它用的工具
    e.models = e.model ? [nameOf(e.key.split('/')[0], opt.lang)] : [...(modelLines.get(e.key) ?? new Map()).entries()].sort((a, b) => b[1] - a[1]).map(([m]) => (m === 'auto' ? 'Auto' : m));
  }
  const unknown = acc.get('unattributed') ?? null;
  const agentsAll = [...acc.values()].filter((x) => x.kind === 'agent').sort((a, b) => b.pct - a.pct);
  // 榜单版面放得下 ROW_SLOTS 个具名 agent，其余并入「其他」
  const shown = agentsAll.slice(0, ROW_SLOTS);
  const rest = agentsAll.filter((x) => !shown.includes(x));
  const rows: Contributor[] = [...shown];
  if (rest.length === 1) rows.push(rest[0]);
  else if (rest.length) rows.push({ key: 'other', name: nameOf('other', opt.lang), pct: rest.reduce((s, x) => s + x.pct, 0), lines: rest.reduce((s, x) => s + x.lines, 0), models: [], kind: 'other' });
  if (unknown?.lines) rows.push({ ...unknown, name: nameOf('unattributed', opt.lang) });
  const humans = [...acc.values()].filter((x) => x.kind === 'human').sort((a, b) => b.lines - a.lines);

  const range = c.project?.range ?? {};
  const span = range.from && range.to ? Math.round((Date.parse(range.to) - Date.parse(range.from)) / 86400000) + 1 : 0;
  const t = c.totals ?? {};
  const reviewers = contribs
    .filter((x) => x.kind === 'agent' && (x.roles ?? []).length === 1 && x.roles[0] === 'reviewer')
    .map((x) => ({ key: x.harness, name: nameOf(x.harness, opt.lang), model: x.model ?? '' }));
  const keyOf = (id: string) => {
    if (byId.has(id) || id.startsWith('a:')) return id.startsWith('a:') && !byId.has(id) ? (mg?.keyOf(id) ?? id.slice(2).split('/')[0]) : groupOf(id).key;
    return id;
  };
  return {
    project: c.project?.name ?? '', named: opt.named !== false, lang: opt.lang, history: contributionHistory(c, keyOf), group, keyOf,
    // 主导者不写；贡献者（别人的提交）写作者名，档案里没有名字时是「其他贡献者」
    director: '',
    range: { from: range.from ?? '', to: range.to ?? '' },
    rows,
    humans,
    agents: agentsAll,
    unknown,
    commits: n(t.commits), units: n(t.units), linesAdded: n(t.linesAdded), linesSurviving: n(t.linesSurviving),
    tokens: n(t.tokens), sessions: n(t.sessions), activeDays: n(t.days), spanDays: span,
    reviewers,
    raw: c,
  };
}

export const fmt = (x: number) => Math.round(x).toLocaleString('en-US');
export const pct = (p: number) => (p > 0 && p < 0.1 ? '<0.1%' : `${p.toFixed(1)}%`);
export function compact(x: number) {
  if (x >= 1e9) return `${(x / 1e9).toFixed(x >= 1e10 ? 0 : 1)}B`;
  if (x >= 1e6) return `${(x / 1e6).toFixed(x >= 1e7 ? 0 : 1)}M`;
  if (x >= 1e3) return `${(x / 1e3).toFixed(x >= 1e4 ? 0 : 1)}K`;
  return String(Math.round(x));
}
