// 生成 fixtures/lumen.json：虚构但分布真实的样例档案（多工具、多模型、人、未确认），给测试、样张和 App 调试用。node tools/fixture.mjs fixtures/lumen.json
import { writeFileSync } from 'node:fs';
let seed = 7; const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const C = [
  { id: 'a:claude-code/claude-opus-5-5', kind: 'agent', harness: 'claude-code', model: 'claude-opus-5-5', family: 'claude', roles: ['author'] },
  { id: 'a:claude-code/claude-fable-5-1', kind: 'agent', harness: 'claude-code', model: 'claude-fable-5-1', family: 'claude', roles: ['author'] },
  { id: 'a:codex/gpt-5.6-sol', kind: 'agent', harness: 'codex', model: 'gpt-5.6-sol', family: 'gpt', roles: ['author'] },
  { id: 'a:cursor/composer-2', kind: 'agent', harness: 'cursor', model: 'composer-2', family: 'cursor', roles: ['author'] },
  { id: 'a:kimi/kimi-k3', kind: 'agent', harness: 'kimi', model: 'kimi-k3', family: 'kimi', roles: ['author'] },
  { id: 'a:grok/grok-5-code', kind: 'agent', harness: 'grok', model: 'grok-5-code', family: 'grok', roles: ['author'] },
  { id: 'h:lin', kind: 'human', name: 'Lin', roles: ['author', 'reviewer'] },
  { id: 'h:mara', kind: 'human', name: 'Mara Okafor', roles: ['author'] },
  { id: 'unattributed', kind: 'unattributed', roles: [] },
  { id: 'a:codex/codex-auto-review', kind: 'agent', harness: 'codex', model: 'codex-auto-review', roles: ['reviewer'], reviews: 23 },
];
const lines = { 'a:claude-code/claude-opus-5-5': 18420, 'a:claude-code/claude-fable-5-1': 9310, 'a:codex/gpt-5.6-sol': 12880, 'a:cursor/composer-2': 3120, 'a:kimi/kimi-k3': 1460, 'a:grok/grok-5-code': 610, 'h:lin': 2240, 'h:mara': 380, unattributed: 940 };
const total = Object.values(lines).reduce((a, b) => a + b, 0);
const vers = ['v0.1.0', 'v0.2.0', 'v0.3.0', 'unreleased'];
const breakdown = Object.entries(lines).map(([k, n]) => {
  const v = n * 0.62 | 0, d = n * 0.3 | 0;
  const bv = {}; let left = n; vers.forEach((t, i) => { const x = i === 3 ? left : Math.round(n * [0.15, 0.3, 0.35][i] * (0.6 + rnd() * 0.8)); bv[t] = Math.min(x, left); left -= bv[t]; });
  return { contributor: k, linesAdded: n, linesSurviving: Math.round(n * (0.45 + rnd() * 0.4)),
    byEvidence: k === 'unattributed' ? { unattributed: { linesAdded: n } } : k.startsWith('h:') ? { declared: { linesAdded: n } } : { verified: { linesAdded: v }, declared: { linesAdded: d }, inferred: { linesAdded: n - v - d } },
    byArea: { core: n * 0.5 | 0, frontend: n * 0.25 | 0, tests: n * 0.15 | 0, docs: n * 0.1 | 0 }, byVersion: bv };
});
const start = Date.parse('2026-06-02T00:00:00Z');
const days = []; const timeline = [];
for (let i = 0; i < 112; i++) {
  if (rnd() < 0.3) continue;
  const d = new Date(start + i * 86400000).toISOString().slice(0, 10);
  const by = {}; for (const k of ['claude-code', 'codex', 'cursor', 'kimi', 'grok', 'h:lin', 'unattributed']) if (rnd() < 0.55) by[k] = Math.round(rnd() * 600 * (k === 'claude-code' ? 2 : 1));
  days.push({ date: d, by });
}
for (let w = 23; w <= 38; w++) { const by = {}; for (const c of C.slice(0, 8)) by[c.id] = Math.round(rnd() * lines[c.id] / 8); by.unattributed = Math.round(rnd() * 80); timeline.push({ week: `2026-W${w}`, byContributor: by }); }
const clock = Array.from({ length: 7 }, (_, d) => Array.from({ length: 24 }, (_, h) => Math.max(0, Math.round((h >= 9 && h <= 23 ? 6 : h < 3 ? 3 : 0.4) * rnd() * (d >= 5 ? 0.6 : 1) + (h === 22 ? 4 : 0)))));
const sessions = []; const harnessIds = C.slice(0, 6).map((c) => c.id);
let t = start / 1000;
for (let i = 0; i < 140; i++) { t += 3600 * (2 + rnd() * 16); const c = harnessIds[Math.floor(Math.pow(rnd(), 1.6) * 6)]; sessions.push({ contributor: c, start: Math.round(t), end: Math.round(t + 600 + rnd() * 9000), tokens: Math.round(2e5 + rnd() * 6e6) }); }
for (let i = 0; i < 23; i++) sessions.push({ contributor: 'a:codex/codex-auto-review', start: Math.round(start / 1000 + rnd() * 9e6), end: Math.round(start / 1000 + rnd() * 9e6 + 300), tokens: 90000, review: true });
const titles = ['First runnable attribution pipeline', 'Cursor line-hash parity', 'Credits roll layout', 'Glass theme', 'Kimi adapter', 'Worktree path mapping', 'Release notes generator', 'Survival blame cache', 'Desktop app shell', 'Export to PDF', 'Badge SVG', 'Fix timezone of clock chart', 'Docs: data sources', 'Grok adapter', 'Theme gallery', 'Privacy aliases', 'Incremental analyze', 'GIF export', 'Embed web component', 'Skill for agents'];
const units = titles.map((ti, i) => {
  const a = C[Math.floor(rnd() * 6)].id, b = C[Math.floor(rnd() * 7)].id;
  const s = 0.5 + rnd() * 0.45;
  return { id: i % 3 ? `pr:${i + 3}` : `c:${(0x1a2b3c4d + i * 7919).toString(16)}`, title: ti, date: new Date(start + i * 5.2 * 86400000).toISOString().slice(0, 10), version: vers[Math.min(3, Math.floor(i / 5))] === 'unreleased' ? null : vers[Math.min(3, Math.floor(i / 5))], commits: ['x'], areas: ['core'], lines: Math.round(200 + rnd() * 3000), shares: a === b ? [{ contributor: a, share: 1, evidence: 'verified' }] : [{ contributor: a, share: +s.toFixed(3), evidence: 'verified' }, { contributor: b, share: +(1 - s).toFixed(3), evidence: 'declared' }] };
});
const files = ['crates/core/src/engine.rs', 'crates/adapters/src/codex.rs', 'packages/render/src/forms.ts', 'apps/desktop/src/main.ts', 'README.md', 'crates/cli/src/main.rs', 'packages/render/src/themes.ts', 'crates/adapters/src/git.rs', 'apps/desktop/src/style.css', 'packages/cli/src/cli.ts']
  .map((p) => { const n = Math.round(100 + rnd() * 1600); const by = { 'claude-code': n * 0.5 | 0, codex: n * 0.3 | 0, cursor: n * 0.08 | 0, 'h:lin': n * 0.07 | 0 }; by.unattributed = n - Object.values(by).reduce((a, b) => a + b, 0); return { path: p, lines: n, by }; });
