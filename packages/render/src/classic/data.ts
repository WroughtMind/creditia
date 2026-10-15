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
  /** 在各形式里写出主导者（默认 false） */
  director: boolean;
  lang: 'zh' | 'en';
}
export const DEFAULTS: Options = { director: false, lang: 'zh' };

export interface Model {
  project: string;
  director: string;
  range: { from: string; to: string };
  /** 排行榜：agent（含「其他」）+ 合并的「人」+「未确认」。人不单独参与 agent 排名 */
  rows: Contributor[];
  /** 每个非主导的人，用来在署名处单独写（不与 agent 同级） */
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
  'claude-code': 'Claude Code', codex: 'Codex', cursor: 'Cursor', grok: 'Grok', kimi: 'Kimi', gemini: 'Gemini CLI', qwen: 'Qwen Code',
  opencode: 'OpenCode', cline: 'Cline', roo: 'Roo Code', kilo: 'Kilo Code', copilot: 'GitHub Copilot', aider: 'Aider', devin: 'Devin',
  jules: 'Jules', amp: 'Amp', windsurf: 'Windsurf', trae: 'Trae', crush: 'Crush', goose: 'Goose', continue: 'Continue', zed: 'Zed', 'amazon-q': 'Amazon Q', deepseek: 'DeepSeek', 'github-actions': 'GitHub Actions',
  dependabot: 'Dependabot', renovate: 'Renovate', other: '其他', unattributed: '未确认',
};
const EN: Record<string, string> = { other: 'Others', unattributed: 'Unattributed' };
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
  const directors = new Set((c.directors ?? []).map((d: any) => d.id));
  const acc = new Map<string, Contributor>();
  const modelLines = new Map<string, Map<string, number>>();
  for (const b of c.breakdown ?? []) {
    if (directors.has(b.contributor)) continue;
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
  // 署名只写 AI：人写的行不进分母，也不单独成行
  const total = [...acc.values()].filter((x) => x.kind !== 'human').reduce((s, x) => s + x.lines, 0);
  for (const e of acc.values()) {
    e.pct = total ? (e.lines / total) * 100 : 0;
    e.models = [...(modelLines.get(e.key) ?? new Map()).entries()].sort((a, b) => b[1] - a[1]).map(([m]) => (m === 'auto' ? 'Auto' : m));
  }
  const unknown = acc.get('unattributed') ?? null;
  const agentsAll = [...acc.values()].filter((x) => x.kind === 'agent').sort((a, b) => b.pct - a.pct);
  // agent 排百分比：最多 6 个具名，其余并入「其他」
  const shown = agentsAll.filter((x) => x.pct >= 0.1).slice(0, 6);
  const rest = agentsAll.filter((x) => !shown.includes(x));
  const rows: Contributor[] = [...shown];
  const restPct = rest.reduce((s, x) => s + x.pct, 0);
  if (restPct >= 0.1) rows.push({ key: 'other', name: nameOf('other', opt.lang), pct: restPct, lines: rest.reduce((s, x) => s + x.lines, 0), models: [], kind: 'other' });
  if (unknown && unknown.pct >= 0.05) rows.push({ ...unknown, name: nameOf('unattributed', opt.lang) });

  const range = c.project?.range ?? {};
  const span = range.from && range.to ? Math.round((Date.parse(range.to) - Date.parse(range.from)) / 86400000) + 1 : 0;
  const t = c.totals ?? {};
  const reviewers = contribs
    .filter((x) => x.kind === 'agent' && (x.roles ?? []).length === 1 && x.roles[0] === 'reviewer')
    .map((x) => ({ key: x.harness, name: nameOf(x.harness, opt.lang), model: x.model ?? '' }));
  return {
    project: c.project?.name ?? '',
    // 不写任何人的名字：主导者和个人名单都不出现，人写的行数只作为一行合计
    director: '',
    range: { from: range.from ?? '', to: range.to ?? '' },
    rows,
    humans: [],
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
