// 渲染测试：全部作品 × 全部风格都能出图；没有「主导」；人名默认不出现；导出格式文件头正确；交互区域在画面内
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { buildModel, STYLES, PIECES, art, frames, raster } from '../src/index.ts';
import { makeExporter, sized } from '../src/export.ts';
import { creditsBlock, fetchSegs } from '../src/text.ts';
import { hanzi, modelName } from '../src/data.ts';
import { creditsOf } from '../src/pieces/roll.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/lumen.json', import.meta.url), 'utf8'));
const m = buildModel(raw);
const ex = makeExporter(raster, (d) => new Uint8Array(deflateSync(d)));

test('模型：agent 按工具归组，人不进排名，没有主导者', () => {
  assert.deepEqual(m.rows.map((r) => r.key), ['claude-code', 'codex', 'cursor', 'kimi', 'grok', 'unattributed']);
  assert.ok(!m.rows.some((r) => r.key.startsWith('h:')));
  assert.equal(m.people.length, 2);
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

test('不写「主导」；人名默认不出现，选择署名才出现', () => {
  const all = STYLES.flatMap((st) => Object.keys(PIECES).map((id) => art(id, m, st)?.svg ?? '')).join('');
  assert.ok(!all.includes('主导'));
  assert.ok(!all.includes('Mara Okafor'));
  const named = buildModel(raw, { named: true, sign: 'Lin' });
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
