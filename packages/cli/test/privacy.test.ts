// 公开前的隐私检查：列出类别和样例，找出像密钥、邮箱、本机路径的内容；按类别隐藏只改公开的那一份
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { check, redact } from '../src/privacy.ts';

const raw = {
  project: { name: 'demo', repos: ['https://bob:hunter2@github.com/acme/demo.git'] },
  contributors: [{ id: 'h:1', kind: 'human', name: 'Bob' }, { id: 'a:codex', kind: 'agent', harness: 'codex' }],
  units: [{ id: 'pr:3', title: 'rotate sk-ant-abcdefghijklmnopqrstuvwx', commits: ['abcdef1234'] }, { id: 'c:1', title: 'fix build on /Users/bob/work', commits: ['1234567890'] }, { id: 'pr:4', title: 'PR #4', commits: [] }],
  files: [{ path: 'src/a.rs', lines: 3 }, { lines: 2 }],
  sessions: [{ start: 1, end: 2 }],
  clock: [[0, 1]],
};

test('检查：各类有几条、样例；可疑的内容标出来，密钥不原样印', () => {
  const r = check(raw);
  assert.equal(r.cats.titles.n, 2);
  assert.equal(r.cats.paths.n, 1);
  assert.deepEqual(r.cats.people.samples, ['Bob']);
  assert.equal(r.cats.repos.n, 1);
  const kinds = r.risks.map((x) => `${x.cat}:${x.kind}`).sort();
  assert.deepEqual(kinds, ['repos:credential', 'titles:key', 'titles:local-path']);
  assert.ok(!JSON.stringify(r.risks).includes('hunter2') && !JSON.stringify(r.risks).includes('abcdefghijklmnop'), JSON.stringify(r.risks));
});

test('隐藏：标题换成编号，路径、人名、作息去掉；仓库地址里的账号密码总是去掉', () => {
  const o = redact(raw, new Set(['titles', 'paths', 'people', 'hours']));
  assert.deepEqual(o.units.map((u: any) => u.title), ['PR #3', '1234567', 'PR #4']);
  assert.ok(!o.files.some((f: any) => f.path));
  assert.ok(!o.contributors.some((c: any) => c.name));
  assert.equal(o.sessions, undefined);
  assert.deepEqual(o.project.repos, ['https://github.com/acme/demo.git']);
  assert.equal(raw.units[0].title.startsWith('rotate'), true); // 原档案不动
  assert.equal(check(redact(raw, new Set(['titles']))).risks.filter((r) => r.cat !== 'repos').length, 0);
});

test('publish：不能问的时候有可疑内容就不写；--hide 掉那一类或 --yes 才写；--check 只检查', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-pub-'));
  try {
    spawnSync('git', ['-C', dir, 'init', '-q']);
    const src = path.join(dir, 'in.json');
    writeFileSync(src, JSON.stringify(raw));
    const bin = path.join(import.meta.dirname, '../src/cli.ts');
    const run = (...a: string[]) => spawnSync(process.execPath, [bin, 'publish', '--credits', src, '--repo', dir, '--lang', 'en', ...a], { encoding: 'utf8', env: { ...process.env, CREDITIA_CORE: '/nonexistent' } });
    const out = path.join(dir, 'credits.json');
    const chk = run('--check', '--json');
    assert.equal(chk.status, 0, chk.stderr);
    assert.equal(JSON.parse(chk.stdout).risks.length, 3);
    assert.ok(!existsSync(out));
    const no = run();
    assert.notEqual(no.status, 0);
    assert.ok(!/[\u3000-\u9fff\uff00-\uffef]/.test(no.stderr), no.stderr);
    assert.ok(!existsSync(out), 'should not write');
    const ok = run('--hide', 'titles,repos', '--json');
    assert.equal(ok.status, 0, ok.stderr);
    const pub = JSON.parse(readFileSync(out, 'utf8'));
    assert.equal(pub.units[0].title, 'PR #3');
    assert.equal(pub.project.repos, undefined);
    assert.equal(pub.files[0].path, 'src/a.rs');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
