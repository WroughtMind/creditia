import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES, raster } from '../src/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const model = buildModel(raw);
const cases: Record<string, string[]> = {
  share: [''], hand: [''], receipt: ['slip', 'ticket', 'itemized'],
  badge: ['flat', 'stack', 'emblem', 'split'], byline: ['line', 'models'],
};
const normalize = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');
const hash = (text: string | Uint8Array) => createHash('sha256').update(text).digest('hex');
const texts = (svg: string) => [...svg.matchAll(/<text\b[^>]*>(.*?)<\/text>/gs)].map((m) => m[1].replace(/<[^>]*>/g, ''));

test('手稿与排版圆章的弧形题字不生成圆外水平批注', () => {
  const styles = [STYLES.find(s => s.id === 'manuscript')!, bridgeStyle(THEMES.find(s => s.id === 'editorial')!)];
  for (const style of styles) for (const t of [0.2, 0.5, 0.8]) {
    const svg = art('badge', model, style, { variant: 'emblem', t })!.svg;
    const annotations = [...svg.matchAll(/<line\b[^>]*y1="([\d.]+)"[^>]*stroke="([^\"]+)"[^>]*>/g)].filter(m => m[2] === style.c.accent);
    assert.ok(annotations.every(m => Number(m[1]) < 200), `${t}: curved lettering has no straight underline outside the seal`);
    assert.match(svg, /<textPath\b[^>]*>[^<]+<\/textPath>/, `${t}: whole curved lettering`);
  }
});

// 16 风格静态指纹，按版式汇总；终端已移除纸面外不可见的代码雨字形。
const staticHashes: Record<string, string> = {
  'share/': 'b82c961bc1238317ca1be610a5707fb7c1d7712278070bce840f5e1ccd90730b',
  'hand/': 'e058382f55737fb7d4b00f1b31e48bf51b080685342ede684faa6c574524880b',
  'receipt/slip': 'df6924565543c7776523051801fa6907893186b9c8e7d37b80cdd5dc222b1c4a',
  'receipt/ticket': 'ccdbc434ee4a5dd894011e030b93689bd14b6bd13836140f07324f1d0367c297',
  'receipt/itemized': 'c10b6f081d0ead8334e080dfda409b7d6a17ee511515e9580aaccf764364fef7',
  'badge/flat': 'b64c8da82bd8667c893d142186d4e3b4ccd967e8fb0ee45da14fe7b8fb995100',
  'badge/stack': '728b578aac2cbb45e13094746c141038ea6354ffeb55133f5e87bc7241ca04b4',
  'badge/emblem': '33e2bf28c55f43b48fd6dad9e8a6b4e336ec5f81cff8449a3d1cdccf2e79a012',
  'badge/split': 'b4d4237b99c730343cc2e340594f6d1d84ad1571bb42f169fa9c685aa6c41331',
  'byline/line': 'd19794fdbc3b68396747e348e95be8247d33fd4d5a7942af3f00c449b98091c6',
  'byline/models': '85ce010363b4c1d0cf351ddcfbaa21c1d8f7819a1b5d42f6361f2a47d5d93a92',
};

test('cards 全部 176 个组合的静态终帧保持原样', () => {
  for (const [id, variants] of Object.entries(cases)) for (const variant of variants) {
    const fingerprints = STYLES.map((st) => {
      const a = art(id, model, st, { variant: variant || undefined, t: 1 })!;
      assert.equal(normalize(art(id, model, st, { variant: variant || undefined })!.svg), normalize(a.svg));
      return { id, variant, style: st.id, w: a.w, h: a.h, hash: hash(normalize(a.svg)) };
    });
    assert.equal(hash(JSON.stringify(fingerprints)), staticHashes[`${id}/${variant}`], `${id}/${variant}`);
  }
});

test('cards 全版式全风格有实际动作，画布和显示的数据保持稳定', () => {
  for (const [id, variants] of Object.entries(cases)) for (const variant of variants) for (const st of STYLES) {
    const key = `${id}/${variant}/${st.id}`;
    const scenes = [0, 0.16, 0.5, 0.86, 1].map((t) => art(id, model, st, { variant: variant || undefined, t })!);
    const finalTexts = new Set(texts(scenes.at(-1)!.svg));
    assert.equal(new Set(scenes.map((a) => `${a.w}/${a.h}`)).size, 1, key);
    assert.ok(new Set(scenes.map((a) => normalize(a.svg))).size >= 4, `${key}: no motion`);
    for (const a of scenes) {
      assert.doesNotMatch(a.svg, /NaN|Infinity/, key);
      for (const value of texts(a.svg)) assert.ok(finalTexts.has(value), `${key}: changed text ${value}`);
    }
  }
});

test('cards 的动画遮罩不会扫过文字，票面和完整图标直接落位', () => {
  for (const [id, variants] of Object.entries(cases)) for (const variant of variants) for (const st of STYLES) for (const t of [0.16, 0.5, 0.86]) {
    const key = `${id}/${variant}/${st.id}/${t}`;
    const svg = art(id, model, st, { variant: variant || undefined, t })!.svg;
    const stack: boolean[] = [];
    for (const [tag] of svg.matchAll(/<\/?g\b[^>]*>|<text\b[^>]*>/g)) {
      if (tag.startsWith('</g')) stack.pop();
      else if (tag.startsWith('<g')) { if (!tag.endsWith('/>')) stack.push(/clip-path="url\(#[^"]*-(?:motion|material)-/.test(tag)); }
      else assert.ok(!stack.some(Boolean), `${key}: text inside animated clip`);
    }

  }
});

