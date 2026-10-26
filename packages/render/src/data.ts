import { contributionHistory, versionBuckets, type ContributionHistory } from './history.ts';
// credits.json → 展示模型。所有作品只读这一份。
//
// 口径：
// - 分母是全部新增行：工具、贡献者、未确认加起来 100%；
// - agent 按工具（harness）归组排名，组内列出模型及各自行数；
// - 别人的提交算那位贡献者，写作者名（档案里没有名字时合成「其他贡献者」）。不说是谁手写的：
//   他的提交里没有 AI 线索，不等于他没用 AI；
// - 未确认单独一行，永远不并进任何人名下。

export type Lang = 'zh' | 'en';

export interface ModelLine { id: string; name: string; lines: number; share: number }
export interface Agent {
  key: string;
  name: string;
  /** 所属工具（harness）；按模型分组时 key 是「工具/模型」，这里仍是工具 */
  tool: string;
  /** 按模型分组时这一行的模型 id */
  model?: string;
  /** 工具的显示名 */
  toolName: string;
  /** 同一工具下第几个模型（按行数排），用来在品牌色上分出深浅 */
  shade: number;
  /** person：别人的提交（贡献者）；people：折叠或不写名字时合成的「其他贡献者」 */
  kind: 'agent' | 'bot' | 'other' | 'unknown' | 'person' | 'people';
  lines: number;
  pct: number;
  surviving: number;
  /** 留存率：至今仍在 / 新增 */
  survival: number;
  /** 有逐行证据的比例 */
  verified: number;
  /** 按证据等级的行数：逐行证据、明确声明、推断（旧档案没有时都是 0） */
  proof?: { verified: number; declared: number; inferred: number };
  models: ModelLine[];
  providers: string[];
  sessions: number;
  tokens: number;
  first: string;
  last: string;
  rank: number;
  /** 折叠行（其他、其他贡献者）里的各项：提示框里列出全部 */
  members?: Agent[];
}
export interface Person { key: string; name: string; lines: number; pct: number; surviving: number }
/** 按贡献者看：一位提交者的提交里，各工具、模型、他本人（没有 AI 记录）和未确认各多少。pct 都是占全部新增行的比例 */
export interface AuthorPart { key: string; name: string; lines: number; pct: number }
export interface Author { key: string; name: string; me: boolean; lines: number; pct: number; parts: AuthorPart[] }

export interface Hand {
  /** 发出的指令（会话里人发的消息） */
  prompts: number;
  /** 人做的 review */
  reviews: number;
  /** 其他贡献者提交里、没有 agent 证据的行 */
  handLines: number;
  /** Tab 补全辅助的行 */
  tabLines: number;
  /** 写在文档里的规划行 */
  plans: number;
  /** agent 在会话里写下的行 → 提交进仓库的行 → 至今仍在的行：人做的取舍 */
  written: number;
  committed: number;
  surviving: number;
  /** 开过的会话数 */
  sessions: number;
}

/** 分组方式：按工具，或按模型（同一工具的不同模型各成一行）；auto 按数据自动选 */
export type Group = 'tool' | 'model';
export type GroupOpt = Group | 'auto';

export interface Model {
  project: string;
  lang: Lang;
  /** 实际用的分组方式 */
  group: Group;
  /** 分组方式是自动选的 */
  groupAuto: boolean;
  /** 按工具的排名（不论分组方式都有）：分段、分组这类版式用它，组内模型在 models 里 */
  tools: Agent[];
  /** 贡献者 id → 当前分组 key */
  keyOf: (id: string) => string;
  range: { from: string; to: string };
  spanDays: number;
  activeDays: number;
  /** 全部 agent（含 bot），按行数排序 */
  agents: Agent[];
  /** 全部贡献方：agent、贡献者、未确认，不折叠。文字类作品（表格、评论）用它 */
  all: Agent[];
  /** 排名用：版面只放得下 ROW_SLOTS 个具名项，其余折叠成「其他」「其他贡献者」，再加未确认 */
  rows: Agent[];
  unknown: Agent | null;
  people: Person[];
  /** 按贡献者看（档案的 authors）；旧档案没有时是空的 */
  authors: Author[];
  /** 写出贡献者的名字（档案里有名字时；默认写） */
  named: boolean;
  /** 署名：用户主动填写时才有 */
  sign: string;
  hand: Hand;
  /** 作品里写不写开发者这一侧（发了多少消息、审阅几次）。默认不写 */
  showHand: boolean;
  commits: number;
  units: number;
  linesAdded: number;
  linesSurviving: number;
  tokens: number;
  sessions: number;
  verifiedShare: number;
  versions: { tag: string; date: string; by: Record<string, number>; total: number }[];
  history: ContributionHistory;
  weeks: { week: string; by: Record<string, number> }[];
  days: { date: string; by: Record<string, number> }[];
  clock: number[][];
  /** runs：按行顺序的连续分段 [分组 key, 行数]（人的行合并成 human）；旧档案没有 */
  files: { path: string; lines: number; by: Record<string, number>; runs?: [string, number][] }[];
  /** 板块 → 分组 key → 行数（核心、界面、测试、文档……） */
  areas: Record<string, Record<string, number>>;
  timeline: { key: string; start: number; end: number; tokens: number; review: boolean; prompts: number }[];
  units: any[];
  reviewers: { key: string; name: string; model: string; reviews: number }[];
  raw: any;
}

