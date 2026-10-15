// 提交盖章：没有 credits.json 时，PR 评论和 Release 说明读提交上的 Creditia-Lines（CI 里的情形）
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { stampedShares, stampedComment } from '../../render/src/text.ts';

test('盖章汇总：Creditia-Lines 按行数记，只有 Harness 的整个提交算那个工具，没盖章的只计入总数', () => {
  const cs = [
    { lines: 'claude-code/claude-opus-5-5 60, codex 20 / 100', added: 100 },
    { harness: 'Kimi', model: 'kimi-k3', added: 20 },
    { added: 80 },
  ];
  const { by, total } = stampedShares(cs);
  assert.deepEqual(by, [['claude-code/claude-opus-5-5', 60], ['codex', 20], ['kimi/kimi-k3', 20]]);
  assert.equal(total, 200);
  const s = stampedComment(cs, 'en');
  assert.ok(s.startsWith('**creditia** · Claude Code (Claude Opus 5.5) 30% · Codex 10% · Kimi'), s);
  assert.ok(!/[　-鿿]/.test(s), s);
  assert.equal(stampedComment([{ added: 10 }], 'zh'), '');
});

// 直接跑源码（本机可能有打包过的旧 dist）
const bin = path.join(import.meta.dirname, '../src/cli.ts');
const git = (cwd: string, ...a: string[]) => spawnSync('git', ['-C', cwd, '-c', 'user.name=x', '-c', 'user.email=x@example.com', '-c', 'commit.gpgsign=false', ...a], { encoding: 'utf8' }).stdout.trim();

test('CI 里没有 credits.json：PR 评论读这些提交的盖章，Release 说明读这一版的提交', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-stamps-'));
  try {
    git(dir, 'init', '-q', '-b', 'main');
    writeFileSync(path.join(dir, 'a.txt'), 'one\n');
    git(dir, 'add', '-A'); git(dir, 'commit', '-q', '-m', 'base');
    git(dir, 'tag', 'v0.1.0');
    writeFileSync(path.join(dir, 'b.txt'), Array.from({ length: 10 }, (_, i) => `line ${i}`).join('\n') + '\n');
    git(dir, 'add', '-A'); git(dir, 'commit', '-q', '-m', 'add b\n\nCreditia-Lines: codex/gpt-5.6-sol 8 / 10');
    const head = git(dir, 'rev-parse', 'HEAD');
    git(dir, 'tag', 'v0.2.0');
    const run = (...a: string[]) => spawnSync(process.execPath, [bin, ...a, '--repo', dir, '--lang', 'en'], { encoding: 'utf8', env: { ...process.env, CREDITIA_CORE: '/nonexistent' } });
    const pr = run('pr-comment', '--commits', head);
    assert.equal(pr.status, 0, pr.stderr);
    assert.ok(pr.stdout.startsWith('**creditia** · Codex (GPT-5.6 Sol) 80%'), pr.stdout);
    const notes = path.join(dir, 'notes.md');
    writeFileSync(notes, 'Fixes.');
    const rel = run('release-notes', 'v0.2.0', '--into', notes);
    assert.equal(rel.status, 0, rel.stderr);
    assert.ok(rel.stdout.startsWith('Fixes.\n\n<!-- creditia:release -->\n**creditia** · Codex (GPT-5.6 Sol) 80%'), rel.stdout);
    // 上一版没有盖章：说清楚，不出空段
    const old = run('release-notes', 'v0.1.0', '--into', notes);
    assert.notEqual(old.status, 0);
    void readFileSync;
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('动态徽章：shields endpoint 的格式，没有档案时汇总全部提交的盖章', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-badge-'));
  try {
    git(dir, 'init', '-q', '-b', 'main');
    writeFileSync(path.join(dir, 'a.txt'), Array.from({ length: 4 }, (_, i) => `l${i}`).join('\n') + '\n');
    git(dir, 'add', '-A'); git(dir, 'commit', '-q', '-m', 'a\n\nCreditia-Lines: claude-code/claude-opus-5-5 3 / 4');
    const r = spawnSync(process.execPath, [bin, 'badge-json', '--repo', dir, '--lang', 'en'], { encoding: 'utf8', env: { ...process.env, CREDITIA_CORE: '/nonexistent' } });
    assert.equal(r.status, 0, r.stderr);
    const v = JSON.parse(r.stdout);
    assert.equal(v.schemaVersion, 1);
    assert.equal(v.label, 'written with');
    assert.equal(v.message, 'Claude Code (Claude Opus 5.5) 75%');
    assert.match(v.color, /^[0-9A-Fa-f]{6}$/);
    assert.equal(v.namedLogo, 'claude');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
