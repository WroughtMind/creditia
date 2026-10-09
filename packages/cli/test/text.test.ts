// 文本形式：CREDITS.md 的标记区间合并、版本段落
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { creditsBlock, mergeCreditsMd, MARK_START, MARK_END } from '../../render/src/text.ts';

function fakeModel(): any {
  return {
    project: '魏碑',
    director: '',
    rows: [
      { key: 'codex', name: 'Codex', pct: 82.5, lines: 405310, kind: 'agent', models: ['gpt-5.6-sol'] },
      { key: 'unattributed', name: '未确认', pct: 9.9, lines: 48466, kind: 'unknown', models: [] },
    ],
    commits: 1755,
    units: 816,
    linesAdded: 491273,
    range: { from: '2026-07-03', to: '2026-10-06' },
    raw: {
      generatedAt: '2026-10-08T12:00:00+08:00',
      directors: [{ id: 'h:me' }],
      breakdown: [
        { contributor: 'a:codex/gpt-5.6-sol', linesAdded: 405310, linesSurviving: 168252, byVersion: { v1: 100 }, byEvidence: {} },
        { contributor: 'unattributed', linesAdded: 48466, linesSurviving: 14380, byVersion: {}, byEvidence: {} },
      ],
      contributors: [
        { id: 'h:me', kind: 'human', name: 'me', roles: ['director'] },
        { id: 'a:codex/gpt-5.6-sol', kind: 'agent', harness: 'codex', model: 'gpt-5.6-sol', roles: ['author'] },
      ],
      coverage: [{ from: '2026-07-03', to: '2026-07-31', best: 'inferred' }],
      versions: [{ tag: 'v0.1.0', date: '2026-07-16' }],
      units: [
        { id: 'pr:12', title: '修一个并发 bug (#12)', date: '2026-07-10', version: 'v0.1.0', commits: ['x'], areas: ['core'], lines: 40, shares: [{ contributor: 'a:codex/gpt-5.6-sol', share: 1, evidence: 'verified' }] },
      ],
    },
  };
}

test('creditsBlock 有占比表、版本段落和标记说明', () => {
  const s = creditsBlock(fakeModel());
  assert.match(s, /\| Codex \| 82\.5% \| \+405,310 \| 168,252 \|/);
  assert.match(s, /未确认 \| 9\.9%/);
  assert.match(s, /## v0\.1\.0 · 2026-07-16/);
  assert.match(s, /- #12 修一个并发 bug — Codex（gpt-5\.6-sol）/);
  assert.match(s, /没有逐行日志/);
});

test('mergeCreditsMd 只改标记区间，区间外的手写内容原样保留', () => {
  const first = mergeCreditsMd(null, 'BLOCK1');
  assert.ok(first.startsWith('# Credits'));
  assert.ok(first.includes(MARK_START) && first.includes(MARK_END));
  // 区间外前后各加一段手写内容
  const handwritten = '前言：这段是我自己写在最前面的。\n\n' + first + '\n## 手写段落\n\n这里是我自己加的。\n';
  const again = mergeCreditsMd(handwritten, 'BLOCK2');
  assert.ok(again.includes('BLOCK2'));
  assert.ok(!again.includes('BLOCK1'));
  assert.ok(again.startsWith('前言：'));
  assert.ok(again.includes('## 手写段落'));
  assert.equal((again.match(new RegExp(MARK_START, 'g')) ?? []).length, 1);
});

test('没有标记时，新块追加在文件末尾', () => {
  const s = mergeCreditsMd('# 我自己的 Credits\n\n手写内容\n', 'BLOCK');
  assert.ok(s.includes('手写内容'));
  assert.ok(s.indexOf('手写内容') < s.indexOf(MARK_START));
});