export interface Options {
  lang: Lang;
  /** 写出贡献者的名字（档案里有才有；false 时都合成「其他贡献者」） */
  named: boolean;
  /** 署名（用户主动填写） */
  sign: string;
  /** 分组方式，默认 auto */
  group?: GroupOpt;
}
export const DEFAULTS: Options = { lang: 'zh', named: true, sign: '', group: 'auto' };
/** 排名版面上的具名项数：卡片、榜单一行一项，再多就挤不下；折叠掉的在提示框里列全 */
export const ROW_SLOTS = 6;

// ───────────────────────── 名称 ─────────────────────────

export const TOOL_NAMES: Record<string, string> = {
  'claude-code': 'Claude Code', codex: 'Codex', cursor: 'Cursor', grok: 'Grok', kimi: 'Kimi', gemini: 'Gemini CLI', qwen: 'Qwen Code',
  opencode: 'OpenCode', cline: 'Cline', roo: 'Roo Code', kilo: 'Kilo Code', copilot: 'GitHub Copilot', aider: 'Aider', devin: 'Devin',
  jules: 'Jules', amp: 'Amp', windsurf: 'Windsurf', trae: 'Trae', crush: 'Crush', goose: 'Goose', continue: 'Continue', zed: 'Zed', 'amazon-q': 'Amazon Q', antigravity: 'Antigravity', deepseek: 'DeepSeek', 'github-actions': 'GitHub Actions',
  dependabot: 'Dependabot', renovate: 'Renovate',
};
const BOTS = new Set(['github-actions', 'dependabot', 'renovate']);
const ZH: Record<string, string> = { other: '其他', unattributed: '未确认', human: '贡献者', hand: '贡献者', people: '其他贡献者', me: '项目作者', noai: '没有 AI 记录' };
const EN: Record<string, string> = { other: 'Others', unattributed: 'Unattributed', human: 'Contributors', hand: 'Contributors', people: 'Other contributors', me: 'Maintainer', noai: 'No AI trace' };
export const nameOf = (k: string, lang: Lang = 'zh') => (lang === 'en' ? EN : ZH)[k] ?? TOOL_NAMES[k] ?? k;

/** 模型 id → 好读的名字：claude-opus-5-5 → Claude Opus 5.5，gpt-5.6-sol → GPT-5.6 Sol */
export function modelName(id: string): string {
  if (!id) return '';
  if (id === 'auto' || id === 'default') return 'Auto';
  const UP = new Set(['gpt', 'glm', 'gpt-oss', 'ai', 'r1', 'v3', 'o1', 'o3', 'o4', 'xl', 'vl']);
  const parts = id.split('-');
  const out: string[] = [];
  for (const p of parts) {
    const prev = out[out.length - 1];
    if (/^\d+(\.\d+)?$/.test(p) && prev && /\d$/.test(prev) && !/^\d{6,}$/.test(p)) { out[out.length - 1] = `${prev}.${p}`; continue; }
    if (/^\d{8}$/.test(p)) continue; // 日期后缀
    if (UP.has(p)) out.push(p.toUpperCase());
    else if (/^k\d/.test(p)) out.push(p.toUpperCase());
    else out.push(p.charAt(0).toUpperCase() + p.slice(1));
  }
  // GPT 系列习惯写成 GPT-5.6
  return out.join(' ').replace(/^GPT (\d)/, 'GPT-$1');
}

