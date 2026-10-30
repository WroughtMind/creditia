import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES } from '../src/index.ts';
import { ease } from '../src/svg.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const shapes = (svg: string) => [...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<path\b[^>]*\bd="([^"]+)"/g)].map((m) => m[1]);
const normalized = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-');

// 16 种风格静态终帧；终端只移除纸面外的代码雨，数据几何或文字不能随动画改变。
const chartHashes: Record<string, string> = {
  'donut/ring': 'ce5de68e6fe8e52877d38411cb65275b381d7ef20787c73450ec81eb82f14897',
  'donut/nested': '21274afac9f7380dc4df0f462c1a5e1b286091bcdeacac2b616d16f5157cc597',
  'waffle/grid': '19d5563c2f0b12024a22d6e1373339b8f5bc4b1a82f0e3b7562d731e4fe9cf0d',
  'waffle/blocks': 'a15c972a977ce1d4b7f7be956ae8725f6468adc878e0cc3e677884eeb7d3e21e',
  'stream/default': 'd04a8180ce7211be6566c5c5dc43ccc6fab3a459a7e0f6d119c7d1477d932513',
  'lanes/default': '6ccbdd3105636ced12321da66e3efdd293d518fa1a6196ebf8b51ab111218fe1',
  'clock/default': '16ecd53d2ac0f42771b6333ec131ffd525b4353e65d4807896982363801b6401',
  'versions/default': '5c9448d4716fc4ab26b48fa8e76b775f819d2297eca9114e01bd35e27db6f55b',
  'survival/default': 'c4c1845be764d0c1be3d999c3886d5d00a7774e09a21c1774328280f7d5f464c',
  'mosaic/default': '5de0e58a13c78bb657cc39298d9f60376784f70f7e1f57a4615881aa48fff25f',
  'calendar/default': '51afa8cee9a7a759eaa54d63a8a9b4abd5e5977e561c650d33f5e8d7e160f069',
  'evidence/default': '069f90172fb2764b8d281e859f361bcf771dac0369c3b3d6ca21a4a263e07563',
  'areas/default': '99bf96f2347ae9ef8a59eae8cc3506786ebe98e32bebb0afe06c5cfafb3dbfee',
};

test('全部统计图版式在全部风格中有实际展开，终帧保持原作品', () => {
  for (const [key, hash] of Object.entries(chartHashes)) {
    const [id, name] = key.split('/'), variant = name === 'default' ? undefined : name;
    const finals: string[] = [];
    for (const st of STYLES) {
      const frames = [0, 0.15, 0.5, 1].map((t) => art(id, model, st, { variant, t })!);
      assert.ok(frames.every(Boolean), `${st.id}/${key}`);
      assert.equal(new Set(frames.map((f) => `${f.w}/${f.h}`)).size, 1, `${st.id}/${key}: stable canvas`);
      assert.ok(new Set(frames.map((f) => normalized(f.svg))).size >= 3, `${st.id}/${key}: actual motion`);
      for (const frame of frames) assert.doesNotMatch(frame.svg, /NaN|Infinity/, `${st.id}/${key}`);
      const final = normalized(frames.at(-1)!.svg);
      assert.equal(normalized(art(id, model, st, { variant })!.svg), final, `${st.id}/${key}: static export`);
      finals.push(final);
    }
    assert.equal(createHash('sha256').update(finals.join('\n')).digest('hex'), hash, key);
  }
});

test('华夫饼逐格点数，分块版也在工具块内部继续铺开，格子面积不变', () => {
  for (const st of STYLES) for (const variant of ['grid', 'blocks']) {
    const full = art('waffle', model, st, { variant })!;
    const size = Math.min(...full.hits.map((h) => h.w));
    const cells = (t: number) => art('waffle', model, st, { variant, t })!.hits.filter((h) => h.w === size && h.h === size);
    assert.equal(cells(0).length, 0, `${st.id}/${variant}: starts empty`);
    assert.equal(cells(1).length, 100, `${st.id}/${variant}: one hundred percent`);
    const earlier = cells(0.15), later = cells(0.5);
    assert.ok(earlier.length > 0 && later.length > earlier.length, `${st.id}/${variant}: cells continue appearing`);
    if (st.id === 'minimal' && variant === 'blocks') {
      // 两个采样点都在第一个工具的阶段，排除“整块瞬间出现”的回归。
      const first = cells(0.02), next = cells(0.08);
      assert.ok(first.length > 0 && next.length > first.length);
      assert.ok(next.every((h) => h.k === model.tools[0].key || h.k?.startsWith(`${model.tools[0].key}/`)));
    }
    const finalByPosition = new Map(cells(1).map((h) => [`${h.x}/${h.y}`, h]));
    for (const cell of [...earlier, ...later, ...cells(0.5)]) assert.deepEqual(cell, finalByPosition.get(`${cell.x}/${cell.y}`));
  }
});

