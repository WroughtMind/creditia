import { test } from 'node:test';
import assert from 'node:assert/strict';
import { contributionHistory, historyEmpty, historyLabels, versionBuckets } from '../src/history.ts';
import { buildModel } from '../src/data.ts';
import { buildModel as classicModel } from '../src/classic/data.ts';
import { art, STYLES } from '../src/index.ts';
import { art as classicArt, themeById } from '../src/classic/index.ts';
import { pieceName } from '../src/classic/catalog.ts';
import { Resvg } from '@resvg/resvg-js';

const raw = {
  project: { name: 'History', range: { from: '2026-01-01', to: '2026-02-01' } },
  contributors: [{ id: 'a:codex/test', kind: 'agent', harness: 'codex', model: 'test' }, { id: 'h:one', kind: 'human', name: 'One' }],
  breakdown: [{ contributor: 'a:codex/test', linesAdded: 30, byVersion: { unreleased: 30 } }, { contributor: 'h:one', linesAdded: 50, byVersion: { unreleased: 50 } }],
  days: [{ date: '2026-02-01', by: { 'a:codex/test': 20, 'h:one': 50 } }, { date: '2026-01-02', by: { 'a:codex/test': 7 } }, { date: '2026-01-01', by: { 'a:codex/test': 3 } }],
  timeline: [{ week: '2026-W01', byContributor: { 'a:codex/test': 10 } }, { week: '2026-W05', byContributor: { 'a:codex/test': 20, 'h:one': 50 } }],
  totals: { linesAdded: 80 }, versions: [],
};

test('没有版本时按真实日期合并到月份，不叠加周明细或 unreleased 总数；贡献者单独成段', () => {
  const h = buildModel(raw).history;
  assert.equal(h.basis, 'months');
  assert.deepEqual(h.buckets.map((b) => [b.tag, b.total, b.by]), [['2026-01', 10, { codex: 10 }], ['2026-02', 70, { codex: 20, 'h:one': 50 }]]);
  assert.deepEqual(classicModel(raw).history, h);
  assert.equal(buildModel(raw, { group: 'model' }).history.buckets.reduce((s, b) => s + b.total, 0), 80);
  assert.deepEqual(raw.versions, [], '展示不能把月份写回版本');
  assert.equal(historyLabels(h.basis).name, '开发历程');
});

test('版本优先，同名节点去重，未发布列在最后；零贡献别名不重复画柱子', () => {
  const r = { ...raw, versions: [{ tag: 'v1', date: '2026-01-02' }, { tag: 'v1', date: '2026-01-02' }, { tag: 'alias', date: '2026-01-02' }], breakdown: [{ contributor: 'a:codex/test', linesAdded: 30, byVersion: { v1: 10, unreleased: 20 } }] };
  const h = buildModel(r).history;
  assert.equal(h.basis, 'versions');
  assert.deepEqual(h.buckets.map((b) => [b.tag, b.total]), [['v1', 10], ['unreleased', 20]]);
  assert.deepEqual(classicModel(r).history, h);
});

test('有标签但缺版本明细时说明缺失，不用时间数据冒充版本数据', () => {
  const r = { ...raw, versions: [{ tag: 'v1', date: '2026-01-02' }], breakdown: [{ contributor: 'a:codex/test', linesAdded: 30 }] };
  const h = buildModel(r).history;
  assert.equal(h.basis, 'versions');
  assert.equal(h.buckets.length, 0);
  assert.match(historyEmpty(h), /版本明细.*重新分析/);
});

test('只有周明细的档案保留真实周粒度，支持旧工具名键，不推测跨月归属', () => {
  const r = { ...raw, days: [], timeline: [{ week: '2026-W05', byContributor: { codex: 30, 'h:one': 50 } }] };
  const h = buildModel(r).history;
  assert.equal(h.basis, 'weeks');
  assert.deepEqual(h.buckets.map((b) => [b.tag, b.total]), [['2026-W05', 80]]);
  assert.deepEqual(classicModel(r).history, h);
  assert.equal(historyLabels(h.basis, 'en').caption, 'Lines added per week');
});

test('明确区分明细缺失与没有贡献，不用总数伪造时间统计', () => {
  const missing = contributionHistory({ ...raw, days: [], timeline: [] }, (s) => s);
  assert.equal(missing.buckets.length, 0);
  assert.match(historyEmpty(missing), /时间明细/);
  const empty = contributionHistory({ ...raw, days: [{ date: '2026-01-01', by: { 'h:one': 0 } }] }, (s) => s);
  assert.equal(empty.buckets.length, 0);
  assert.match(historyEmpty(empty), /暂无可展示/);
});

test('无效日期、负数和非有限数不产生伪月份或损坏柱高', () => {
  const h = contributionHistory({ ...raw, timeline: [], days: [{ date: '2026-02-30', by: { codex: 100 } }, { date: '2026-01-01', by: { codex: 3, cursor: -1, 'claude-code': NaN, unknown: Infinity } }] }, (s) => s);
  assert.deepEqual(h.buckets.map((b) => [b.tag, b.total]), [['2026-01', 3]]);
  assert.deepEqual(versionBuckets({ ...raw, breakdown: [] }, (s) => s), []);
});

test('单版本、单月份与缺失提示在两套渲染器和中英文中都可生成', async () => {
  const samples = [
    { ...raw, days: [raw.days[0]] },
    { ...raw, versions: [{ tag: 'v1', date: '2026-02-01' }], breakdown: [{ contributor: 'a:codex/test', linesAdded: 30, byVersion: { v1: 30 } }] },
    { ...raw, days: [], timeline: [] },
  ];
  for (const r of samples) for (const lang of ['zh', 'en'] as const) {
    const m = buildModel(r, { lang });
    for (const st of STYLES) {
      const a = art('versions', m, st)!;
      assert.ok(a, st.id);
      assert.ok(!/NaN|undefined/.test(a.svg), st.id);
      assert.ok(a.title.endsWith(historyLabels(m.history.basis, lang).name));
      assert.doesNotThrow(() => new Resvg(a.svg, { font: { loadSystemFonts: false } }), st.id);
    }
    const c = await classicArt('versions', classicModel(r, { director: false, lang }), themeById('minimal-light')!, { lang });
    assert.ok(c);
    assert.ok(c.title.endsWith(historyLabels(m.history.basis, lang).name));
    assert.doesNotThrow(() => new Resvg(c.svg, { font: { loadSystemFonts: false } }));
    assert.equal(pieceName('versions', lang, r), historyLabels(m.history.basis, lang).name);
  }
});

test('较多月份全部保留，柱子和交互区域不越过画布', async () => {
  const r = { ...raw, days: Array.from({ length: 24 }, (_, i) => ({ date: `${2025 + Math.floor(i / 12)}-${String(i % 12 + 1).padStart(2, '0')}-01`, by: { 'a:codex/test': i + 1 } })) };
  const m = buildModel(r), st = STYLES.find((s) => s.id === 'cyber')!;
  const a = art('versions', m, st)!;
  assert.equal(m.history.buckets.length, 24);
  assert.equal(a.hits.filter((h) => /^202[56]-\d\d$/.test(h.t)).length, 24);
  assert.ok(a.hits.every((h) => h.x >= 0 && h.x + h.w <= a.w && h.y + h.h <= a.h));
  const c = await classicArt('versions', classicModel(r), themeById('minimal-light')!, { lang: 'zh' });
  assert.ok(c && c.w > 1200);
  assert.doesNotThrow(() => new Resvg(c!.svg, { font: { loadSystemFonts: false } }));
});