// ───────────────────────── 分组 ─────────────────────────

/** 贡献者 id → (工具, 模型)；人、未确认返回 null */
export function agentOf(c: any, id: string): { tool: string; model: string } | null {
  const x = (c.contributors ?? []).find((q: any) => q.id === id);
  if (id === 'unattributed' || x?.kind === 'unattributed') return null;
  if (x?.harness) return { tool: x.harness, model: x.model ?? '' };
  if (id.startsWith('a:')) { const [tool, ...rest] = id.slice(2).split('/'); return { tool, model: rest.join('/') }; }
  return null;
}

/** 每个工具下各模型的行数（不含机器人） */
function toolModels(c: any): Map<string, Map<string, number>> {
  const out = new Map<string, Map<string, number>>();
  for (const b of c.breakdown ?? []) {
    const a = agentOf(c, b.contributor);
    if (!a || BOTS.has(a.tool)) continue;
    const mm = out.get(a.tool) ?? new Map<string, number>();
    mm.set(a.model, (mm.get(a.model) ?? 0) + n(b.linesAdded));
    out.set(a.tool, mm);
  }
  return out;
}

/** 分组方式：auto 就是按工具（模型列在工具下面）；按模型分组由用户选。不再按占比自动切换 */
export function resolveGroup(_c: any, g: GroupOpt = 'auto'): Group {
  return g === 'model' ? 'model' : 'tool';
}

/** 按模型分组时同一工具下各模型的序号：品牌色按它分深浅（两套渲染器共用） */
export const SHADES = new Map<string, number>();

/** 按模型分组：key 是「工具/模型」，没记模型的仍是工具；同一个模型出现在几个工具下时名字后面带上工具 */
export function modelGroups(c: any, lang: Lang) {
  const tm = toolModels(c);
  const toolsOf = new Map<string, Set<string>>();
  for (const [tool, mm] of tm) for (const m of mm.keys()) if (m) { const set = toolsOf.get(m) ?? new Set(); set.add(tool); toolsOf.set(m, set); }
  const shade = new Map<string, number>();
  for (const [tool, mm] of tm) [...mm.entries()].filter(([m]) => m).sort((a, b) => b[1] - a[1]).forEach(([m], i) => shade.set(`${tool}/${m}`, i));
  for (const [k, i] of shade) SHADES.set(k, i);
  const keyOf = (id: string): string | null => {
    const a = agentOf(c, id);
    if (!a) return null;
    return a.model && !BOTS.has(a.tool) ? `${a.tool}/${a.model}` : a.tool;
  };
  const name = (key: string): string => {
    const i = key.indexOf('/');
    if (i < 0) {
      const named = [...(tm.get(key)?.keys() ?? [])].some((m) => m);
      return nameOf(key, lang) + (named ? (lang === 'zh' ? '（未记录模型）' : ' (model not recorded)') : '');
    }
    const tool = key.slice(0, i), m = key.slice(i + 1);
    return modelName(m) + ((toolsOf.get(m)?.size ?? 0) > 1 ? ` · ${nameOf(tool, lang)}` : '');
  };
  return { keyOf, name, shade: (key: string) => shade.get(key) ?? 0 };
}

// ───────────────────────── 构建 ─────────────────────────

const n = (x: unknown) => (typeof x === 'number' && isFinite(x) && x > 0 ? x : 0);
/** 档案的时区偏移（秒）：分析时按项目定下的；旧档案没有这个字段，当时按 +08:00 算的 */
export const tzOf = (c: any) => (typeof c?.tzOffset === 'number' ? c.tzOffset : 8 * 3600);
const dayAt = (ts: number, off: number) => new Date((ts + off) * 1000).toISOString().slice(0, 10);