test('出处结论和板块百分比始终完整，不放进动画裁切组', () => {
  for (const st of STYLES) for (const id of ['evidence', 'areas']) for (const t of [0.15, 0.5, 0.79, 0.93]) {
    const svg = art(id, model, st, { t })!.svg;
    const clipped: boolean[] = [];
    for (const match of svg.matchAll(/<g\b([^>]*)>|<\/g>|<text\b[^>]*>([\s\S]*?)<\/text>/g)) {
      if (match[0].startsWith('<g')) clipped.push(!!clipped.at(-1) || /clip-path="url\(#[^"]*(?:motion-|material-)/.test(match[1]));
      else if (match[0] === '</g>') clipped.pop();
      else if (match[2].includes('%')) assert.ok(!clipped.at(-1), `${st.id}/${id}/${t}: ${match[2]}`);
    }
  }
});

test('终端和手稿的字符条不被动画切成半个字', () => {
  for (const st of STYLES.filter((s) => ['terminal', 'manuscript'].includes(s.id))) for (const id of ['survival', 'evidence']) for (const t of [0.15, 0.5, 0.8]) {
    const stack: boolean[] = [];
    for (const match of art(id, model, st, { t })!.svg.matchAll(/<g\b([^>]*)>|<\/g>|<text\b[^>]*>([\s\S]*?)<\/text>/g)) {
      if (match[0].startsWith('<g')) stack.push(!!stack.at(-1) || /clip-path="url\(#[^"]*material-/.test(match[1]));
      else if (match[0] === '</g>') stack.pop();
      else assert.ok(!stack.at(-1), `${st.id}/${id}/${t}: ${match[2]}`);
    }
  }
});

test('作息圆点显示时已经是最终大小和强度，不把进度误作提交量', () => {
  const circles = (svg: string) => new Map([...svg.matchAll(/<circle cx="([^"]+)" cy="([^"]+)" r="([^"]+)"[^>]*opacity="([^"]+)"[^>]*\/>/g)]
    .filter((m) => Number(m[3]) >= 1.5).map((m) => [`${m[1]}/${m[2]}`, m[0]]));
  for (const st of STYLES) {
    const full = circles(art('clock', model, st)!.svg);
    const halfway = circles(art('clock', model, st, { t: 0.5 })!.svg);
    assert.ok(halfway.size > 0 && halfway.size < full.size, st.id);
    for (const [position, circle] of halfway) assert.equal(circle, full.get(position), `${st.id}/${position}`);
  }
});

test('留存分阶段显现，所有中间帧的条长始终是真实留存量', () => {
  const list = model.rows.filter((a) => a.kind === 'agent' && a.lines > 0);
  for (const st of STYLES) {
    const widths = (t: number) => {
      const values: number[] = [];
      art('survival', model, { ...st, bar(s, x, y, w, h, color, i) { values.push(w); st.bar(s, x, y, w, h, color, i); } }, { t });
      return values;
    };
    const retained = widths(1);
    assert.equal(retained.length, list.length);
    for (const t of [0, 0.15, 0.45, 0.7, 0.9]) assert.deepEqual(widths(t), retained, `${st.id}/${t}: no invented retention`);
  }
});

test('马赛克文件块移动时悬停区跟着移动，尚未出现的文件不抢悬停', () => {
  const st = STYLES[0], full = art('mosaic', model, st)!;
  const fileHits = (t: number) => art('mosaic', model, st, { t })!.hits.filter((h) => model.files.some((f) => f.path === h.t));
  assert.equal(fileHits(0).length, 0);
  const middle = fileHits(0.1);
  assert.ok(middle.length > 0 && middle.length < model.files.length);
  let moved = false;
  for (const hit of middle) {
    const final = full.hits.find((h) => h.t === hit.t)!;
    assert.equal(hit.w, final.w);
    assert.equal(hit.h, final.h);
    assert.deepEqual(hit.d, final.d);
    moved ||= hit.x !== final.x || hit.y !== final.y;
  }
  assert.ok(moved);
});

test('比例环揭示真实图形，中间帧不移动扇区', () => {
  for (const style of STYLES) for (const [id, variant] of [['donut', 'ring'], ['donut', 'nested']]) {
    if (style.cells) continue;
    const full = art(id!, model, style, { variant })!;
    for (const t of [0, 0.2, 0.6]) {
      const frame = art(id!, model, style, { variant, t })!;
      assert.deepEqual(shapes(frame.svg), shapes(full.svg), `${style.id}/${id}/${variant}/${t}`);
      assert.equal(frame.w, full.w);
      assert.equal(frame.h, full.h);
      // 尚未揭示的扇区可以不接收悬停；已可见的热区必须仍对应相同真值和位置。
      for (const hit of frame.hits) assert.ok(full.hits.some((final) => final.k === hit.k && final.t === hit.t
        && JSON.stringify(final.d) === JSON.stringify(hit.d) && hit.x >= final.x - 0.02 && hit.y >= final.y - 0.02
        && hit.x + hit.w <= final.x + final.w + 0.02 && hit.y + hit.h <= final.y + final.h + 0.02), `${style.id}: hit remains inside its data shape`);
    }
  }
});

