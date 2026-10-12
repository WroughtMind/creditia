// 原版渲染器（satori）：全部作品在几个代表主题下都能出图；署名只写 AI
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { art, buildModel, FORMS, themeById, THEMES } from '../src/classic/index.ts';
import { ORDERED_THEMES, THEME_INFO, PIECE_INFO } from '../src/classic/catalog.ts';

const raw = JSON.parse(readFileSync(path.join(import.meta.dirname, '../fixtures/lumen.json'), 'utf8'));
const m = buildModel(raw, { director: false, lang: 'zh' });

test('目录覆盖全部主题和作品', () => {
  assert.equal(ORDERED_THEMES.length, THEMES.length);
  for (const t of THEMES) assert.ok(THEME_INFO[t.id], t.id);
  for (const id of Object.keys(FORMS)) if (id !== 'profile' && id !== 'blame') assert.ok(PIECE_INFO[id], id);
});

test('署名只写 AI：没有人这一行、没有主导者，占比加起来是 100%', () => {
  assert.equal(m.director, '');
  assert.equal(m.humans.length, 0);
  assert.ok(!m.rows.some((r) => r.kind === 'human'));
  const sum = m.rows.reduce((s, r) => s + r.pct, 0);
  assert.ok(Math.abs(sum - 100) < 0.5, `占比合计 ${sum}`);
});

test('全部作品在代表主题下都能出图，滤镜区域收在画布内', async () => {
  for (const tid of ['minimal-light', 'glass', 'cinema', 'arcade']) {
    const t = themeById(tid)!;
    for (const id of [...Object.keys(FORMS).filter((x) => x !== 'profile' && x !== 'blame'), 'badge']) {
      const a = await art(id, m, t, { lang: 'zh' });
      assert.ok(a && a.svg.startsWith('<svg') && a.w > 0 && a.h > 0, `${id} × ${tid}`);
      assert.ok(!/ (width|height)="-/.test(a.svg), `${id} × ${tid} 有负尺寸`);
      assert.ok(!/<filter[^>]*x="-/.test(a.svg), `${id} × ${tid} 滤镜区域越界`);
    }
  }
});

test('片尾字幕：全部版式都能出图；没有标志的工具也进名单', async () => {
  const raw2 = structuredClone(raw);
  for (const c of raw2.contributors) if (c.harness === 'kimi') c.harness = 'opencode';
  const m2 = buildModel(raw2, { director: false, lang: 'zh' });
  assert.ok(m2.rows.some((r) => r.key === 'opencode'));
  for (const v of FORMS.roll.variants!) assert.ok(await art('roll', m2, themeById('film')!, { lang: 'zh', variant: v }), v);
});

test('英文：全部作品、主题、版式里没有残留的中文', async () => {
  const { enTree, leftoverCjk } = await import('../src/classic/i18n.ts');
  const en = buildModel(raw, { director: false, lang: 'en' });
  const left: string[] = [];
  for (const [id, f] of Object.entries(FORMS)) {
    if (id === 'blame' || id === 'profile') continue;
    for (const t of THEMES) for (const v of f.variants ?? [undefined]) {
      const out = f.render(en, t, { lang: 'en', variant: v });
      if (out) left.push(...leftoverCjk(enTree(out.tree)).map((s) => `${id}/${t.id}/${v ?? ''}: ${s}`));
    }
  }
  assert.deepEqual(left, []);
});

test('片尾字幕新版式：内容少时整体放大，不缩在画面中间；原来的十一种不变', async () => {
  const one = structuredClone(raw);
  const keep = new Set(one.contributors.filter((c: any) => c.harness === 'claude-code' || c.kind !== 'agent').map((c: any) => c.id));
  one.contributors = one.contributors.filter((c: any) => keep.has(c.id));
  one.breakdown = one.breakdown.filter((b: any) => keep.has(b.contributor));
  const m1 = buildModel(one, { director: false, lang: 'zh' });
  assert.deepEqual(FORMS.roll.variants!.slice(0, 11), ['classic', 'columns', 'billing', 'title', 'crawl', 'cast', 'scroll', 'cards', 'end', 'block', 'program']);
  for (const v of ['opening', 'production', 'crew']) {
    const a = await art('roll', m1, themeById('film')!, { lang: 'zh', variant: v });
    const k = Number(/matrix\(([0-9.]+),0,0,\1,/.exec(a!.svg)?.[1] ?? 1);
    assert.ok(k > 1.04, `${v} 没有放大（${k}）`);
  }
});