export function buildModel(c: any, opt: Partial<Options> = {}): Model {
  const o = { ...DEFAULTS, ...opt };
  const lang = o.lang;
  const contribs: any[] = c.contributors ?? [];
  const byId = new Map(contribs.map((x) => [x.id, x]));
  const group = resolveGroup(c, o.group);
  const mg = group === 'model' ? modelGroups(c, lang) : null;
  const toolOfKey = (k: string) => k.split('/')[0];
  /** 贡献者 id → 展示分组 key */
  const groupOf = (id: string): { key: string; kind: 'agent' | 'bot' | 'human' | 'unknown'; name?: string } => {
    const x = byId.get(id);
    if (id === 'unattributed' || x?.kind === 'unattributed') return { key: 'unattributed', kind: 'unknown' };
    if (x?.harness) return { key: mg?.keyOf(id) ?? x.harness, kind: BOTS.has(x.harness) || x.kind === 'bot' ? 'bot' : 'agent' };
    if (id.startsWith('a:')) { const h = id.slice(2).split('/')[0]; return { key: mg?.keyOf(id) ?? h, kind: BOTS.has(h) ? 'bot' : 'agent' }; }
    // 贡献者按名字归并（同一个人用了几个邮箱只出一行）；档案里没有名字、或者选了不写名字，合成「其他贡献者」
    const name = o.named ? x?.name : undefined;
    return name ? { key: `h:${name.toLowerCase().trim()}`, kind: 'human', name } : { key: 'people', kind: 'human', name: nameOf('people', lang) };
  };

  const agentsBy = new Map<string, Agent>();
  const peopleBy = new Map<string, Person>();
  const modelLines = new Map<string, Map<string, number>>();
  const verifiedBy = new Map<string, number>();
  let verifiedAll = 0;
  for (const b of c.breakdown ?? []) {
    const gr = groupOf(b.contributor);
    const lines = n(b.linesAdded), surv = n(b.linesSurviving);
    const ver = n(b.byEvidence?.verified?.linesAdded);
    verifiedAll += ver;
    if (gr.kind === 'human') {
      const p = peopleBy.get(gr.key) ?? { key: gr.key, name: gr.name ?? '', lines: 0, pct: 0, surviving: 0 };
      p.lines += lines; p.surviving += surv;
      peopleBy.set(gr.key, p);
      continue;
    }
    const tool = toolOfKey(gr.key), isModel = gr.key.includes('/');
    const a = agentsBy.get(gr.key) ?? { key: gr.key, name: mg ? mg.name(gr.key) : nameOf(gr.key, lang), tool, toolName: nameOf(tool, lang), model: isModel ? gr.key.slice(tool.length + 1) : undefined, shade: mg ? mg.shade(gr.key) : 0, kind: gr.kind === 'unknown' ? 'unknown' : gr.kind, lines: 0, pct: 0, surviving: 0, survival: 0, verified: 0, models: [], providers: [], sessions: 0, tokens: 0, first: '', last: '', rank: 0 } as Agent;
    a.lines += lines; a.surviving += surv;
    verifiedBy.set(gr.key, (verifiedBy.get(gr.key) ?? 0) + ver);
    const pf = (a.proof ??= { verified: 0, declared: 0, inferred: 0 });
    pf.verified += ver; pf.declared += n(b.byEvidence?.declared?.linesAdded); pf.inferred += n(b.byEvidence?.inferred?.linesAdded);
    agentsBy.set(gr.key, a);
    const model = byId.get(b.contributor)?.model;
    if (gr.kind === 'agent' && model) {
      const mm = modelLines.get(gr.key) ?? new Map();
      mm.set(model, (mm.get(model) ?? 0) + lines);
      modelLines.set(gr.key, mm);
    }
    const prov = byId.get(b.contributor)?.provider;
    if (prov && !a.providers.includes(prov)) a.providers.push(prov);
  }
  // 分母是全部新增行：工具、贡献者、未确认
  const total = [...agentsBy.values(), ...peopleBy.values()].reduce((s, x) => s + x.lines, 0);
  const pctOf = (x: number) => (total ? (x / total) * 100 : 0);

  // 会话：次数、token、首末日期
  const timeline = (c.sessions ?? []).map((s: any) => ({ key: groupOf(s.contributor).key, start: n(s.start), end: n(s.end), tokens: n(s.tokens), review: !!s.review, prompts: n(s.prompts) }));
  for (const s of timeline) {
    const a = agentsBy.get(s.key);
    if (!a || s.review) continue;
    a.sessions++; a.tokens += s.tokens;
    const d0 = dayAt(s.start, tzOf(c)), d1 = dayAt(s.end, tzOf(c));
    if (!a.first || d0 < a.first) a.first = d0;
    if (!a.last || d1 > a.last) a.last = d1;
  }
  for (const a of agentsBy.values()) {
    a.pct = pctOf(a.lines);
    a.survival = a.lines ? Math.min(1, a.surviving / a.lines) : 0;
    a.verified = a.lines ? Math.min(1, (verifiedBy.get(a.key) ?? 0) / a.lines) : 0;
    const mm = [...(modelLines.get(a.key) ?? new Map()).entries()].sort((x, y) => y[1] - x[1]);
    const mt = mm.reduce((s, [, v]) => s + v, 0) || 1;
    // 按模型分组时一行就是一个模型，组内不再列模型
    a.models = a.model ? [] : mm.map(([id, v]) => ({ id, name: modelName(id), lines: v, share: v / mt }));
  }
  for (const p of peopleBy.values()) p.pct = pctOf(p.lines);

  const unknown = agentsBy.get('unattributed') ?? null;
  if (unknown) { unknown.name = nameOf('unattributed', lang); unknown.rank = 99; }
  const agents = [...agentsBy.values()].filter((x) => x.kind === 'agent' || x.kind === 'bot').sort((a, b) => b.lines - a.lines);
  agents.forEach((a, i) => (a.rank = i));
  const people = [...peopleBy.values()].sort((a, b) => b.lines - a.lines);
  // 贡献者也是排名里的一行：颜色用风格给「人」的那个色，几位贡献者按 shade 分深浅
  const persons: Agent[] = people.map((p, i) => ({
    key: p.key, name: p.name, tool: p.key, toolName: p.name, shade: i, kind: p.key === 'people' ? 'people' : 'person', lines: p.lines, pct: p.pct, surviving: p.surviving,
    survival: p.lines ? Math.min(1, p.surviving / p.lines) : 0, verified: 0, models: [], providers: [], sessions: 0, tokens: 0, first: '', last: '', rank: i,
  }));
  const all = [...agents, ...persons].sort((a, b) => b.lines - a.lines);
  // 具名项按行数排；版面放不下的折叠，折叠行带上全部成员
  const shown = all.filter((x) => x.kind !== 'people').slice(0, ROW_SLOTS);
  const rows: Agent[] = [...shown];
  const fold = (key: 'other' | 'people', xs: Agent[]): Agent => {
    // 只剩一项时直接放它自己：折叠成「其他」省不了地方
    if (xs.length === 1) return xs[0];
    const lines = xs.reduce((s, x) => s + x.lines, 0);
    const sum = (k: 'verified' | 'declared' | 'inferred') => xs.reduce((s, x) => s + (x.proof?.[k] ?? 0), 0);
    return {
      key, name: nameOf(key, lang), tool: key, toolName: nameOf(key, lang), shade: 0, kind: key, lines, pct: pctOf(lines), surviving: xs.reduce((s, x) => s + x.surviving, 0), survival: 0, verified: 0,
      proof: key === 'other' ? { verified: sum('verified'), declared: sum('declared'), inferred: sum('inferred') } : undefined, models: [], providers: [], sessions: xs.reduce((s, x) => s + x.sessions, 0), tokens: 0, first: '', last: '', rank: rows.length, members: xs,
    };
  };
  const restA = agents.filter((x) => !shown.includes(x)), restP = persons.filter((x) => !shown.includes(x));
  if (restA.length) rows.push(fold('other', restA));
  if (restP.length) rows.push(fold('people', restP));
  if (unknown && unknown.lines) rows.push(unknown);
  // 分组 key → 排名里那一行的 key：折叠掉的工具、贡献者在图表里也算进折叠行
  const rowKey = new Map<string, string>();
  for (const r of rows) for (const x of r.members ?? [r]) rowKey.set(x.key, r.key);
  const keyOf = (id: string) => { const k = groupOf(id).key; return rowKey.get(k) ?? k; };
  const hu = c.human ?? {};
  const t = c.totals ?? {};
  const hand: Hand = {
    prompts: n(hu.prompts), reviews: n(hu.reviews), handLines: people.reduce((s, p) => s + p.lines, 0), tabLines: n(hu.tabAssistedLines), plans: n(hu.plans),
    written: n(t.linesWritten), committed: n(t.linesAdded), surviving: n(t.linesSurviving), sessions: n(t.sessions),
  };

  // 按贡献者看：只说是谁提交的。他本人那份是「没有 AI 记录」，不说是他手写的
  const authorsBy = new Map<string, Author>();
  for (const a of c.authors ?? []) {
    const named = o.named && typeof a.name === 'string' && a.name ? a.name : '';
    const key = a.me ? 'me' : named ? `h:${named.toLowerCase().trim()}` : 'people';
    const au: Author = authorsBy.get(key) ?? { key, name: named || nameOf(a.me ? 'me' : 'people', lang), me: !!a.me, lines: 0, pct: 0, parts: [] };
    for (const [id, v] of Object.entries(a.by ?? {})) {
      const gr = groupOf(id);
      const pk = gr.kind === 'human' ? 'noai' : gr.key;
      let part = au.parts.find((p) => p.key === pk);
      if (!part) au.parts.push((part = { key: pk, name: pk === 'noai' || pk === 'unattributed' ? nameOf(pk, lang) : agentsBy.get(pk)?.name ?? nameOf(pk, lang), lines: 0, pct: 0 }));
      part.lines += n(v); au.lines += n(v);
    }
    authorsBy.set(key, au);
  }
  const authors = [...authorsBy.values()].filter((a) => a.lines > 0).sort((a, b) => b.lines - a.lines);
  for (const a of authors) {
    a.pct = pctOf(a.lines);
    for (const p of a.parts) p.pct = pctOf(p.lines);
    // 工具按行数排，没有 AI 记录、未确认放最后
    const tail = (k: string) => (k === 'noai' ? 1 : k === 'unattributed' ? 2 : 0);
    a.parts.sort((x, y) => tail(x.key) - tail(y.key) || y.lines - x.lines);
  }

  // 按版本、按周、按天：都换成分组 key
  const versions = versionBuckets(c, keyOf);
  const history = contributionHistory(c, keyOf);
  // 旧档案按天、按文件的统计里可能只写了工具名（不带 a:），原样当分组 key
  const rowOf = (id: string) => { if (id.includes(':') || byId.has(id) || id === 'unattributed') return keyOf(id); return rowKey.get(id) ?? id; };
  const regroup = (by: Record<string, number>) => {
    const out: Record<string, number> = {};
    for (const [id, v] of Object.entries(by ?? {})) { const k = rowOf(id); out[k] = (out[k] ?? 0) + n(v); }
    return out;
  };
  const weeks = (c.timeline ?? []).map((w: any) => ({ week: w.week, by: regroup(w.byContributor) }));
  const days = (c.days ?? []).map((d: any) => ({ date: d.date, by: regroup(d.by) }));
  const files = (c.files ?? []).map((f: any) => ({
    path: f.path ?? '', lines: n(f.lines), by: regroup(f.by),
    runs: Array.isArray(f.runs) ? f.runs.filter((x: any) => Array.isArray(x)).map(([k, v]: [string, number]) => [rowOf(String(k)), n(v)] as [string, number]) : undefined,
  }));

  const areas: Record<string, Record<string, number>> = {};
  for (const b of c.breakdown ?? []) {
    const k = keyOf(b.contributor);
    for (const [ar, v] of Object.entries(b.byArea ?? {})) { (areas[ar] ??= {})[k] = (areas[ar][k] ?? 0) + n(v); }
  }
  const range = c.project?.range ?? {};
  const span = range.from && range.to ? Math.round((Date.parse(range.to) - Date.parse(range.from)) / 86400000) + 1 : 0;
  const reviewers = contribs
    .filter((x) => x.kind === 'agent' && (x.roles ?? []).includes('reviewer') && !(x.roles ?? []).includes('author'))
    .map((x) => ({ key: x.harness, name: nameOf(x.harness, lang), model: x.model ?? '', reviews: n(x.reviews) }));
  // 旧档案里的 directors 只用来认出「本机用户」是人；不再生成「主导」字样
  const sign = o.sign || '';
  const tools = group === 'model' ? buildModel(c, { ...o, group: 'tool' }).rows : rows;
  return {
    project: c.project?.name ?? '', lang, group, groupAuto: (o.group ?? 'auto') === 'auto', tools, keyOf,
    range: { from: range.from ?? '', to: range.to ?? '' }, spanDays: span, activeDays: n(t.days),
    agents, all: unknown?.lines ? [...all, unknown] : all, rows, unknown, people, authors, named: o.named, sign, hand, showHand: false,
    commits: n(t.commits), units: n(t.units), linesAdded: n(t.linesAdded) || total, linesSurviving: n(t.linesSurviving), tokens: n(t.tokens), sessions: n(t.sessions),
    verifiedShare: total ? verifiedAll / total : 0,
    versions, history, weeks, days, clock: Array.isArray(c.clock) ? c.clock : [], files, areas, timeline, units: c.units ?? [], reviewers, raw: c,
  };
}

