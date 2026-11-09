import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, PIECES, raster, STYLES } from '../src/index.ts';
import { Scene, rect } from '../src/svg.ts';
import { move, reveal } from '../src/motion.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const m = buildModel(raw);
// Scene id 每次生成不同，不能把 id 差异误当作动画。
const normalized = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');
const percentages = (svg: string) => [...svg.matchAll(/<text\b[^>]*>(.*?)<\/text>/gs)]
  .map((m) => m[1].replace(/<[^>]*>/g, '')).filter((text) => text.includes('%'));

// 修改前保存的静态作品指纹：涵盖材质钩子、特殊版式与稳定的数据几何。
const staticHashes: Record<string, string> = {
  'tier/minimal/rows': '16bbeb63d34789a312ea7242499977eae6127809928d337f2ed82ddf406cc42f',
  'tier/woodblock/vertical': 'c3378638b7c108860d1839fabcb83c34adae68f73ff1e343d2257088d480b914',
  'tier/cyber/hud': '7c5011e50db297fd3b86fc83ef99a829d9fc77e9b8753fdd14889d92f431b3d9',
  'tier/baroque/podium': '32c788d05dda16d02bbc833ea87618c88259786cd5644094b524024065056451',
  'tier/manga/stats': '455628cbf2290e1c7ce60b843b0b66dc3a9af14e513f3c881cc61c82daf28e3c',
  'roll/cinema/classic': '341d1a72a4428f1fc825e7911cf4ec916c27ca6612ed5a5a5a475e010b02bf71',
  'roll/meadow/ed': '065ea8dde6310aa0ed7913b657fb46b08f7cf749c00a1cb30f405df6b6b26972',
  'roll/cutout/billing': 'bf9e374cf2f3f664a65601e33e40dbf5113074823fd36de83c749f51a864bff0',
  'receipt/manuscript/itemized': '01f2a5291668b0b64ef0233da49d5ffab724fa562065541a1c02a918dfab2464',
  'share/glass/': 'b474cb99b21a41963bd1e377a1609924b6f1b77a155d37f218402174f39e4d65',
  'hand/minimal/': '3570009b6283a8b8f785fd464571fd8122a34cd288f05c2dbeaa3b701b4ae8ce',
  'asterisk/minimal/': '53296b197e2a6d17a5ed7568bf8c3748298f80326e52b06645cddf571965ed15',
  'people/minimal/': 'fdb44527a2b1f749d280863a942df859b166fccb83d17beed986f0567c90f9db',
  'codemap/terminal/': 'e63b6734c7cb61275fdc5e65bc190d1cced8d253e56cc9ae0a3e34729a0e802e',
};

test('作品动作保持修改前的静态终帧', () => {
  for (const [key, hash] of Object.entries(staticHashes)) {
    const [id, style, variant] = key.split('/');
    const st = STYLES.find((s) => s.id === style)!;
    const final = art(id, m, st, { variant: variant || undefined, t: 1 })!;
    assert.equal(createHash('sha256').update(normalized(final.svg)).digest('hex'), hash, key);
    assert.equal(normalized(art(id, m, st, { variant: variant || undefined })!.svg), normalized(final.svg), key);
  }
});

test('百分比在动画中一直是真值，分组总数与模型值不相互矛盾', () => {
  for (const [id, variants] of [['tier', ['rows', 'hud', 'split', 'grouped']], ['roll', ['card']], ['asterisk', ['']], ['people', ['']]] as const) {
    for (const variant of variants) for (const st of [STYLES[0], STYLES.find((s) => s.id === 'terminal')!]) {
      const expected = percentages(art(id, m, st, { variant: variant || undefined, t: 1 })!.svg);
      assert.ok(expected.length);
      for (const t of [0, 0.17, 0.5, 0.83]) assert.deepEqual(percentages(art(id, m, st, { variant: variant || undefined, t })!.svg), expected, `${id}/${variant}/${st.id}/${t}`);
    }
  }
});

