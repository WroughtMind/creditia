// 跨项目汇总：几个项目的档案并成一份，行数相加，路径和项带上项目名，每种作品都能画
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildModel, STYLES, PIECES, art } from '../src/index.ts';
import { mergeCredits } from '../src/merge.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));

test('两个项目并成一份：行数相加、贡献方合并、文件和项带上项目名、版本不合并', () => {
  const other = structuredClone(raw);
  other.project.name = 'beacon';
  const m = mergeCredits([{ name: 'catnap', raw }, { name: 'beacon', raw: other }], 'en');
  assert.equal(m.project.name, 'All projects');
  assert.equal(m.totals.linesAdded, raw.totals.linesAdded * 2);
  assert.equal(m.breakdown.length, raw.breakdown.length);
  const cc = m.breakdown.find((b: any) => b.contributor === 'a:claude-code/claude-opus-5-5');
  assert.equal(cc.byEvidence.verified.linesAdded, 2 * raw.breakdown.find((b: any) => b.contributor === 'a:claude-code/claude-opus-5-5').byEvidence.verified.linesAdded);
  assert.equal(cc.byVersion, undefined);
  assert.deepEqual(m.versions, []);
  assert.ok(m.files.every((f: any) => /^(catnap|beacon)\//.test(f.path)));
  assert.ok(m.units.every((u: any) => /^(catnap|beacon):/.test(u.id)));
  assert.equal(m.days.length, raw.days.length);
  assert.equal(m.clock[2][10], raw.clock[2][10] * 2);
  assert.deepEqual(m.projects.map((p: any) => p.name), ['catnap', 'beacon']);
  // 占比和单个项目一样（两份一样的档案）
  const a = buildModel(raw, { lang: 'en' }), b = buildModel(m, { lang: 'en' });
  assert.deepEqual(b.rows.map((r) => [r.key, Math.round(r.pct)]), a.rows.map((r) => [r.key, Math.round(r.pct)]));
  for (const [id, p] of Object.entries(PIECES)) for (const v of p.variants ?? [undefined]) {
    const x = art(id, b, STYLES[0], { variant: v });
    assert.ok(!x || !/NaN|undefined/.test(x.svg), `${id}/${v}`);
  }
});