// ───────────────────────── 格式化 ─────────────────────────

export const fmt = (x: number) => Math.round(x).toLocaleString('en-US');
export const pct = (p: number) => (p > 0 && p < 0.1 ? '<0.1%' : `${p.toFixed(1)}%`);
export const pct0 = (p: number) => (p > 0 && p < 1 ? '<1%' : `${Math.round(p)}%`);
export function compact(x: number) {
  if (x >= 1e9) return `${(x / 1e9).toFixed(x >= 1e10 ? 0 : 1)}B`;
  if (x >= 1e6) return `${(x / 1e6).toFixed(x >= 1e7 ? 0 : 1)}M`;
  if (x >= 1e3) return `${(x / 1e3).toFixed(x >= 1e4 ? 0 : 1)}K`;
  return String(Math.round(x));
}
/** 中文数字：古籍、水墨风格用 */
export function hanzi(x: number): string {
  const D = '〇一二三四五六七八九';
  let v = Math.round(x);
  if (v === 0) return D[0];
  const units = ['', '十', '百', '千'];
  if (v >= 10000) return String(v).split('').map((d) => D[+d] ?? d).join('');
  const ds = String(v).split('').map(Number);
  let out = '', zero = false;
  ds.forEach((d, i) => {
    const u = units[ds.length - 1 - i];
    if (d === 0) { zero = out !== ''; return; }
    if (zero) { out += D[0]; zero = false; }
    out += (d === 1 && u === '十' && out === '' ? '' : D[d]) + u;
  });
  return out;
}
/** 年份、编号这类逐字读的数字 */
export const hanziDigits = (x: number | string) => String(x).split('').map((d) => '〇一二三四五六七八九'[+d] ?? d).join('');

