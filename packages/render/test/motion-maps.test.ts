import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES, raster } from '../src/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

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

test('代码地图拼贴按完整文件片段入位，套版按完整归属色层显现', () => {
  const cutout = STYLES.find((s) => s.id === 'cutout')!;
  const full = art('codemap', model, cutout)!, all = codeStrokes(full.svg);
  const current = new Set(codeStrokes(art('codemap', model, cutout, { t: 0.35 })!.svg));
  const fileHits = full.hits.filter((h) => model.files.some((f) => f.path === h.t));
  let visible = 0, pending = 0;
  for (const hit of fileHits) {
    const strokes = all.filter((stroke) => {
      const [, x, y] = /M([-\d.]+) ([-\d.]+)h/.exec(stroke)!;
      return +x >= hit.x && +x < hit.x + hit.w && +y >= hit.y && +y < hit.y + hit.h;
    });
    if (!strokes.length) continue;
    const shown = strokes.filter((stroke) => current.has(stroke)).length;
    assert.ok(shown === 0 || shown === strokes.length, `${hit.t}: complete file fragment`);
    if (shown) visible++; else pending++;
  }
  assert.ok(visible > 0 && pending > 0);

  const ukiyoe = STYLES.find((s) => s.id === 'ukiyoe')!;
  const colors = (svg: string) => {
    const map = new Map<string, Set<string>>();
    for (const stroke of codeStrokes(svg)) {
      const color = stroke.slice(0, stroke.indexOf('/'));
      (map.get(color) ?? map.set(color, new Set()).get(color)!).add(stroke);
    }
    return map;
  };
  const complete = colors(art('codemap', model, ukiyoe)!.svg), partial = colors(art('codemap', model, ukiyoe, { t: 0.25 })!.svg);
  assert.ok(partial.size > 0 && partial.size < complete.size);
  for (const [color, strokes] of partial) assert.deepEqual(strokes, complete.get(color), `${color}: complete color plate`);
});

test('26风格代码地图的汇总份额条始终完整，不沿数值轴裁开归属', () => {
  const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(st => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(st.id))];
  const summaryBars = (svg: string) => {
    const stack: { tag: string; xml: string }[] = [], bars: { xml: string; x: number; y: number; w: number; parents: string[] }[] = [];
    for (const match of normalized(svg).matchAll(/<\/?([\w:-]+)\b[^>]*>/g)) {
      const [xml, tag] = match;
      if (xml.startsWith('</')) { stack.pop(); continue; }
      if (tag === 'rect' && !stack.some(parent => parent.tag === 'defs') && /\bheight="5"/.test(xml)) {
        bars.push({ xml, x: Number(/\bx="([^"]+)"/.exec(xml)![1]), y: Number(/\by="([^"]+)"/.exec(xml)![1]), w: Number(/\bwidth="([^"]+)"/.exec(xml)![1]), parents: stack.filter(p => p.tag === 'g').map(p => p.xml) });
      }
      if (!xml.endsWith('/>')) stack.push({ tag, xml });
    }
    const y = bars.find(bar => /\bfill-opacity="0\.1"/.test(bar.xml))!.y;
    return bars.filter(bar => bar.y === y);
  };
  for (const st of styles) {
    const full = summaryBars(art('codemap', model, st)!.svg);
    assert.ok(full.length >= 3, st.id);
    for (const t of [0.12, 0.2, 0.5, 0.8]) {
      const frame = art('codemap', model, st, { t })!, svg = normalized(frame.svg), current = summaryBars(svg);
      assert.deepEqual(current.map(bar => bar.xml), full.map(bar => bar.xml), `${st.id}/${t}: true widths and positions`);
      // 保留实际父组的 mask/filter/opacity，栅格化这条真实汇总；完整材质纹理不是裁窗。
      const shares = current.filter(bar => !/\bfill-opacity="0\.1"/.test(bar.xml));
      let body = '', open: string[] = [];
      for (const bar of shares) {
        let same = 0; while (same < open.length && open[same] === bar.parents[same]) same++;
        body += open.slice(same).map(() => '</g>').join('') + bar.parents.slice(same).join('') + bar.xml;
        open = bar.parents;
      }
      body += open.map(() => '</g>').join('');
      const definitions = [...svg.matchAll(/<defs>[\s\S]*?<\/defs>/g)].map(m => m[0]).join('');
      const y = shares[0].y, top = Math.floor(y) - 8;
      const isolated = `<svg xmlns="http://www.w3.org/2000/svg" width="${frame.w}" height="22" viewBox="0 ${top} ${frame.w} 22">${definitions}${body}</svg>`;
      const pixels = raster.rgba(isolated, frame.w);
      const columns = shares.map(bar => {
        const visible: boolean[] = [];
        for (let x = Math.ceil(bar.x + 1); x < Math.floor(bar.x + bar.w - 1); x++) {
          let alpha = 0; for (let yy = 0; yy < pixels.h; yy++) alpha += pixels.data[(yy * pixels.w + x) * 4 + 3];
          visible.push(alpha > 0);
        }
        assert.ok(visible.length > 0, `${st.id}/${t}: inspect even the smallest attribution`);
        return visible;
      });
      if (columns.some(cols => cols.some(Boolean)))
        assert.ok(columns.every(cols => cols.every(Boolean)), `${st.id}/${t}: every visible attribution must reach both real endpoints with no holes along the value axis`);
      if (t >= .2) assert.ok(columns.every(cols => cols.some(Boolean)), `${st.id}/${t}: each attribution has actually appeared`);
    }
  }
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
