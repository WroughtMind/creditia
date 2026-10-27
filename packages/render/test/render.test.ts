// 渲染测试：全部作品 × 全部风格都能出图；没有「主导」；人名默认不出现；导出格式文件头正确；交互区域在画面内
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { Resvg } from '@resvg/resvg-js';
import { buildModel, STYLES, PIECES, MODEL_VARIANTS, art, frames, raster } from '../src/index.ts';
import { makeExporter, sized } from '../src/export.ts';
import { creditsBlock, fetchSegs, commitsComment, mergeMarked } from '../src/text.ts';
import { hanzi, modelName, resolveGroup } from '../src/data.ts';
import * as C from '../src/classic/data.ts';
import { creditsOf } from '../src/pieces/roll.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const m = buildModel(raw);
const ex = makeExporter(raster, (d) => new Uint8Array(deflateSync(d)));

test('模型：agent 按工具归组，其他贡献者按名字进排名，合计 100%，没有主导者', () => {
  assert.deepEqual(m.rows.map((r) => r.key), ['claude-code', 'codex', 'cursor', 'kimi', 'grok', 'h:mara okafor', 'unattributed']);
  assert.equal(m.rows.find((r) => r.kind === 'person')!.name, 'Mara Okafor');
  const sum = m.rows.reduce((s, r) => s + r.pct, 0);
  assert.ok(Math.abs(sum - 100) < 0.05, `占比合计 ${sum}`);
  // 「你」不是贡献者行：自己的提交没有 AI 证据就是未确认
  assert.ok(!m.rows.some((r) => r.name === 'Lin'));
  assert.deepEqual(m.authors.map((a) => [a.name, a.me]), [['Lin', true], ['Mara Okafor', false]]);
  assert.ok(Math.abs(m.authors.reduce((s, a) => s + a.pct, 0) - 100) < 0.05);
  // 不署名：名字换成「其他贡献者」
  const anon = buildModel(raw, { named: false });
  assert.ok(anon.rows.some((r) => r.key === 'people' && r.name === '其他贡献者'));
  assert.ok(!JSON.stringify(anon.rows).includes('Mara'));
  assert.equal(m.agents[0].models.map((x) => x.name).join(','), 'Claude Opus 5.5,Claude Fable 5.1');
  assert.ok(m.hand.prompts > 0 && m.hand.written > m.hand.committed);
});

test('模型名好读', () => {
  assert.equal(modelName('claude-opus-5-5'), 'Claude Opus 5.5');
  assert.equal(modelName('gpt-5.6-sol'), 'GPT-5.6 Sol');
  assert.equal(modelName('kimi-k3'), 'Kimi K3');
  assert.equal(modelName('auto'), 'Auto');
});

test('汉字数字', () => {
  assert.equal(hanzi(56), '五十六');
  assert.equal(hanzi(10), '十');
  assert.equal(hanzi(105), '一百〇五');
  assert.equal(hanzi(0), '〇');
});

test('每种作品在每种风格下都能出图，交互区域都在画面里', () => {
  for (const st of STYLES) {
    for (const [id, p] of Object.entries(PIECES)) {
      for (const v of p.variants ?? [undefined]) {
        const a = art(id, m, st, { variant: v });
        assert.ok(a, `${id}/${v}/${st.id}`);
        assert.ok(a!.svg.startsWith('<svg') && a!.svg.endsWith('</svg>'), `${id}/${st.id}`);
        assert.ok(!/NaN|undefined/.test(a!.svg), `${id}/${v}/${st.id} 有 NaN 或 undefined`);
        assert.ok(a!.hits.length > 0, `${id}/${v}/${st.id} 没有交互区域`);
        for (const h of a!.hits) assert.ok(h.x >= -2 && h.y >= -2 && h.x + h.w <= a!.w + 2 && h.y + h.h <= a!.h + 2, `${id}/${v}/${st.id} 区域越界 ${JSON.stringify(h)}`);
      }
    }
  }
});

test('赛博英文标题中的特殊字符不会让环形图预览消失', () => {
  const a = art('donut', buildModel(raw, { lang: 'en' }), STYLES.find((s) => s.id === 'cyber')!, { variant: 'ring' })!;
  assert.doesNotThrow(() => new Resvg(a.svg, { font: { loadSystemFonts: false } }));
});

test('不写「主导」；其他贡献者默认写 git 名字，关掉署名后一个名字都不出现', () => {
  const all = STYLES.flatMap((st) => Object.keys(PIECES).map((id) => art(id, m, st)?.svg ?? '')).join('');
  assert.ok(!all.includes('主导'));
  assert.ok(all.includes('Mara Okafor'));
  const anon = buildModel(raw, { named: false });
  const hidden = STYLES.flatMap((st) => Object.keys(PIECES).map((id) => art(id, anon, st)?.svg ?? '')).join('') + creditsBlock(anon) + creditsBlock(anon, 10, 'contributor');
  assert.ok(!hidden.includes('Mara'), '关掉署名后还有名字');
  const named = buildModel(raw, { sign: 'Lin' });
  const svg = art('hand', named, STYLES[0])!.svg + art('roll', named, STYLES[0])!.svg;
  assert.ok(svg.includes('Lin'));
  assert.ok(!creditsBlock(m).includes('主导'));
});

