// 渲染冒烟测试：所有形式都能出图；风格之间结构不同；人不和 agent 同级
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderSvg, buildModel, THEMES } from '../src/index.ts';
import { badge, ROLL_VARIANTS, RECEIPT_VARIANTS } from '../src/forms.ts';

const fixture = {
  project: { name: 'Fixture', range: { from: '2026-01-01', to: '2026-02-15' } },
  generatedAt: '2026-02-15T00:00:00+08:00',
  directors: [{ id: 'h:boss', name: 'Boss' }],
  contributors: [
    { id: 'a:codex/gpt-5.6', kind: 'agent', harness: 'codex', model: 'gpt-5.6' },
    { id: 'a:cursor/auto', kind: 'agent', harness: 'cursor', model: 'auto' },
    { id: 'h:boss', kind: 'human', name: 'Boss' },
    { id: 'h:friend', kind: 'human', name: 'Friend' },
    { id: 'unattributed', kind: 'unattributed' },
  ],
  totals: { commits: 100, units: 20, linesAdded: 10000, linesSurviving: 6000, sessions: 30, days: 20 },
  breakdown: [
    { contributor: 'a:codex/gpt-5.6', linesAdded: 6000, linesSurviving: 4000, byVersion: { v1: 6000 } },
    { contributor: 'a:cursor/auto', linesAdded: 2000, linesSurviving: 1000, byVersion: { v1: 2000 } },
    { contributor: 'h:boss', linesAdded: 1500 },
    { contributor: 'h:friend', linesAdded: 300 },
    { contributor: 'unattributed', linesAdded: 200 },
  ],
  versions: [{ tag: 'v1', date: '2026-02-01' }],
  coverage: [],
  units: [],
};

const m = buildModel(fixture, { director: true, lang: 'zh' });

test('人不和 agent 同级：合并成一行，另有单独的名单', () => {
  assert.deepEqual(m.rows.map((r) => r.key), ['codex', 'cursor', 'human', 'unattributed']);
  assert.equal(m.humans.length, 1);
  assert.equal(m.humans[0].name, 'Friend');
});

test('体裁与版式清单：15 个主题、11 种片尾版式、7 种小票版式', () => {
  assert.equal(THEMES.length, 15, '主题总数');
  assert.deepEqual(
    THEMES.map((t) => t.id),
    [
      'minimal-light', 'minimal-dark', 'film', 'terminal', 'glass', 'editorial', 'mono',
      'cinema', 'playbill', 'gala', 'broadsheet', 'crt', 'arcade', 'neon', 'poster',
    ],
  );
  assert.deepEqual(ROLL_VARIANTS, [
    'classic', 'columns', 'billing', 'title', 'crawl', 'cast', 'scroll', 'cards', 'end', 'block', 'program',
  ]);
  assert.deepEqual(RECEIPT_VARIANTS, ['slip', 'thermal', 'folio', 'stub', 'ticket', 'ledger', 'scroll']);
});

test('十五种风格的 tier 互不相同', async () => {
  const svgs = new Map<string, string>();
  for (const t of THEMES) {
    const svg = await renderSvg('tier', m, t);
    assert.ok(svg && svg.length > 500, t.id);
    svgs.set(t.id, svg!);
  }
  assert.equal(new Set(svgs.values()).size, THEMES.length);
});

test('片尾字幕十一种版式互不相同', async () => {
  const film = THEMES.find((t) => t.id === 'film')!;
  const svgs: string[] = [];
  for (const v of ROLL_VARIANTS) {
    const svg = await renderSvg('roll', m, film, { lang: 'zh', variant: v });
    assert.ok(svg && svg.length > 500, v);
    svgs.push(svg!);
  }
  assert.equal(new Set(svgs).size, ROLL_VARIANTS.length);
});

test('徽章每种风格画法不同', () => {
  const out = THEMES.map((t) => badge(m, t));
  out.forEach((svg, i) => assert.ok(svg.startsWith('<svg'), THEMES[i].id));
  assert.equal(new Set(out).size, THEMES.length);
});

test('15 个主题 × 代表形式冒烟：返回字符串或 null，不抛异常', async () => {
  const forms = ['tier', 'roll', 'receipt', 'share', 'byline'];
  for (const t of THEMES) {
    for (const form of forms) {
      let svg: string | null;
      try {
        svg = await renderSvg(form, m, t, { lang: 'zh' });
      } catch (e) {
        throw new Error(`${t.id}/${form} 渲染抛异常：${(e as Error).message}`, { cause: e });
      }
      assert.ok(svg === null || typeof svg === 'string', `${t.id}/${form} 应返回字符串或 null`);
    }
    assert.ok(badge(m, t).startsWith('<svg'), `${t.id}/badge`);
  }
});

test('blame 时间按本机时区，不写死 UTC+8', async () => {
  const { localTime } = await import('../src/text.ts');
  assert.equal(localTime(0, 0), '1970-01-01 00:00');
  assert.equal(localTime(0, 480), '1970-01-01 08:00');
  assert.equal(localTime(0, -300), '1969-12-31 19:00');
});
