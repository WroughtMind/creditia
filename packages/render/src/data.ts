// credits.json → 展示模型。所有作品只读这一份。
//
// 口径：
// - agent 按工具（harness）归组排名，组内列出模型及各自行数；
// - 人不和 agent 抢百分比：人的贡献用「做了什么」来呈现（指令、审阅、亲手写的行、取舍），
//   名字默认不出现，只有用户明确选择署名才写；
// - 未确认单独一行，永远不并进任何人名下。

export type Lang = 'zh' | 'en';

export interface ModelLine { id: string; name: string; lines: number; share: number }
export interface Agent {
  key: string;
  name: string;
  kind: 'agent' | 'bot' | 'other' | 'unknown';
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
}
export interface Person { key: string; name: string; lines: number; pct: number; surviving: number }

export interface Hand {
  /** 发出的指令（会话里人发的消息） */
  prompts: number;
  /** 人做的 review */
  reviews: number;
  /** 亲手写下的行（人类提交中、没有 agent 证据的部分） */
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

export interface Model {
  project: string;
  lang: Lang;
  range: { from: string; to: string };
  spanDays: number;
  activeDays: number;
  /** 全部 agent（含 bot），按行数排序 */
  agents: Agent[];
  /** 排名用：最多 6 个具名 + 其他 + 未确认 */
  rows: Agent[];
  unknown: Agent | null;
  people: Person[];
  /** 是否写出人名（默认否） */
  named: boolean;
  /** 署名：用户主动填写时才有 */
  sign: string;
  hand: Hand;
  /** 作品里写不写人这一侧（发了多少消息、review、人写的行）。默认不写 */
  showHand: boolean;
  commits: number;
  units: number;
  linesAdded: number;
  linesSurviving: number;
  tokens: number;
  sessions: number;
  verifiedShare: number;
  versions: { tag: string; date: string; by: Record<string, number>; total: number }[];
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
  /** 写出人名 */
  named: boolean;
  /** 署名（用户主动填写） */
  sign: string;
}
export const DEFAULTS: Options = { lang: 'zh', named: false, sign: '' };

// ───────────────────────── 名称 ─────────────────────────

export const TOOL_NAMES: Record<string, string> = {
  'claude-code': 'Claude Code', codex: 'Codex', cursor: 'Cursor', grok: 'Grok', kimi: 'Kimi', gemini: 'Gemini CLI', qwen: 'Qwen Code',
  opencode: 'OpenCode', cline: 'Cline', roo: 'Roo Code', kilo: 'Kilo Code', copilot: 'GitHub Copilot', aider: 'Aider', devin: 'Devin',
  jules: 'Jules', amp: 'Amp', windsurf: 'Windsurf', trae: 'Trae', crush: 'Crush', goose: 'Goose', continue: 'Continue', zed: 'Zed', deepseek: 'DeepSeek', 'github-actions': 'GitHub Actions',
  dependabot: 'Dependabot', renovate: 'Renovate',
};
const BOTS = new Set(['github-actions', 'dependabot', 'renovate']);
const ZH: Record<string, string> = { other: '其他', unattributed: '未确认', human: '人', hand: '人' };
const EN: Record<string, string> = { other: 'Others', unattributed: 'Unattributed', human: 'People', hand: 'People' };
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

// ───────────────────────── 构建 ─────────────────────────

const n = (x: unknown) => (typeof x === 'number' && isFinite(x) && x > 0 ? x : 0);
const day = (ts: number) => new Date(ts * 1000 + 8 * 3600e3).toISOString().slice(0, 10);

export function buildModel(c: any, opt: Partial<Options> = {}): Model {
  const o = { ...DEFAULTS, ...opt };
  const lang = o.lang;
  const contribs: any[] = c.contributors ?? [];
  const byId = new Map(contribs.map((x) => [x.id, x]));
  /** 贡献者 id → 展示分组 key */
  const groupOf = (id: string): { key: string; kind: 'agent' | 'bot' | 'human' | 'unknown'; name?: string } => {
    const x = byId.get(id);
    if (id === 'unattributed' || x?.kind === 'unattributed') return { key: 'unattributed', kind: 'unknown' };
    if (x?.harness) return { key: x.harness, kind: BOTS.has(x.harness) || x.kind === 'bot' ? 'bot' : 'agent' };
    if (id.startsWith('a:')) { const h = id.slice(2).split('/')[0]; return { key: h, kind: BOTS.has(h) ? 'bot' : 'agent' }; }
    const name = x?.name ?? id.replace(/^h:/, '');
    return { key: `h:${name.toLowerCase().trim()}`, kind: 'human', name };
  };
  const keyOf = (id: string) => groupOf(id).key;

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
    const a = agentsBy.get(gr.key) ?? { key: gr.key, name: nameOf(gr.key, lang), kind: gr.kind === 'unknown' ? 'unknown' : gr.kind, lines: 0, pct: 0, surviving: 0, survival: 0, verified: 0, models: [], providers: [], sessions: 0, tokens: 0, first: '', last: '', rank: 0 } as Agent;
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
  // 署名只写 AI：占比的分母不含人写的行（和原版一致）
  const total = [...agentsBy.values()].reduce((s, x) => s + x.lines, 0);
  const pctOf = (x: number) => (total ? (x / total) * 100 : 0);

  // 会话：次数、token、首末日期
  const timeline = (c.sessions ?? []).map((s: any) => ({ key: keyOf(s.contributor), start: n(s.start), end: n(s.end), tokens: n(s.tokens), review: !!s.review, prompts: n(s.prompts) }));
  for (const s of timeline) {
    const a = agentsBy.get(s.key);
    if (!a || s.review) continue;
    a.sessions++; a.tokens += s.tokens;
    const d0 = day(s.start), d1 = day(s.end);
    if (!a.first || d0 < a.first) a.first = d0;
    if (!a.last || d1 > a.last) a.last = d1;
  }
  for (const a of agentsBy.values()) {
    a.pct = pctOf(a.lines);
    a.survival = a.lines ? Math.min(1, a.surviving / a.lines) : 0;
    a.verified = a.lines ? Math.min(1, (verifiedBy.get(a.key) ?? 0) / a.lines) : 0;
    const mm = [...(modelLines.get(a.key) ?? new Map()).entries()].sort((x, y) => y[1] - x[1]);
    const mt = mm.reduce((s, [, v]) => s + v, 0) || 1;
    a.models = mm.map(([id, v]) => ({ id, name: modelName(id), lines: v, share: v / mt }));
  }
  for (const p of peopleBy.values()) p.pct = pctOf(p.lines);

  const unknown = agentsBy.get('unattributed') ?? null;
  const agents = [...agentsBy.values()].filter((x) => x.kind === 'agent' || x.kind === 'bot').sort((a, b) => b.lines - a.lines);
  agents.forEach((a, i) => (a.rank = i));
  const shown = agents.filter((x) => x.pct >= 0.1).slice(0, 6);
  const rest = agents.filter((x) => !shown.includes(x));
  const rows: Agent[] = [...shown];
  const restLines = rest.reduce((s, x) => s + x.lines, 0);
  if (pctOf(restLines) >= 0.1) {
    const sum = (k: 'verified' | 'declared' | 'inferred') => rest.reduce((s, x) => s + (x.proof?.[k] ?? 0), 0);
    rows.push({ key: 'other', name: nameOf('other', lang), kind: 'other', lines: restLines, pct: pctOf(restLines), surviving: rest.reduce((s, x) => s + x.surviving, 0), survival: 0, verified: 0, proof: { verified: sum('verified'), declared: sum('declared'), inferred: sum('inferred') }, models: [], providers: [], sessions: rest.reduce((s, x) => s + x.sessions, 0), tokens: 0, first: '', last: '', rank: rows.length });
  }
  if (unknown && unknown.pct >= 0.05) { unknown.name = nameOf('unattributed', lang); unknown.rank = 99; rows.push(unknown); }

  const people = [...peopleBy.values()].sort((a, b) => b.lines - a.lines);
  const hu = c.human ?? {};
  const t = c.totals ?? {};
  const hand: Hand = {
    prompts: n(hu.prompts), reviews: n(hu.reviews), handLines: people.reduce((s, p) => s + p.lines, 0), tabLines: n(hu.tabAssistedLines), plans: n(hu.plans),
    written: n(t.linesWritten), committed: n(t.linesAdded), surviving: n(t.linesSurviving), sessions: n(t.sessions),
  };

  // 按版本、按周、按天：都换成分组 key
  const versionsRaw: any[] = c.versions ?? [];
  const verTags = [...versionsRaw.map((v) => v.tag), 'unreleased'];
  const versions = verTags.map((tag) => {
    const by: Record<string, number> = {};
    for (const b of c.breakdown ?? []) { const v = n(b.byVersion?.[tag]); if (v && groupOf(b.contributor).kind !== 'human') { const k = keyOf(b.contributor); by[k] = (by[k] ?? 0) + v; } }
    return { tag, date: versionsRaw.find((v) => v.tag === tag)?.date ?? '', by, total: Object.values(by).reduce((s, x) => s + x, 0) };
  }).filter((v) => v.total > 0);
  const regroup = (by: Record<string, number>) => {
    const out: Record<string, number> = {};
    for (const [id, v] of Object.entries(by ?? {})) {
      const gr = id.includes(':') || byId.has(id) ? groupOf(id) : { key: id, kind: id.startsWith('h:') ? 'human' : 'agent' };
      if (gr.kind === 'human' || id.startsWith('h:')) continue;
      out[gr.key] = (out[gr.key] ?? 0) + n(v);
    }
    return out;
  };
  const weeks = (c.timeline ?? []).map((w: any) => ({ week: w.week, by: regroup(w.byContributor) }));
  const days = (c.days ?? []).map((d: any) => ({ date: d.date, by: regroup(d.by) }));
  const runKey = (id: string) => {
    const gr = id.includes(':') || byId.has(id) ? groupOf(id) : { key: id, kind: id.startsWith('h:') ? 'human' : 'agent' };
    return gr.kind === 'human' || id.startsWith('h:') ? 'human' : gr.key;
  };
  const files = (c.files ?? []).map((f: any) => ({
    path: f.path ?? '', lines: n(f.lines), by: regroup(f.by),
    runs: Array.isArray(f.runs) ? f.runs.filter((x: any) => Array.isArray(x)).map(([k, v]: [string, number]) => [runKey(String(k)), n(v)] as [string, number]) : undefined,
  }));

  const areas: Record<string, Record<string, number>> = {};
  for (const b of c.breakdown ?? []) {
    const gr = groupOf(b.contributor);
    if (gr.kind === 'human') continue;
    const k = gr.key;
    for (const [ar, v] of Object.entries(b.byArea ?? {})) { (areas[ar] ??= {})[k] = (areas[ar][k] ?? 0) + n(v); }
  }
  const range = c.project?.range ?? {};
  const span = range.from && range.to ? Math.round((Date.parse(range.to) - Date.parse(range.from)) / 86400000) + 1 : 0;
  const reviewers = contribs
    .filter((x) => x.kind === 'agent' && (x.roles ?? []).includes('reviewer') && !(x.roles ?? []).includes('author'))
    .map((x) => ({ key: x.harness, name: nameOf(x.harness, lang), model: x.model ?? '', reviews: n(x.reviews) }));
  // 旧档案里的 directors 只用来认出「本机用户」是人；不再生成「主导」字样
  const sign = o.sign || '';
  return {
    project: c.project?.name ?? '', lang,
    range: { from: range.from ?? '', to: range.to ?? '' }, spanDays: span, activeDays: n(t.days),
    agents, rows, unknown, people, named: o.named, sign, hand, showHand: false,
    commits: n(t.commits), units: n(t.units), linesAdded: n(t.linesAdded) || total, linesSurviving: n(t.linesSurviving), tokens: n(t.tokens), sessions: n(t.sessions),
    verifiedShare: total ? verifiedAll / total : 0,
    versions, weeks, days, clock: Array.isArray(c.clock) ? c.clock : [], files, areas, timeline, units: c.units ?? [], reviewers, raw: c,
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
  if (a.kind === 'unknown') return { k: a.key, t: a.name, d: [zh ? `${fmt(a.lines)} 行 · ${pct(a.pct)}` : `${fmt(a.lines)} lines · ${pct(a.pct)}`, zh ? '找不到证据，不算到任何人头上' : 'No evidence for anyone, so not credited'] };
  const d = [zh ? `${fmt(a.lines)} 行 · ${pct(a.pct)}` : `${fmt(a.lines)} lines · ${pct(a.pct)}`];
  if (a.models.length) d.push(a.models.slice(0, 3).map((x) => `${x.name} ${Math.round(x.share * 100)}%`).join(' · '));
  if (a.surviving) d.push(zh ? `现在还在 ${fmt(a.surviving)} 行（${Math.round(a.survival * 100)}%）` : `${fmt(a.surviving)} still in the code (${Math.round(a.survival * 100)}%)`);
  if (a.sessions) d.push(zh ? `${a.sessions} 个会话${a.tokens ? ` · ${compact(a.tokens)} token` : ''}` : `${a.sessions} sessions${a.tokens ? ` · ${compact(a.tokens)} tokens` : ''}`);
  if (a.verified) d.push(zh ? `${Math.round(a.verified * 100)}% 能在日志里对上` : `${Math.round(a.verified * 100)}% line-level evidence`);
  if (a.providers.length) d.push((zh ? '通过 ' : 'via ') + a.providers.join(' · '));
  return { k: a.key, t: a.name, d };
}