test('片尾字幕：职务来自数据，相同名单合并', () => {
  const secs = creditsOf(m);
  assert.ok(secs[0].credits.some((c) => c.name === 'Claude Code'));
  const heads = secs.map((s) => s.head);
  assert.equal(new Set(heads).size, heads.length);
});

test('风格之间不是换色：同一作品的 SVG 结构差异大', () => {
  const sizes = STYLES.map((st) => art('tier', m, st)!.svg.length);
  assert.ok(new Set(sizes).size === STYLES.length);
});

test('导出：PNG、PDF、GIF、SVG、HTML 文件头正确', async () => {
  const st = STYLES.find((s) => s.id === 'cyber')!;
  const a = art('share', m, st)!;
  const png = ex.png(a, 1);
  assert.deepEqual([...png.slice(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  const pdf = await ex.pdf(a, 1);
  assert.equal(new TextDecoder().decode(pdf.slice(0, 8)), '%PDF-1.4');
  assert.ok(new TextDecoder().decode(pdf.slice(-6)).includes('%%EOF'));
  const gif = ex.gif(frames('tier', m, st, {}, 3), { width: 200 });
  assert.equal(new TextDecoder().decode(gif.slice(0, 6)), 'GIF89a');
  const svg = ex.svg(a);
  assert.ok(svg.includes('<path') && !svg.includes('<text'), 'SVG 应该把文字转成路径');
  const html = ex.html([a], { title: 't' });
  assert.ok(html.includes('mountHits') && html.includes('<svg'));
  const og = sized(a, 1200, 630);
  assert.equal(og.w, 1200);
  assert.ok(og.hits.every((h) => h.x >= 0 && h.x + h.w <= 1200));
});

test('终端名片', () => {
  const lines = fetchSegs(m).map((l) => l.map((s) => s.text).join(''));
  assert.ok(lines.some((l) => l.includes('Claude Code')));
});

test('银幕滚动的 GIF：取景框扫过整张字幕', () => {
  const st = STYLES.find((s) => s.id === 'cinema')!;
  const fr = frames('roll', m, st, { variant: 'scroll' });
  assert.ok(fr.length > 30);
  assert.equal(fr[0].h, Math.round((1080 * 9) / 16));
  assert.notEqual(fr[0].svg, fr[fr.length - 1].svg);
});

// ───────────────────────── 按模型分组 ─────────────────────────

/** 只留一个工具（Claude Code，两个模型） */
const single = (() => {
  const one = structuredClone(raw);
  const keep = new Set(one.contributors.filter((c: any) => c.harness === 'claude-code' || c.kind !== 'agent').map((c: any) => c.id));
  one.contributors = one.contributors.filter((c: any) => keep.has(c.id));
  one.breakdown = one.breakdown.filter((b: any) => keep.has(b.contributor));
  return one;
})();

test('分组：默认按工具（不按占比自动切换）；指定按模型才按模型', () => {
  assert.equal(resolveGroup(raw), 'tool');
  assert.equal(m.group, 'tool');
  assert.equal(resolveGroup(single), 'tool');
  const ms = buildModel(single, { group: 'model' });
  assert.equal(ms.group, 'model');
  assert.deepEqual(ms.rows.filter((r) => r.kind === 'agent').map((r) => r.name), ['Claude Opus 5.5', 'Claude Fable 5.1']);
  assert.equal(buildModel(single).group, 'tool');
  assert.equal(buildModel(raw, { group: 'model' }).group, 'model');
});

test('按模型分组：key 是「工具/模型」，占比合计不变，按工具的排名还在 tools 里，两套渲染器数字一致', () => {
  const mm = buildModel(raw, { group: 'model' });
  assert.ok(mm.rows.some((r) => r.key === 'claude-code/claude-opus-5-5' && r.tool === 'claude-code' && r.toolName === 'Claude Code'));
  const sum = mm.rows.reduce((s, r) => s + r.pct, 0);
  assert.ok(Math.abs(sum - 100) < 0.5, `占比合计 ${sum}`);
  assert.deepEqual(mm.tools.map((r) => r.key), m.rows.map((r) => r.key));
  // 按周、按天的 key 也换成了模型
  assert.ok(mm.weeks.some((w) => 'claude-code/claude-fable-5-1' in w.by));
  const cm = C.buildModel(raw, { director: false, lang: 'zh', group: 'model' });
  for (const r of mm.rows) {
    // 原版渲染器把贡献者单列在 humans 里
    const c = [...cm.rows, ...cm.humans].find((x) => x.key === r.key);
    assert.ok(c, r.key);
    assert.ok(Math.abs(c!.pct - r.pct) < 0.05, `${r.key}: ${c!.pct} vs ${r.pct}`);
  }
});

test('按模型分组：每种作品在每种风格下都能出图；英文里没有中日文；分层画模型的版式（分段、分组、内外环、分部、明细、分块……）也能画按工具的', () => {
  const mm = buildModel(single, { lang: 'en', group: 'model' });
  for (const st of STYLES) {
    for (const [id, p] of Object.entries(PIECES)) {
      for (const v of p.variants ?? [undefined]) {
        const a = art(id, mm, st, { variant: v });
        assert.ok(a && !/NaN|undefined/.test(a.svg), `${id}/${v}/${st.id}`);
        const left = (a!.svg.replace(/<[^>]+>/g, ' ') + JSON.stringify(a!.hits)).match(/[　-鿿＀-￯]+/g);
        assert.equal(left, null, `${id}/${v}/${st.id}: ${left}`);
      }
    }
  }
  for (const [id, v] of Object.entries(MODEL_VARIANTS).flatMap(([id, vs]) => vs.map((v) => [id, v]))) {
    const a = art(id, m, STYLES[0], { variant: v })!;
    assert.ok(a.hits.some((h) => /Claude Fable 5\.1/.test(JSON.stringify(h))), `${id}/${v} 没有分出模型`);
  }
});

test('旧档案：按天、按文件的 key 是工具名时，按模型分组也能画', () => {
  const old = structuredClone(single);
  for (const d of old.days ?? []) d.by = Object.fromEntries(Object.entries(d.by).map(([k, v]) => [k.startsWith('a:') ? k.slice(2).split('/')[0] : k, v]));
  const mm = buildModel(old, { group: 'model' });
  for (const id of ['calendar', 'mosaic', 'codemap']) assert.ok(art(id, mm, STYLES[0], {}), id);
});

test('PR 评论按分支上的提交汇总；Release 说明只换掉上次放的那段', () => {
  const units = [
    { commits: ['aaaaaaa1111111111'], lines: 300, shares: [{ contributor: 'a:claude-code/claude-opus-5-5', share: 1 }] },
    { commits: ['bbbbbbb2222222222'], lines: 100, shares: [{ contributor: 'a:codex/gpt-5.6-sol', share: 0.998 }, { contributor: 'unattributed', share: 0.002 }] },
    { commits: ['ccccccc3333333333'], lines: 999, shares: [{ contributor: 'a:kimi/kimi-k3', share: 1 }] },
  ];
  const mm = { ...m, units, lang: 'en' as const };
  const s = commitsComment(mm, ['aaaaaaa1111111111', 'bbbbbbb']);
  // 不设门槛：0.2% 的未确认也写出来（<1%）
  assert.equal(s, '**creditia** · Claude Code (Claude Opus 5.5) 75% · Codex (GPT-5.6 Sol) 25% · Unattributed <1%\n');
  assert.equal(commitsComment(mm, ['0000000deadbeef']), '');
  const once = mergeMarked('My notes', '### Credits\n\nA 60%');
  assert.ok(once.startsWith('My notes\n\n<!-- creditia:release -->'), once);
  const twice = mergeMarked(once, '### Credits\n\nB 40%');
  assert.equal(twice.split("<!-- creditia:release -->").length, 2);
  assert.ok(twice.includes('B 40%') && !twice.includes('A 60%') && twice.startsWith('My notes'), twice);
});

test('按贡献者看：每位提交者一行，合计 100%；只说是谁提交的，不说谁手写的', () => {
  const s = creditsBlock(m, 10, 'contributor');
  assert.ok(s.includes('| 贡献者 | 占比 | 新增行 | 其中 |'), s);
  assert.ok(/\| Lin \| 99\.2% \|/.test(s) && /\| Mara Okafor \| 0\.8% \| \+380 \| 没有 AI 记录 0\.8% \|/.test(s), s);
  assert.ok(!s.includes('手写'));
  const tool = creditsBlock(m);
  assert.ok(tool.includes('| 工具 / 贡献者 |') && tool.includes('| Mara Okafor |'));
  const lines = fetchSegs(m, 'contributor').map((l) => l.map((x) => x.text).join(''));
  assert.ok(lines.some((l) => l.startsWith('Lin')) && lines.some((l) => l.startsWith('Mara Okafor')));
  // 贡献者卡片：只有一位提交者时没有
  assert.ok(art('people', m, STYLES[0]));
  assert.equal(art('people', buildModel({ ...raw, authors: raw.authors.slice(0, 1) }), STYLES[0]), null);
});

test('星号：每个风格都画得出，瓣跟着动画长出来', () => {
  for (const st of STYLES) {
    const a = art('asterisk', m, st, {})!;
    assert.ok(a && a.svg.includes('<path'), st.id);
    assert.ok(a.hits.some((h) => /Claude Code/.test(JSON.stringify(h))), st.id);
  }
  const fr = frames('asterisk', m, STYLES[0], {}, 4);
  assert.equal(fr.length, 5);
  assert.notEqual(fr[0].svg, fr[4].svg);
});