const out = {
  schemaVersion: 0, project: { name: 'lumen', repos: ['github.com/example/lumen'], range: { from: '2026-06-02', to: '2026-09-21' } },
  generatedAt: '2026-09-22T10:00:00+08:00', level: 'standard', head: 'f'.repeat(40),
  versions: [{ tag: 'v0.1.0', date: '2026-06-28' }, { tag: 'v0.2.0', date: '2026-07-30' }, { tag: 'v0.3.0', date: '2026-09-01' }],
  coverage: [{ name: '2026-06', from: '2026-06-02', to: '2026-06-30', best: 'declared', verifiedShare: 0.31 }, { name: '2026-07', from: '2026-07-01', to: '2026-09-21', best: 'verified', verifiedShare: 0.78 }],
  directors: [{ id: 'h:lin', name: 'Lin' }],
  contributors: C, totals: { commits: 612, units: 20, linesAdded: total, linesSurviving: Math.round(total * 0.64), linesWritten: Math.round(total * 1.9), tokens: sessions.reduce((a, s) => a + s.tokens, 0), sessions: 140, days: days.length, binaryFiles: 31 },
  breakdown, human: { prompts: 1873, plans: 1420, reviews: 41, tabAssistedLines: 860, agentReviews: 23 },
  timeline, days, clock, files, sessions, units, conflictLines: 12,
};
writeFileSync(process.argv[2], JSON.stringify(out, null, 1));
console.log('total', total);
