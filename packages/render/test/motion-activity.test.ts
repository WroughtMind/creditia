import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES } from '../src/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id))];
const normalize = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');

test('26 个产品风格的作息与日历终帧保持 ca5869a 静态作品', () => {
  const hashes = {
    clock: 'dcdcef9b7cc2836d2b44d21db32f467d8088734a3fa22f8c3f0991662592bc38',
    calendar: 'f0e49fe0d8718463f3dbd7d5ca7cb9d114aa5e2a72c9dd5703cca8c16aea2125',
  };
  for (const [id, expected] of Object.entries(hashes)) {
    const svg = styles.map(st => normalize(art(id, model, st, { t: 1 })!.svg)).join('\n');
    assert.equal(createHash('sha256').update(svg).digest('hex'), expected, id);
  }
});

test('屏幕标记准备过程不改变圆点大小、日期坐标或真实活动浓度', () => {
  const marks = (svg: string, id: string) => [...normalize(svg).matchAll(id === 'clock'
    ? /<circle cx="[^"]+" cy="[^"]+" r="[^"]+"[^>]*opacity="[^"]+"[^>]*\/>/g
    : /<rect x="[^"]+" y="[^"]+"[^>]*opacity="[^"]+"[^>]*\/>/g)].map(m => m[0]);
  for (const st of styles.filter(s => ['terminal', 'crt', 'neon'].includes(s.id))) for (const id of ['clock', 'calendar']) {
    const full = new Set(marks(art(id, model, st)!.svg, id));
    for (const t of [0.1, 0.3, 0.5, 0.7, 0.9]) {
      const svg = art(id, model, st, { t })!.svg;
      for (const mark of marks(svg, id)) {
        // 未落定时的 1.2px 网点是原来的位置底托，不表示活动量。
        if (id === 'clock' && / r="1.2"/.test(mark)) continue;
        assert.ok(full.has(mark), `${st.id}/${id}/${t}: ${mark}`);
      }
      assert.doesNotMatch(svg, /<g[^>]+(?:transform|filter|clip-path)=/, `${st.id}/${id}: data is not transformed or clipped`);
    }
  }
});