test('每个非滚动版式都有实际展开，画布尺寸不随进度跳动', () => {
  for (const id of ['tier', 'roll', 'receipt', 'share', 'hand', 'asterisk', 'people', 'codemap']) {
    for (const variant of PIECES[id].variants ?? ['']) {
      if (variant === 'scroll') continue; // 由 frames 的固定取景框处理。
      for (const st of [STYLES[0], STYLES.find((s) => s.id === 'terminal')!]) {
        const scenes = [0, 0.25, 0.6, 1].map((t) => art(id, m, st, { variant: variant || undefined, t })!);
        assert.equal(new Set(scenes.map((s) => `${s.w}/${s.h}`)).size, 1, `${id}/${variant}`);
        assert.ok(new Set(scenes.map((s) => normalized(s.svg))).size >= 3, `${id}/${variant}/${st.id} is static`);
        for (const scene of scenes) assert.ok(!/NaN|Infinity/.test(scene.svg), `${id}/${variant}`);
      }
    }
  }
});

test('动画分组裁剪命中区域，终帧不会增加容器或裁剪定义', () => {
  const s = new Scene(100, 100);
  reveal(s, 0.5, { x: 10, y: 10, w: 80, h: 40 }, 'right', () => s.hit(10, 10, 80, 40, { t: 'data' }));
  assert.deepEqual(s.hits[0], { x: 10, y: 10, w: 40, h: 40, t: 'data' });
  reveal(s, 0, { x: 10, y: 60, w: 80, h: 30 }, 'down', () => s.hit(10, 60, 80, 30, { t: 'hidden' }));
  assert.equal(s.hits.length, 1);
  const moved = new Scene(100, 100);
  move(moved, 0.5, 20, 10, () => moved.hit(10, 10, 20, 20, { t: 'moving' }));
  assert.deepEqual(moved.hits[0], { x: 20, y: 15, w: 20, h: 20, t: 'moving' });
  const full = new Scene(100, 100), plain = new Scene(100, 100);
  reveal(full, 1, { x: 0, y: 0, w: 100, h: 100 }, 'up', () => full.add(rect(5, 5, 20, 20)));
  plain.add(rect(5, 5, 20, 20));
  assert.equal(full.toString(), plain.toString());
});

test('特殊材质的中间帧可被实际 SVG 栅格器绘制', () => {
  for (const key of Object.keys(staticHashes)) {
    const [id, style, variant] = key.split('/');
    const st = STYLES.find((s) => s.id === style)!;
    const a = art(id, m, st, { variant: variant || undefined, t: 0.47 })!;
    const png = raster.png(a.svg, 160);
    assert.deepEqual([...png.slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10], key);
  }
});

test('星号各材质保持完整瓣形，显示的贡献比例始终不变', () => {
  const scene = (style: string, t: number) => art('asterisk', m,
    ['cinema', 'playbill'].includes(style) ? bridgeStyle(THEMES.find((s) => s.id === style)!) : STYLES.find((s) => s.id === style)!, { t })!.svg;
  const cyber = scene('cyber', 0.24), cutout = scene('cutout', 0.24);
  // 墨色变化不能靠矩形裁切花瓣，任意中间帧仍应有完整圆头。
  for (const style of ['ink', 'woodblock']) for (const t of [0.12, 0.38, 0.72]) {
    assert.doesNotMatch(scene(style, t), /<clipPath|clip-path=/, `${style}/${t}`);
  }
  assert.match(cyber, /stroke-dasharray="[\d.]+" stroke-dashoffset="[\d.]+"/);
  assert.ok(!cyber.includes('pathLength') && !cyber.includes('path-length'));
  assert.match(cutout, /<g transform="translate\([^)]*\) rotate\(/);
  assert.ok(!cutout.includes('petal-ink-'));
  // 临时描线和纸片变换复用最终瓣形；样例五瓣每瓣都有 26 个 M/L 顶点。
  const petals = (svg: string) => [...svg.matchAll(/<path\b[^>]*d="([^"]+)"/g)]
    .map((match) => match[1]).filter((d) => (d.match(/[ML]/g) ?? []).length === 26 && d.endsWith('Z'));
  for (const style of ['ink', 'woodblock', 'cyber', 'cutout', 'grimoire', 'meadow', 'cinema', 'playbill']) {
    const final = scene(style, 1), finalPetals = new Set(petals(final));
    assert.equal(finalPetals.size, 5, style);
    for (const t of [0.12, 0.38, 0.72]) {
      const middle = scene(style, t);
      if (style !== 'ink') assert.deepEqual(new Set(petals(middle)), finalPetals, `${style}/${t}`);
      assert.deepEqual(percentages(middle), percentages(final), `${style}/${t}`);
      assert.ok(raster.png(middle, 160).length > 100, `${style}/${t}`);
    }
  }
});
