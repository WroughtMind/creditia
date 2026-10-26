// 文本产物：CREDITS.md 区块、版本说明、PR 评论、终端名片、嵌入代码。都读同一个模型。
import type { Model } from './data.ts';
import { fmt, pct, pct0, compact, nameOf, modelName } from './data.ts';
import { handFacts } from './pieces/common.ts';
import { BRAND } from './style.ts';

export const MARK_START = '<!-- creditia:start -->';
export const MARK_END = '<!-- creditia:end -->';
const zh = (m: Model) => m.lang === 'zh';
const T = (m: Model, a: string, b: string) => (zh(m) ? a : b);
const clean = (s: string) => s.replace(/\s*\(#\d+\)\s*$/, '');

/** 一项的主要贡献方（按工具合并） */
export function unitLead(m: Model, u: any): string {
  const by = new Map<string, number>();
  const contribs = m.raw.contributors ?? [];
  for (const s of u.shares ?? []) {
    const c = contribs.find((x: any) => x.id === s.contributor);
    // 其他贡献者也算：写名字（关掉署名时写「其他贡献者」）
    const k = c?.harness ?? (s.contributor === 'unattributed' ? 'unattributed' : m.named && c?.name ? `h:${c.name}` : 'people');
    by.set(k, (by.get(k) ?? 0) + s.share);
  }
  const top = [...by.entries()].sort((a, b) => b[1] - a[1])[0];
  return top ? (top[0].startsWith('h:') ? top[0].slice(2) : nameOf(top[0], m.lang)) : '—';
}

/** 按工具 / 模型看（默认），或按贡献者看（要档案里有 authors） */
export type View = 'tool' | 'contributor';
/** 按贡献者看的一行说明：Claude Code 44.0% · Codex 14.0% · 未确认 10.0% */
export const authorParts = (a: Model['authors'][number]) => a.parts.map((p) => `${p.name} ${pct(p.pct)}`).join(' · ');

/** CREDITS.md 标记区间里的内容：一张表（按工具或按贡献者）、版本段落 */
export function creditsBlock(m: Model, maxPerSection = 10, view: View = 'tool'): string {
  const out: string[] = [];
  out.push([m.project, T(m, `${fmt(m.commits)} 次提交`, `${fmt(m.commits)} commits`), `+${fmt(m.linesAdded)} ${T(m, '行', 'lines')}`, m.spanDays ? T(m, `${m.spanDays} 天`, `${m.spanDays} days`) : ''].filter(Boolean).join(' · '), '');
  // 文字没有版面限制：全部列出，不折叠
  if (view === 'contributor') {
    out.push(T(m, '| 贡献者 | 占比 | 新增行 | 其中 |', '| Contributor | Share | Added | Of which |'), '|---|---:|---:|---|');
    for (const a of m.authors) out.push(`| ${a.name} | ${pct(a.pct)} | +${fmt(a.lines)} | ${authorParts(a)} |`);
  } else {
    const both = m.all.some((a) => a.kind === 'person' || a.kind === 'people');
    out.push(both ? T(m, '| 工具 / 贡献者 | 模型 | 占比 | 新增行 | 留存行 |', '| Tool / contributor | Models | Share | Added | Retained |') : T(m, '| 工具 | 模型 | 占比 | 新增行 | 留存行 |', '| Tool | Models | Share | Added | Retained |'), '|---|---|---:|---:|---:|');
    for (const a of m.all) out.push(`| ${a.name} | ${a.models.map((x) => x.name).join(', ') || '—'} | ${pct(a.pct)} | +${fmt(a.lines)} | ${fmt(a.surviving)} |`);
  }
  out.push('');
  const hf = handFacts(m);
  if (hf.length) {
    out.push(T(m, `**开发者**：${hf.map((f) => f.text).join('，')}。`, `**Developer**: ${hf.map((f) => f.text).join(', ')}.`), '');
  }
  const units: any[] = m.units;
  const groups: [string, string, any[]][] = [];
  const unrel = units.filter((u) => !u.version);
  if (unrel.length) groups.push([T(m, '未发布', 'Unreleased'), '', unrel]);
  for (const v of [...(m.raw.versions ?? [])].reverse()) groups.push([v.tag, v.date, units.filter((u) => u.version === v.tag)]);
  for (const [title, date, us] of groups) {
    if (!us.length) continue;
    out.push(`## ${title}${date ? ` · ${date}` : ''}`, '');
    const prs = us.filter((u) => String(u.id).startsWith('pr:')).sort((a, b) => b.date.localeCompare(a.date));
    for (const u of prs.slice(0, maxPerSection)) out.push(`- #${String(u.id).slice(3)} ${clean(u.title)} — ${unitLead(m, u)}`);
    if (prs.length > maxPerSection) out.push(T(m, `- 另有 ${prs.length - maxPerSection} 个 PR`, `- and ${prs.length - maxPerSection} more PRs`));
    const direct = us.length - prs.length;
    if (direct) out.push(T(m, `- ${direct} 个直接提交`, `- ${direct} direct commits`));
    out.push('');
  }
  out.push(`<sub>creditia · ${String(m.raw.generatedAt ?? '').slice(0, 10)}</sub>`);
  return out.join('\n');
}

/** 把区块并进已有 CREDITS.md：只改标记区间 */
export function mergeCreditsMd(existing: string | null, block: string): string {
  const wrapped = `${MARK_START}\n\n${block}\n\n${MARK_END}`;
  if (!existing) return `# Credits\n\n${wrapped}\n`;
  const a = existing.indexOf(MARK_START), b = existing.indexOf(MARK_END);
  if (a >= 0 && b > a) return existing.slice(0, a) + wrapped + existing.slice(b + MARK_END.length);
  return existing.trimEnd() + '\n\n' + wrapped + '\n';
}

export function releaseNotes(m: Model, tag: string): string {
  const v = m.versions.find((x) => x.tag === tag);
  if (!v) return '';
  const byKey = new Map([...m.all, ...m.rows].map((a) => [a.key, a.name]));
  const parts = Object.entries(v.by).sort((a, b) => b[1] - a[1]).map(([k, n]) => `${byKey.get(k) ?? nameOf(k, m.lang)} ${pct((n / v.total) * 100)}`);
  return `### Credits\n\n${parts.join(' · ')}\n`;
}

export function prComment(m: Model, id: string): string {
  const u = m.units.find((x: any) => x.id === id);
  if (!u) return '';
  return sharesLine(m, (u.shares ?? []).map((s: any) => [s.contributor, s.share] as [string, number]));
}

/** 份额 → 一行 `**creditia** · 工具 (模型) 71% · 贡献者 9% · 未确认 20%`：分母是全部新增行，加起来 100% */
function sharesLine(m: Model, shares: [string, number][]): string {
  const contribs = m.raw.contributors ?? [];
  const by = new Map<string, number>();
  for (const [id, share] of shares) {
    const c = contribs.find((x: any) => x.id === id);
    const name = c?.harness ? `${nameOf(c.harness, m.lang)}${c.model ? ` (${modelName(c.model)})` : ''}`
      : id === 'unattributed' || c?.kind === 'unattributed' ? nameOf('unattributed', m.lang)
      : (m.named && c?.name) || nameOf('people', m.lang);
    by.set(name, (by.get(name) ?? 0) + share);
  }
  const parts = [...by].filter(([, s]) => s > 0).sort((a, b) => b[1] - a[1]).map(([name, s]) => `${name} ${pct0(s * 100)}`);
  return parts.length ? `**creditia** · ${parts.join(' · ')}\n` : '';
}

/** 一组提交（PR 分支上的）的署名：含这些提交的单元按行数加权。档案里还没有这些提交时返回空串 */
export function commitsComment(m: Model, shas: string[]): string {
  const want = shas.map((s) => s.trim().toLowerCase()).filter((s) => s.length >= 7);
  const hit = (c: string) => want.some((s) => c.startsWith(s) || s.startsWith(c));
  const by = new Map<string, number>();
  let total = 0;
  for (const u of m.units as any[]) {
    if (!(u.commits ?? []).some((c: string) => hit(String(c).toLowerCase()))) continue;
    const lines = Number(u.lines) > 0 ? Number(u.lines) : 1;
    for (const s of u.shares ?? []) { by.set(s.contributor, (by.get(s.contributor) ?? 0) + s.share * lines); total += s.share * lines; }
  }
  if (!total) return '';
  return sharesLine(m, [...by].map(([id, v]): [string, number] => [id, v / total]));
}

/** 一个提交上的盖章：提交盖章 hook 写的 Creditia-Lines（各工具写了几行 / 新增总行数），或者只有 Creditia-Harness / Model；added 是这个提交新增的行数 */
export interface Stamped { lines?: string; harness?: string; model?: string; added: number }

/** 一组提交的盖章汇总：Creditia-Lines 按行数记；只有 Harness 的提交整个算那个工具；没盖章的只计入总数 */
export function stampedShares(cs: Stamped[]): { by: [string, number][]; total: number } {
  const by = new Map<string, number>();
  let total = 0;
  for (const c of cs) {
    const m = /^(.*)\/\s*(\d+)\s*$/.exec(c.lines ?? '');
    if (m) {
      for (const part of m[1].split(',')) {
        const x = /^\s*(\S+)\s+(\d+)\s*$/.exec(part);
        if (x) by.set(x[1], (by.get(x[1]) ?? 0) + Number(x[2]));
      }
      total += Math.max(Number(m[2]), c.added);
    } else if (c.harness) {
      const k = c.model ? `${c.harness.toLowerCase()}/${c.model}` : c.harness.toLowerCase();
      by.set(k, (by.get(k) ?? 0) + c.added);
      total += c.added;
    } else total += c.added;
  }
  return { by: [...by].sort((a, b) => b[1] - a[1]), total };
}

/** 没有 credits.json 时的 PR 评论 / Release 说明：按提交盖章记下的行数。没有 AI 写的行返回空串 */
export function stampedComment(cs: Stamped[], lang: 'zh' | 'en'): string {
  const { by, total } = stampedShares(cs);
  if (!by.length || !total) return '';
  const name = (k: string) => {
    const [tool, model] = k.split('/');
    return `${nameOf(tool, lang)}${model ? ` (${modelName(model)})` : ''}`;
  };
  // 盖章没记到的行：提交时没对上 AI 证据，记「未确认」，让各项加起来是 100%
  const rest = total - by.reduce((s, [, n]) => s + n, 0);
  const parts = [...by.map(([k, n]): [string, number] => [name(k), n]), ...(rest > 0 ? [[nameOf('unattributed', lang), rest] as [string, number]] : [])]
    .map(([k, n]) => `${k} ${pct0((n / total) * 100)}`);
  const note = lang === 'zh' ? `按提交盖章：新增 ${fmt(total)} 行里各工具写的` : `from commit stamps: shares of ${fmt(total)} lines added`;
  return `**creditia** · ${parts.join(' · ')}\n<sub>${note}</sub>\n`;
}

/** shields.io 的 endpoint 徽章：README 里写 `https://img.shields.io/endpoint?url=<这份 JSON 的地址>`，JSON 更新了徽章就跟着变 */
const SHIELDS_LOGO: Record<string, string> = { 'claude-code': 'claude', codex: 'openai', cursor: 'cursor', gemini: 'googlegemini', copilot: 'githubcopilot', windsurf: 'windsurf', zed: 'zedindustries' };
export function shieldsBadge(shares: [string, number][], lang: 'zh' | 'en'): object {
  // 徽章就一行字：写占比最高的两个工具
  const top = shares.filter(([, p]) => p > 0).slice(0, 2);
  const tool = top[0]?.[0].split('/')[0] ?? '';
  const name = (k: string) => { const [t, mdl] = k.split('/'); return mdl ? `${nameOf(t, lang)} (${modelName(mdl)})` : nameOf(t, lang); };
  return {
    schemaVersion: 1,
    label: lang === 'zh' ? 'AI 参与' : 'written with',
    message: top.length ? top.map(([k, p]) => `${name(k)} ${pct0(p)}`).join(' · ') : lang === 'zh' ? '暂无' : 'none yet',
    color: (BRAND[tool] ?? '#555555').replace('#', ''),
    ...(SHIELDS_LOGO[tool] ? { namedLogo: SHIELDS_LOGO[tool], logoColor: 'white' } : {}),
    cacheSeconds: 3600,
  };
}
/** 从档案：各工具（按当前分组）占全部新增行的比例 */
export const badgeShares = (m: Model): [string, number][] => m.agents.filter((a) => a.kind === 'agent').map((a) => [a.model ? `${a.tool}/${a.model}` : a.tool, a.pct]);
/** 从提交盖章 */
export function stampedBadgeShares(cs: Stamped[]): [string, number][] {
  const { by, total } = stampedShares(cs);
  return total ? by.map(([k, n]) => [k, (n / total) * 100]) : [];
}

/** 把一段署名放进已有的文字（Release 说明、PR 评论）：有标记就换掉标记之间的，没有就接在后面 */
export const RELEASE_START = '<!-- creditia:release -->', RELEASE_END = '<!-- /creditia:release -->';
export function mergeMarked(existing: string, block: string, start = RELEASE_START, end = RELEASE_END): string {
  const wrapped = `${start}\n${block.trim()}\n${end}`;
  const a = existing.indexOf(start), b = existing.indexOf(end);
  if (a >= 0 && b > a) return existing.slice(0, a) + wrapped + existing.slice(b + end.length);
  return existing.trim() ? `${existing.trimEnd()}\n\n${wrapped}\n` : `${wrapped}\n`;
}

/** 嵌入代码 */
export function embedSnippet(piece: string, style: string, src = '/.creditia/credits.json', variant?: string): string {
  return `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"></script>\n<creditia-card src="${src}" piece="${piece}" theme="${style}"${variant ? ` variant="${variant}"` : ''}></creditia-card>\n`;
}

/** 终端里的显示宽度：中日文占两格 */
const cols = (s: string) => [...s].reduce((n, ch) => n + (/[\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF]/.test(ch) ? 2 : 1), 0);
const padCols = (s: string, n: number) => s + ' '.repeat(Math.max(0, n - cols(s)));

export interface Seg { text: string; color?: string; dim?: boolean; bold?: boolean }
/** 终端名片（neofetch 式） */
export function fetchSegs(m: Model, view: View = 'tool'): Seg[][] {
  const lines: Seg[][] = [];
  // 标题前挂 creditia 的 *，用写得最多的那个工具的颜色
  lines.push([{ text: '* ', bold: true, color: m.rows.find((a) => a.kind === 'agent')?.key }, { text: m.project, bold: true }, { text: `  ${m.range.from} → ${m.range.to}`, dim: true }]);
  lines.push([{ text: '─'.repeat(Math.max(24, m.project.length + 26)), dim: true }]);
  const bar = (name: string, p: number, key?: string): Seg[] => {
    const n = Math.max(p > 0 ? 1 : 0, Math.round(p / 4));
    return [{ text: padCols(name, 16), color: key }, { text: '█'.repeat(n), color: key }, { text: '░'.repeat(25 - n), dim: true }, { text: ` ${pct(p).padStart(6)}` }];
  };
  // 终端里一行一项，不受版面限制：全部列出
  if (view === 'contributor') for (const a of m.authors) lines.push(bar(a.name, a.pct), [{ text: `  ${authorParts(a)}`, dim: true }]);
  else for (const a of m.all) lines.push([...bar(a.name, a.pct, a.key), { text: a.models[0] ? `  ${a.models[0].name}` : '', dim: true }]);
  lines.push([]);
  lines.push([{ text: T(m, '提交 ', 'commits '), dim: true }, { text: fmt(m.commits) }, { text: T(m, '  行 ', '  lines '), dim: true }, { text: fmt(m.linesAdded) }, { text: m.tokens ? T(m, '  token ', '  tokens ') : '', dim: true }, { text: m.tokens ? compact(m.tokens) : '' }]);
  const hf = handFacts(m);
  if (hf.length) lines.push([{ text: T(m, '人  ', 'people  '), dim: true }, { text: hf.map((f) => f.text).join(' · ') }]);
  return lines;
}