test('徽记沿圆周描线，票根按完整条打印，模型署名分段进入', () => {
  const st = STYLES[0];
  const emblem = art('badge', model, st, { variant: 'emblem', t: 0.5 })!.svg;
  assert.match(emblem, /<circle[^>]*stroke-dasharray="[\d.]+" stroke-dashoffset="[\d.]+"/);
  assert.doesNotMatch(emblem, /<clipPath/);
  assert.match(emblem, /<textPath[^>]*>· catnap/);
  const bars = (t: number) => [...art('receipt', model, st, { variant: 'ticket', t })!.svg.matchAll(/<rect\b[^>]*height="90"/g)].length;
  assert.ok(bars(0.5) > 0 && bars(0.5) < bars(0.8));
  assert.equal(bars(1), 44);
  const byline = art('byline', model, st, { variant: 'models', t: 0.5 })!.svg;
  assert.ok((byline.match(/transform="translate\(/g) ?? []).length > 1);
});

test('只有提示词和审阅、没有留存条或周曲线时，开发者卡仍有动作', () => {
  const m = { ...model, timeline: [], hand: { ...model.hand, written: 0, committed: 0, surviving: 0, prompts: 12, reviews: 3 } };
  const stages = [0, 0.6, 0.8, 1].map((t) => art('hand', m, STYLES[0], { t })!);
  assert.equal(new Set(stages.map((a) => normalize(a.svg))).size, 4);
  assert.equal(new Set(stages.map((a) => `${a.w}/${a.h}`)).size, 1);
});

test('真实栅格器验证各版式纸面与发光材质的早中晚帧都有像素变化', () => {
  for (const [id, variants] of Object.entries(cases)) for (const variant of variants) for (const style of ['woodblock', 'cyber']) {
    const st = STYLES.find((st) => st.id === style)!;
    const pixels = [0.16, 0.5, 0.86].map((t) => raster.png(art(id, model, st, { variant: variant || undefined, t })!.svg, 240));
    assert.equal(new Set(pixels.map(hash)).size, 3, `${id}/${variant}/${style}`);
    for (const png of pixels) assert.deepEqual([...png.slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  }
});

test('终端署名条的越界代码雨不会使静态或动画导出崩溃', () => {
  const st = STYLES.find((style) => style.id === 'terminal')!;
  for (const variant of ['line', 'models']) for (const t of [0, 0.16, 0.5, 1]) {
    const a = art('byline', model, st, { variant, t })!;
    const png = raster.png(a.svg, 430);
    assert.deepEqual([...png.slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  }
});

test('分享图和堆叠徽章沿厚度揭示整张份额图，不逐段改写中间比例', () => {
  for (const st of [...STYLES, ...THEMES.map(bridgeStyle)]) for (const [id, variant, finalHeight] of [['share', '', 42], ['badge', 'stack', 26], ['badge', 'split', 26]] as const) {
    const final = art(id, model, st, { variant: variant || undefined, t: 1 })!;
    const bars = final.hits.filter(hit => hit.h === finalHeight);
    assert.ok(bars.length > 1);
    const middle = art(id, model, st, { variant: variant || undefined, t: .4 })!;
    for (const end of bars) {
      const visible = middle.hits.find(hit => hit.k === end.k && hit.t === end.t && Math.abs(hit.x - end.x) < .01);
      assert.ok(visible, `${st.id}/${id}/${variant}/${end.t}: all shares enter together`);
      assert.equal(visible.w, end.w, `${st.id}/${id}/${variant}/${end.t}: horizontal data width stays true`);
    }
  }
});

test('木刻细徽章份额压印完整条，不用大面刀纹切成条码', () => {
  const style = STYLES.find(s => s.id === 'woodblock')!;
  const paper = style.c.paper.replace('#', '').match(/../g)!.map(value => parseInt(value, 16));
  for (const variant of ['stack', 'split']) {
    const full = art('badge', model, style, { variant, t: 1 })!, final = raster.rgba(full.svg, full.w);
    const y0 = variant === 'stack' ? 42 : 40;
    const inkAt = (data: Uint8Array, x: number) => {
      let difference = 0;
      for (let y = y0 + 1; y < y0 + 13; y++) {
        const pixel = (y * full.w + x) * 4;
        difference = Math.max(difference, ...paper.map((c, channel) => Math.abs(c - data[pixel + channel])));
      }
      return difference;
    };
    for (const t of [0.2, 0.5, 0.8]) {
      const middle = raster.rgba(art('badge', model, style, { variant, t })!.svg, full.w);
      for (let x = 19; x < full.w - 19; x++) {
        const finalInk = inkAt(final.data, x);
        if (finalInk > 12) assert.ok(inkAt(middle.data, x) > finalInk * .3,
          `${variant}/${t}/${x}: each true value-axis column retains continuous base ink`);
      }
    }
  }
});
