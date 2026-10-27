// 文本形式：CREDITS.md 的标记区间合并、版本段落
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { creditsBlock, mergeCreditsMd, MARK_START, MARK_END } from '../../render/src/text.ts';
import { buildModel } from '../../render/src/data.ts';

function fakeModel() {
  return buildModel({
    project: { name: '示例项目', range: { from: '2025-03-01', to: '2025-08-31' } },
    generatedAt: '2025-09-01T12:00:00+08:00',
    totals: { commits: 1240, units: 355, linesAdded: 357916 },
    breakdown: [
      { contributor: 'a:codex/gpt-5.6-sol', linesAdded: 312045, linesSurviving: 97531, byVersion: { 'v0.1.0': 100 }, byEvidence: {} },
      { contributor: 'unattributed', linesAdded: 45871, linesSurviving: 12240, byVersion: {}, byEvidence: {} },
    ],
    contributors: [
      { id: 'a:codex/gpt-5.6-sol', kind: 'agent', harness: 'codex', model: 'gpt-5.6-sol', roles: ['author'] },
      { id: 'unattributed', kind: 'unattributed', roles: [] },
    ],
    human: { prompts: 12, reviews: 3 },
    coverage: [{ from: '2025-03-01', to: '2025-03-31', best: 'inferred' }],
    versions: [{ tag: 'v0.1.0', date: '2025-04-02' }],
    units: [
      { id: 'pr:12', title: '修一个并发 bug (#12)', date: '2025-03-28', version: 'v0.1.0', commits: ['x'], areas: ['core'], lines: 40, shares: [{ contributor: 'a:codex/gpt-5.6-sol', share: 1, evidence: 'verified' }] },
    ],
  });
}

test('creditsBlock：工具表、版本段落，默认不写开发者那一段，表头使用留存行', () => {
  const s = creditsBlock(fakeModel());
  assert.match(s, /\| 工具 \| 模型 \| 占比 \| 新增行 \| 留存行 \|/);
  assert.match(s, /\| Codex \| GPT-5\.6 Sol \| 87\.2% \| \+312,045 \| 97,531 \|/);
  assert.match(s, /未确认 \| — \| 12\.8%/);
  assert.ok(!s.includes('**开发者**'), '默认不写开发者的提示词与审阅');
  assert.match(s, /## v0\.1\.0 · 2025-04-02/);
  assert.match(s, /- #12 修一个并发 bug — Codex/);
  assert.match(s, /<sub>creditia · 2025-09-01<\/sub>/);
  assert.ok(!s.includes('主导'));
  assert.ok(!s.includes('没有逐行日志'), '不放长篇叠甲声明');
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