test('环形材质沿圆心显现，不用条形扫描窗切出直角断面', () => {
  for (const st of STYLES) for (const variant of ['ring', 'nested']) {
    const svg = art('donut', model, st, { variant, t: 0.46 })!.svg;
    assert.doesNotMatch(svg, /<clipPath[^>]*material-/, `${st.id}/${variant}: no rectangular material window`);
    assert.deepEqual(shapes(svg), shapes(art('donut', model, st, { variant })!.svg), `${st.id}/${variant}: fixed sectors`);
  }
});

test('河流逐周从中线展开，同一周各层占比不变，文字和悬停数据保持真值', () => {
  const bands = (svg: string) => [...svg.matchAll(/<path d="([^"]+)"[^>]*opacity="0\.92"[^>]*stroke-width="0\.6"/g)].map((m) =>
    [...m[1].matchAll(/[MLC]([^MLCZ]+)/g)].map((segment) => segment[1].trim().split(/[ ,]+/).map(Number).slice(-2)));
  const text = (svg: string) => [...normalized(svg).matchAll(/<text\b[^>]*>[\s\S]*?<\/text>/g)].map((m) => m[0]);
  const n = model.weeks.length;
  for (const style of STYLES) {
    const full = art('stream', model, style)!;
    const final = bands(full.svg);
    for (const t of [0.2, 0.5, 0.8]) {
      const frame = art('stream', model, style, { t })!;
      assert.equal(frame.w, full.w);
      assert.equal(frame.h, full.h);
      assert.deepEqual(frame.hits, full.hits);
      assert.deepEqual(text(frame.svg), text(full.svg));
      assert.ok(!frame.svg.includes('motion-'), `${style.id}: no rectangular scan edge`);
      if (style.cells) continue;
      const current = bands(frame.svg);
      assert.equal(current.length, final.length);
      assert.ok(current.length > 1);
      for (let i = 0; i < n; i++) {
        const bottom = 2 * n - 1 - i;
        const fullHeight = final.at(-1)![bottom][1] - final[0][i][1];
        const height = current.at(-1)![bottom][1] - current[0][i][1];
        const scale = height / fullHeight;
        if (style.id === 'ink' && fullHeight > 0) assert.ok(Math.abs(scale - ease(t, i, n, 0.5)) < 0.001, `ink/${t}/${i}: original flow`);
        assert.ok(Math.abs(current[0][i][1] + current.at(-1)![bottom][1] - final[0][i][1] - final.at(-1)![bottom][1]) < 0.03, `${style.id}/${t}/${i}: centered`);
        for (let band = 0; band < final.length; band++) {
          assert.equal(current[band][i][0], final[band][i][0]);
          const actual = current[band][bottom][1] - current[band][i][1];
          const expected = (final[band][bottom][1] - final[band][i][1]) * scale;
          assert.ok(Math.abs(actual - expected) < 0.03, `${style.id}/${t}/${i}/${band}: share`);
        }
      }
    }
  }
});

test('河流终帧保持原静态作品', () => {
  const hashes = {
    minimal: 'b4a478dd5e16b7589b76d47e768677ebdca49dce284d50e0e099eb6c8c90d00b',
    ink: 'd749e2b6741bdffc7e6b3ef50e68d26c7956b377f294ccc32a7433b3d5b43710',
    terminal: '50f9d8ec08c88002bbe6dcbeef1bb7ab25a2365853ba4c4e1580a764a235ecf3',
  };
  for (const [style, hash] of Object.entries(hashes)) {
    const svg = art('stream', model, STYLES.find((s) => s.id === style)!, { t: 1 })!.svg;
    assert.equal(createHash('sha256').update(normalized(svg)).digest('hex'), hash, style);
  }
});

test('日历中已出现的日期沿用最终活动强度，不把淡入程度当成活跃量', () => {
  const cells = (svg: string) => new Map([...svg.matchAll(/<rect x="([^"]+)" y="([^"]+)"[^>]*opacity="([^"]+)"[^>]*\/>/g)].map((m) => [`${m[1]},${m[2]}`, m[3]]));
  for (const style of STYLES) {
    const full = cells(art('calendar', model, style)!.svg);
    const halfway = cells(art('calendar', model, style, { t: 0.5 })!.svg);
    assert.ok(halfway.size < full.size, style.id);
    for (const [position, opacity] of halfway) assert.equal(opacity, full.get(position), `${style.id}/${position}`);
  }
});

test('泳道的所有工具共用时间扫描边界，时间段和热区保持完整', () => {
  const style = STYLES[0];
  const full = art('lanes', model, style)!;
  const halfway = art('lanes', model, style, { t: 0.5 })!;
  assert.deepEqual(halfway.hits, full.hits);
  const widths = [...halfway.svg.matchAll(/<clipPath id="[^"]*material-[^"]*"[^>]*><path d="([^"]+)"/g)].map((m) => {
    const numbers = m[1].match(/-?[\d.]+/g)!.map(Number), x = numbers.filter((_, i) => i % 2 === 0);
    return Math.max(...x) - Math.min(...x);
  });
  assert.ok(widths.length > 1);
  assert.equal(new Set(widths).size, 1);
  const segments = (svg: string) => [...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect\b[^>]*stroke-width="1\.4"[^>]*>/g)].map((m) => m[0]);
  assert.deepEqual(segments(halfway.svg), segments(full.svg));
});
