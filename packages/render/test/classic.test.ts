// 原版渲染器（satori）：全部作品在几个代表主题下都能出图；署名只写 AI
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { art, frames, animatedClassic, buildModel, FORMS, themeById, THEMES } from '../src/classic/index.ts';
import { BRIDGED, BRIDGED_LAYOUTS } from '../src/classic/bridge.ts';
import { variantsOf, defaultVariant } from '../src/classic/art.ts';
import { ORDERED_THEMES, THEME_INFO, PIECE_INFO } from '../src/classic/catalog.ts';

const raw = JSON.parse(readFileSync(path.join(import.meta.dirname, '../fixtures/catnap.json'), 'utf8'));
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
  // 只有一个工具时 auto 会按模型分；这里要的是「内容少」，固定按工具
  const m1 = buildModel(one, { director: false, lang: 'zh', group: 'tool' });
  assert.deepEqual(FORMS.roll.variants!.slice(0, 11), ['classic', 'columns', 'billing', 'title', 'crawl', 'cast', 'scroll', 'cards', 'end', 'block', 'program']);
  for (const v of ['opening', 'production', 'crew']) {
    const a = await art('roll', m1, themeById('film')!, { lang: 'zh', variant: v });
    const k = Number(/matrix\(([0-9.]+),0,0,\1,/.exec(a!.svg)?.[1] ?? 1);
    assert.ok(k > 1.04, `${v} 没有放大（${k}）`);
  }
});

test('原版主题也有新风格的作品和版式：每个主题都能出图，英文里没有中日文', async () => {
  const en = buildModel(raw, { director: false, lang: 'en' });
  for (const t of THEMES) {
    for (const id of BRIDGED) {
      const a = await art(id, en, t, { lang: 'en' });
      assert.ok(a && a.svg.startsWith('<svg'), `${id} × ${t.id}`);
      assert.ok(a.hits.length > 0, `${id} × ${t.id} 没有悬停区`);
      const left = (a.svg.replace(/<[^>]+>/g, ' ') + JSON.stringify(a.hits)).match(/[\u3000-\u9fff\uff00-\uffef]+/g);
      assert.equal(left, null, `${id} × ${t.id}: ${left}`);
    }
    for (const [id, vs] of Object.entries(BRIDGED_LAYOUTS)) for (const v of vs) assert.ok(await art(id, en, t, { lang: 'en', variant: v }), `${id}/${v} × ${t.id}`);
  }
  // 原版自己的画法仍是默认，追加的版式排在原版版式后面
  assert.equal(defaultVariant('tier', themeById('cinema')!), 'original');
  assert.deepEqual(variantsOf('roll').slice(0, FORMS.roll.variants!.length), FORMS.roll.variants);
});

test('原版作品有悬停细节：名字所在的行、色块，在画面以内', async () => {
  const en = buildModel(raw, { director: false, lang: 'en' });
  for (const tid of ['minimal-light', 'cinema', 'poster']) for (const id of ['tier', 'share', 'donut', 'waffle', 'versions']) {
    const a = (await art(id, en, themeById(tid)!, { lang: 'en' }))!;
    assert.ok(a.hits.length > 0, `${id} × ${tid}`);
    for (const h of a.hits) assert.ok(h.x >= -1 && h.y >= -1 && h.x + h.w <= a.w + 1 && h.y + h.h <= a.h + 1, `${id} × ${tid} 悬停区出界`);
    if (id === 'tier') for (const r of en.rows) assert.ok(a.hits.some((h) => h.k === r.key), `${id} × ${tid} 缺 ${r.key}`);
  }
});

test('原版作品会动：条形从零长出来，滚字升上来，最后一帧就是静态图', async () => {
  const t = themeById('minimal-light')!;
  assert.ok(!animatedClassic('badge'));
  const fr = await frames('tier', m, t, { lang: 'zh' }, 6);
  assert.equal(fr.length, 7);
  assert.notEqual(fr[0].svg, fr[6].svg);
  const still = (await art('tier', m, t, { lang: 'zh' }))!;
  const strip = (x: string) => x.replace(/s[0-9a-z]+_/g, '');
  assert.equal(strip(fr[6].svg), strip(still.svg));
  const roll = await frames('roll', m, themeById('crt')!, { lang: 'zh', variant: 'crawl' }, 4);
  assert.equal(roll.length, 5);
  assert.ok((await frames('stream', m, themeById('cinema')!, { lang: 'zh' }, 4)).length === 5);
});

test('按模型分组：原版的全部作品在代表主题下都能出图，模型各占一行', async () => {
  const mm = buildModel(raw, { director: false, lang: 'en', group: 'model' });
  assert.equal(mm.group, 'model');
  assert.ok(mm.rows.some((r) => r.key === 'claude-code/claude-opus-5-5' && r.name === 'Claude Opus 5.5'));
  for (const tid of ['minimal-light', 'glass', 'cinema', 'arcade']) {
    const t = themeById(tid)!;
    for (const id of [...Object.keys(FORMS).filter((x) => x !== 'profile' && x !== 'blame'), 'badge']) {
      const a = await art(id, mm, t, { lang: 'en' });
      assert.ok(a && a.svg.startsWith('<svg') && a.w > 0 && a.h > 0, `${id} × ${tid}`);
    }
  }
});