/** 悬停提示的内容：每个贡献方一份 */
export function tipOf(m: Model, a: Agent): { k: string; t: string; d: string[] } {
  const zh = m.lang === 'zh';
  const head = zh ? `${fmt(a.lines)} 行 · ${pct(a.pct)}` : `${fmt(a.lines)} lines · ${pct(a.pct)}`;
  if (a.kind === 'unknown') return { k: a.key, t: a.name, d: [head, zh ? '找不到证据，不算到任何人头上' : 'No evidence for anyone, so not credited'] };
  if (a.kind === 'person') return { k: a.key, t: a.name, d: [head, zh ? '这位贡献者的提交里没有 AI 线索（不说明没用 AI）' : 'No AI trace in their commits (which doesn\'t mean no AI was used)'] };
  // 折叠行：列出全部成员
  if (a.members) return { k: a.key, t: a.name, d: [head, ...a.members.map((x) => `${x.name} ${pct(x.pct)}`)] };
  const d = [head];
  if (a.models.length) d.push(a.models.slice(0, 3).map((x) => `${x.name} ${Math.round(x.share * 100)}%`).join(' · '));
  if (a.model) d.push(zh ? `用 ${a.toolName}` : `via ${a.toolName}`);
  if (a.surviving) d.push(zh ? `现在还在 ${fmt(a.surviving)} 行（${Math.round(a.survival * 100)}%）` : `${fmt(a.surviving)} still in the code (${Math.round(a.survival * 100)}%)`);
  if (a.sessions) d.push(zh ? `${a.sessions} 个会话${a.tokens ? ` · ${compact(a.tokens)} token` : ''}` : `${a.sessions} sessions${a.tokens ? ` · ${compact(a.tokens)} tokens` : ''}`);
  if (a.verified) d.push(zh ? `${Math.round(a.verified * 100)}% 能在日志里对上` : `${Math.round(a.verified * 100)}% line-level evidence`);
  if (a.providers.length) d.push((zh ? '通过 ' : 'via ') + a.providers.join(' · '));
  return { k: a.key, t: a.name, d };
}
