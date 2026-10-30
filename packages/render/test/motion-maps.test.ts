import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES } from '../src/index.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const normalized = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');
const text = (svg: string) => [...normalized(svg).matchAll(/<text\b[^>]*>[\s\S]*?<\/text>/g)].map((m) => m[0]);
const codeStrokes = (svg: string) => [...svg.matchAll(/<path d="([^"]+)" fill="none" stroke="([^"]+)" stroke-opacity="([^"]+)" stroke-width="2"/g)]
  .filter((m) => m[3] !== '0.14').flatMap((m) => [...m[1].matchAll(/M[-\d.]+ [-\d.]+h[\d.]+/g)].map((stroke) => `${m[2]}/${m[3]}/${stroke[0]}`));

test('代码地图以完整代码笔画铺版，已出现笔画与最终归属、长度一致', () => {
  for (const st of STYLES) {
    const full = art('codemap', model, st)!, finalStrokes = new Set(codeStrokes(full.svg));
    assert.ok(finalStrokes.size > 100, st.id);
    let previous = 0;
    for (const t of [0.15, 0.5, 0.8]) {
      const current = art('codemap', model, st, { t })!, strokes = codeStrokes(current.svg);
      assert.ok(strokes.length >= previous && strokes.length <= finalStrokes.size, `${st.id}/${t}: progressive rows`);
      assert.ok(strokes.every((stroke) => finalStrokes.has(stroke)), `${st.id}/${t}: intact data strokes`);
      assert.deepEqual(text(current.svg), text(full.svg), `${st.id}: true labels`);
      assert.deepEqual(current.hits, full.hits, `${st.id}: stable file locations`);
      previous = strokes.length;
    }
  }
  const ink = STYLES.find((s) => s.id === 'ink')!, glass = STYLES.find((s) => s.id === 'glass')!;
  // 同一套笔画：水墨从右列落墨，玻璃从焦点展开，而非共同的左扫进度。
  const positions = (style: typeof ink) => codeStrokes(art('codemap', model, style, { t: 0.25 })!.svg).map((s) => s.slice(s.lastIndexOf('/') + 1));
  assert.notDeepEqual(positions(ink), positions(glass));
});

test('贡献者的文字与比例保持真值，材质展开不改变分段宽度', () => {
  const bars = (svg: string) => [...normalized(svg).replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect x="[^"]+" y="[^"]+" width="[^"]+" height="14"[^>]*\/>/g)].map((m) => m[0]);
  for (const st of STYLES) {
    const full = art('people', model, st)!;
    for (const t of [0, 0.25, 0.6]) {
      const frame = art('people', model, st, { t })!;
      assert.deepEqual(text(frame.svg), text(full.svg), st.id);
      assert.deepEqual(bars(frame.svg), bars(full.svg), st.id);
      assert.deepEqual(frame.hits, full.hits, st.id);
    }
  }
});

test('代码地图和贡献者终帧保持原作品', () => {
  for (const [id, style, hash] of [
    ['codemap', 'terminal', 'e63b6734c7cb61275fdc5e65bc190d1cced8d253e56cc9ae0a3e34729a0e802e'],
    ['people', 'minimal', 'fdb44527a2b1f749d280863a942df859b166fccb83d17beed986f0567c90f9db'],
  ]) {
    const st = STYLES.find((s) => s.id === style)!, final = art(id, model, st, { t: 1 })!;
    assert.equal(createHash('sha256').update(normalized(final.svg)).digest('hex'), hash, `${id}/${style}`);
    assert.equal(normalized(final.svg), normalized(art(id, model, st)!.svg));
  }
});
