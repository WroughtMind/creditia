// credits.json → 展示用模型。所有形式只读这一份模型。
export interface Contributor {
  key: string;
  name: string;
  pct: number;
  lines: number;
  /** 该工具下按新增行排序的模型 */
  models: string[];
  kind: 'agent' | 'human' | 'other' | 'unknown';
}

export interface Options {
  /** 在各形式里写出人员（默认 false） */
  people: boolean;
  lang: 'zh' | 'en';
}
export const DEFAULTS: Options = { people: false, lang: 'zh' };

export interface Model {
  project: string;
  people: { name: string; roles: string[]; note?: string }[];
  signature: string;
  range: { from: string; to: string };
  /** 排行榜：agent（含「其他」）+ 合并的「人」+「未确认」。人不单独参与 agent 排名 */
  rows: Contributor[];
  /** 有代码记录且选择公开姓名的人，用来在署名处单独写（不与 agent 同级） */
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
  codex: 'Codex', cursor: 'Cursor', grok: 'Grok', 'claude-code': 'Claude Code', deepseek: 'DeepSeek', kimi: 'Kimi',
  gemini: 'Gemini', opencode: 'OpenCode', 'github-actions': 'GitHub Actions', other: '其他', unattributed: '未确认', human: '人',
};
const EN: Record<string, string> = { other: 'Others', unattributed: 'Unattributed', human: 'People' };
export const nameOf = (k: string, lang: 'zh' | 'en' = 'zh') => (lang === 'en' && EN[k]) || NAMES[k] || k;

const n = (x: unknown) => (typeof x === 'number' && isFinite(x) && x > 0 ? x : 0);

export function buildModel(c: any, opt: Options = DEFAULTS): Model {
  const contribs: any[] = c.contributors ?? [];
  const byId = new Map(contribs.map((x) => [x.id, x]));
  const groupOf = (id: string) => {
    const x = byId.get(id);
    if (x?.harness) return { key: x.harness as string, kind: 'agent' as const };
    if (id === 'unattributed') return { key: 'unattributed', kind: 'unknown' as const };
    // 人按显示名归并：同一个人用了多个邮箱时只出一行
    const name = x?.name ?? id.replace(/^h:/, '');
    return { key: `h:${name.toLowerCase().trim()}`, kind: 'human' as const, name };
  };
  const acc = new Map<string, Contributor>();
  const modelLines = new Map<string, Map<string, number>>();
  for (const b of c.breakdown ?? []) {
    const g = groupOf(b.contributor);
    const e = acc.get(g.key) ?? { key: g.key, name: (g as any).name ?? nameOf(g.key, opt.lang), pct: 0, lines: 0, models: [], kind: g.kind };
    e.lines += n(b.linesAdded);
    acc.set(g.key, e);
    const model = byId.get(b.contributor)?.model;
    if (g.kind === 'agent' && model) {
      const mm = modelLines.get(g.key) ?? new Map();
      mm.set(model, (mm.get(model) ?? 0) + n(b.linesAdded));
      modelLines.set(g.key, mm);
    }
  }
  const total = n(c.totals?.linesAdded) || [...acc.values()].reduce((s, x) => s + x.lines, 0);
  for (const e of acc.values()) {
    e.pct = total ? (e.lines / total) * 100 : 0;
    e.models = [...(modelLines.get(e.key) ?? new Map()).entries()].sort((a, b) => b[1] - a[1]).map(([m]) => (m === 'auto' ? 'Auto' : m));
  }
  const unknown = acc.get('unattributed') ?? null;
  const agentsAll = [...acc.values()].filter((x) => x.kind === 'agent').sort((a, b) => b.pct - a.pct);
  const humans = [...acc.values()].filter((x) => x.kind === 'human').sort((a, b) => b.pct - a.pct);
  const rows: Contributor[] = [...agentsAll];
  // 人不和 agent 同级：合并成一行放在「未确认」前面，个体名字在 humans 里单独写
  const humanPct = humans.reduce((s, x) => s + x.pct, 0);
  if (humanPct > 0) rows.push({ key: 'human', name: nameOf('human', opt.lang), pct: humanPct, lines: humans.reduce((s, x) => s + x.lines, 0), models: [], kind: 'human' });
  if (unknown && unknown.pct > 0) rows.push({ ...unknown, name: nameOf('unattributed', opt.lang) });

  const range = c.project?.range ?? {};
  const span = range.from && range.to ? Math.round((Date.parse(range.to) - Date.parse(range.from)) / 86400000) + 1 : 0;
  const t = c.totals ?? {};
  const reviewers = contribs
    .filter((x) => x.kind === 'agent' && (x.roles ?? []).includes('reviewer'))
    .map((x) => ({ key: x.harness, name: nameOf(x.harness, opt.lang), model: x.model ?? '' }));
  return {
    project: c.project?.name ?? '',
    signature: opt.people ? (c.people ?? []).map((p: any) => p.name).join(' · ') : '',
    people: opt.people ? (c.people ?? []) : [],
    range: { from: range.from ?? '', to: range.to ?? '' },
    rows,
    humans: opt.people ? humans : [],
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
