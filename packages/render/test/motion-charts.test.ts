import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES, raster } from '../src/index.ts';
import { ease, Scene } from '../src/svg.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

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
    const cells = (t: number) => art('waffle', model, st, { variant, t })!.hits.filter((h) => h.w < size * 1.1 && h.h < size * 1.1);
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
    const finalCells = cells(1);
    const tiles = (svg: string) => [...normalized(svg).replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect\b[^>]*width="([\d.]+)" height="([\d.]+)"[^>]*\/>/g)]
      .filter((m) => Math.abs(Number(m[1]) - size) < 0.02 && Math.abs(Number(m[2]) - size) < 0.02).map((m) => m[0]);
    const finalTiles = new Set(tiles(full.svg));
    for (const t of [0.15, 0.5]) {
      const frame = art('waffle', model, st, { variant, t })!;
      assert.ok(tiles(frame.svg).every((tile) => finalTiles.has(tile)), `${st.id}/${variant}: full-size one-percent tiles`);
      const scaled = (svg: string) => [...normalized(svg).matchAll(/<g[^>]*transform="[^"]*scale\([^>]*>/g)].map((m) => m[0]);
      assert.deepEqual(scaled(frame.svg), scaled(full.svg), `${st.id}: only pre-existing static ornament scales`);
    }
    for (const cell of [...earlier, ...later]) assert.ok(finalCells.some((final) => final.k === cell.k && final.t === cell.t
      && JSON.stringify(final.d) === JSON.stringify(cell.d) && Math.abs(final.x - cell.x) < 16 && Math.abs(final.y - cell.y) < 16), `${st.id}: hit follows its complete tile`);
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
    let partial = false;
    for (const t of [.15, .35, .5, .8]) {
      const current = circles(art('clock', model, st, { t })!.svg);
      partial ||= current.size > 0 && current.size < full.size;
      for (const [position, circle] of current) assert.equal(circle, full.get(position), `${st.id}/${t}/${position}`);
    }
    // 整组可以在半程前已显示、继续刚性归位；真实值不依赖恰好 t=.5 时还缺几个点。
    assert.ok(partial, `${st.id}: intermediate groups appear before the complete work`);
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

test('26风格留存和证据条显现时保留完整数值宽度，不被横向材质窗裁短', () => {
  const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(st => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(st.id))];
  for (const st of styles) for (const id of ['survival', 'evidence']) {
    const bars = (t: number) => {
      let n = 0;
      const frame = art(id, model, { ...st, bar(s, x, y, w, h, color, i) {
        st.bar(s, x, y, w, h, color, i);
        // Probe the visible bounds too: comparing source rect widths alone can
        // miss a material clip that leaves only a false fraction on screen.
        s.hit(x, y, w, h, { k: `numeric-bar-${n++}`, t: 'numeric-bar' });
      } }, { t })!;
      return frame.hits.filter(hit => hit.t === 'numeric-bar');
    };
    const full = new Map(bars(1).map(hit => [hit.k, hit]));
    assert.ok(full.size > 0, `${st.id}/${id}`);
    for (const t of [0.15, 0.3, 0.5, 0.7, 0.9]) for (const hit of bars(t)) {
      const final = full.get(hit.k)!;
      assert.ok(Math.abs(hit.x - final.x) < 0.025 && Math.abs(hit.w - final.w) < 0.025, `${st.id}/${id}/${t}/${hit.k}: complete value axis`);
    }
  }
});

test('26风格版本列出现时具有真实全高和完整上下份额，不从统一窗口长高', () => {
  const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(st => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(st.id))];
  const add = Scene.prototype.add;
  for (const st of styles) {
    const full = art('versions', model, st)!;
    const columns = full.hits.filter(hit => model.history.buckets.some(bucket => bucket.tag === hit.t || bucket.tag === 'unreleased' && ['未发布', 'Unreleased'].includes(hit.t)));
    assert.equal(columns.length, model.history.buckets.length, st.id);
    // Observe the actual painted stacked rectangles inside their material
    // groups, so a fixed plot-height crop cannot pass by retaining source data.
    Scene.prototype.add = function (...parts) {
      const result = add.apply(this, parts);
      for (const part of parts) if (typeof part === 'string') for (const match of part.matchAll(/<rect x="([^"]+)" y="([^"]+)" width="([^"]+)" height="([^"]+)"/g)) {
        const [x, y, w, h] = match.slice(1).map(Number);
        if (columns.some(column => Math.abs(x - column.x - 10) < 0.025 && Math.abs(w - column.w + 20) < 0.025 && y >= column.y && y + h <= column.y + column.h - 40.0 + 0.025)) {
          this.hit(x, y, w, h, { k: `${x}/${y}/${w}/${h}`, t: 'version-rectangle' });
        }
      }
      return result;
    };
    try {
      const rectangles = (t: number) => art('versions', model, st, { t })!.hits.filter(hit => hit.t === 'version-rectangle');
      const final = new Map(rectangles(1).map(hit => [hit.k, hit]));
      assert.ok(final.size > columns.length, st.id);
      for (const t of [0.15, 0.3, 0.5, 0.8]) for (const hit of rectangles(t)) {
        const original = final.get(hit.k)!;
        assert.ok(Math.abs(hit.y - original.y) < 0.025 && Math.abs(hit.h - original.h) < 0.025, `${st.id}/${t}/${hit.k}: true column height and ownership`);
      }
    } finally { Scene.prototype.add = add; }
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

test('比例环保持真角度；完整纸弧刚性归位时命中区随同移动', () => {
  for (const style of STYLES) for (const [id, variant] of [['donut', 'ring'], ['donut', 'nested']]) {
    if (style.cells) continue;
    const full = art(id!, model, style, { variant })!;
    for (const t of [0, 0.2, 0.6]) {
      const frame = art(id!, model, style, { variant, t })!;
      assert.deepEqual(shapes(frame.svg), shapes(full.svg), `${style.id}/${id}/${variant}/${t}`);
      assert.equal(frame.w, full.w);
      assert.equal(frame.h, full.h);
      // 纸弧可以整体搬动，不能缩放份额；悬停区必须采用实际绘图的同一个位移。
      if (['pastel', 'cutout'].includes(style.id)) {
        const moves = [[0, 0], ...[...frame.svg.matchAll(/<g transform="translate\(([-\d.]+) ([-\d.]+)\)"/g)].map(m => [+m[1], +m[2]])];
        for (const hit of frame.hits) assert.ok(full.hits.some(final => final.k === hit.k && final.t === hit.t
          && JSON.stringify(final.d) === JSON.stringify(hit.d) && final.w === hit.w && final.h === hit.h
          && moves.some(([x, y]) => Math.abs(hit.x - final.x - x) < .02 && Math.abs(hit.y - final.y - y) < .02)), `${style.id}: rigid arc and hit move together`);
        continue;
      }
      // 尚未揭示的扇区可以不接收悬停；原位成形的热区仍对应同一真值和位置。
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

test('环带中间帧在每份真实角度都有像素，不把份额揭成临时短弧', () => {
  const equal = { ...model, rows: model.rows.slice(0, 2).map(a => ({ ...a, pct: 50 })) };
  for (const st of STYLES.filter(s => ['ink', 'cinema', 'glass', 'pastel', 'manga', 'cyber'].includes(s.id))) {
    const full = art('donut', equal, st, { variant: 'ring' })!;
    const hit = full.hits.find(h => h.w === 60 && h.h === 60)!;
    const cx = hit.x + 30 - 220, cy = hit.y + 30;
    const empty = raster.rgba(art('donut', equal, st, { variant: 'ring', t: 0 })!.svg, 500);
    for (const t of [.3, .5]) {
      const frame = raster.rgba(art('donut', equal, st, { variant: 'ring', t })!.svg, 500);
      for (const angle of [-Math.PI / 4, Math.PI / 4, Math.PI * 3 / 4, Math.PI * 5 / 4]) {
        let changed = 0;
        for (let radius = 172; radius < 269; radius += 2) {
          const x = Math.round((cx + Math.cos(angle) * radius) / 2), y = Math.round((cy + Math.sin(angle) * radius) / 2);
          const i = (y * frame.w + x) * 4;
          if (Math.max(...[0, 1, 2].map(c => Math.abs(frame.data[i + c] - empty.data[i + c]))) > 8) changed++;
        }
        assert.ok(changed >= 2, `${st.id}/${t}/${angle}: every actual share keeps its full angle`);
      }
    }
  }
});

test('河流材质成形时各周比例保持真值，水墨仍从中线自然生长', () => {
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
      // A vertical time-scan edge is valid: it preserves the whole stacked
      // cross-section. The raster regression below rejects partial layers.
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

test('屏幕与漫画河流每个已出现的时间列保留完整上下截面', () => {
  const keys = model.rows.filter(row => row.kind === 'agent').slice(0, 2).map(row => row.key);
  const flat = { ...model, weeks: model.weeks.map(week => ({ ...week, by: { [keys[0]]: 70, [keys[1]]: 30 } })) };
  const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(style => ['cyber', 'manga'].includes(style.id))];
  for (const st of styles.filter(style => ['crt', 'terminal', 'cyber', 'manga', 'gala'].includes(style.id))) {
    const full = art('stream', flat, st)!;
    const first = full.hits.find(hit => hit.t === flat.weeks[0].week)!, last = full.hits.find(hit => hit.t === flat.weeks.at(-1)!.week)!;
    const left = first.x + first.w / 2, right = last.x + last.w / 2;
    const final = raster.rgba(full.svg, 500), empty = raster.rgba(art('stream', flat, st, { t: 0 })!.svg, 500);
    const scale = 500 / full.w;
    const difference = (a: typeof final, b: typeof final, x: number, y: number) => {
      const i = (y * a.w + x) * 4;
      return Math.max(...[0, 1, 2].map(channel => Math.abs(a.data[i + channel] - b.data[i + channel])));
    };
    for (const t of [.2, .5, .8]) {
      const current = raster.rgba(art('stream', flat, st, { t })!.svg, 500);
      let shownColumns = 0, truncatedColumns = 0;
      for (let x = Math.ceil(left * scale) + 2; x < Math.floor(right * scale) - 2; x += 3) {
        const exposure: number[] = [];
        for (let y = Math.ceil(first.y * scale) + 2; y < Math.floor((first.y + first.h) * scale) - 2; y++) {
          const expected = difference(final, empty, x, y);
          if (expected < 16) continue;
          exposure.push(difference(current, empty, x, y) / expected);
        }
        exposure.sort((a, b) => a - b);
        const low = exposure[Math.floor(exposure.length * .1)], high = exposure[Math.floor(exposure.length * .9)];
        if (!(high >= .18)) continue;
        shownColumns++;
        // Soft light may attenuate a complete column uniformly. Missing lower
        // ownership produces zero exposure in part of that same column.
        if (low < high * .55) truncatedColumns++;
      }
      assert.ok(shownColumns > 0, `${st.id}/${t}: time columns appear`);
      // One anti-aliased frontier column is harmless; a horizontal/diagonal
      // crop removes ownership across many time columns and must fail.
      assert.ok(truncatedColumns <= 1, `${st.id}/${t}: ${truncatedColumns} truncated time columns`);
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
  const cells = (svg: string, top: number, bottom: number) => new Map([...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect x="([^"]+)" y="([^"]+)"[^>]*opacity="([^"]+)"[^>]*\/>/g)]
    .filter(m => Number(m[2]) >= top && Number(m[2]) < bottom).map((m) => [`${m[1]},${m[2]}`, m[3]]));
  for (const style of STYLES) {
    const end = art('calendar', model, style)!;
    const dates = end.hits.filter(hit => /^\d{4}-\d{2}-\d{2}$/.test(hit.t));
    const top = Math.min(...dates.map(hit => hit.y)), bottom = Math.max(...dates.map(hit => hit.y + hit.h));
    const full = cells(end.svg, top, bottom);
    let partial = false;
    for (const t of [.15, .35, .5, .8]) {
      const current = cells(art('calendar', model, style, { t })!.svg, top, bottom);
      partial ||= current.size > 0 && current.size < full.size;
      for (const [position, opacity] of current) assert.equal(opacity, full.get(position), `${style.id}/${t}/${position}`);
    }
    assert.ok(partial, `${style.id}: intermediate weeks appear before the complete calendar`);
  }
});

test('泳道按完整会话成形，不用扫描边伪造会话的结束时刻', () => {
  const segments = (svg: string) => [...svg.replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<rect\b[^>]*stroke-width="1\.4"[^>]*>/g)].map((m) => m[0]);
  for (const style of STYLES) {
    const full = art('lanes', model, style)!;
    for (const t of [0.15, 0.5, 0.8]) {
      const current = art('lanes', model, style, { t })!;
      assert.deepEqual(current.hits, full.hits);
      const currentSegments = segments(current.svg), finalSegments = segments(full.svg);
      if (style.id === 'cyber') {
        // 寻址前的会话尚未绘制；每个已接通区间必须逐属性等于真实最终区间。
        assert.ok(currentSegments.length > 0 && currentSegments.every(segment => finalSegments.includes(segment)), `${style.id}/${t}: whole true intervals`);
        if (t === .15) assert.ok(currentSegments.length < finalSegments.length, 'later sessions remain pending before their real start');
      } else assert.deepEqual(currentSegments, finalSegments, `${style.id}/${t}: true start/end`);
      assert.doesNotMatch(current.svg, /<clipPath[^>]*material-/, `${style.id}/${t}: whole sessions`);
    }
  }
});

test('柔粉色按完整河带与数据条配对显现，不以圆泡裁碎真实轮廓', () => {
  const style = STYLES.find((s) => s.id === 'pastel')!;
  const geometry = (svg: string) => [...normalized(svg).replace(/<defs>[\s\S]*?<\/defs>/g, '').matchAll(/<(?:path|rect)\b[^>]*>/g)].map((m) => m[0]);
  for (const id of ['stream', 'areas', 'versions']) {
    const full = geometry(art(id, model, style)!.svg);
    for (const t of [0.15, 0.5, 0.8]) {
      const svg = art(id, model, style, { t })!.svg;
      assert.deepEqual(geometry(svg), full, `${id}/${t}: complete true outlines`);
      // 材质可在完整底面上着色；底面的逐像素覆盖由 material-motion 测试验证。
      // 几何裁窗与位移仍会改写真实外廓，不能用于这几种数据图。
      assert.doesNotMatch(svg, /<g[^>]+(?:clip-path|transform)=/, `${id}/${t}: no bubble crop or displaced data`);
    }
  }
});

test('浮世绘模块图按真实归属色版套印，已落版份额跨模块保持完整真值', () => {
  const style = STYLES.find((s) => s.id === 'ukiyoe')!;
  const shares = (t: number) => art('areas', model, style, { t })!.hits.filter(h => h.d?.[0]?.includes('里'));
  const full = shares(1);
  const clips = (t: number) => [...normalized(art('areas', model, style, { t })!.svg).matchAll(/<g[^>]+clip-path="([^"]+)"/g)].map(m => m[1]);
  assert.ok(full.length > 4);
  for (const t of [0.15, 0.35, 0.6, 0.8]) {
    const current = shares(t);
    assert.ok(current.length > 0);
    for (const hit of current) assert.ok(full.some(final => JSON.stringify(final) === JSON.stringify(hit)), `${t}: full share, no clipped width`);
    for (const key of new Set(current.map(h => h.k))) {
      assert.equal(current.filter(h => h.k === key).length, full.filter(h => h.k === key).length, `${t}/${key}: same contribution plate across modules`);
    }
    assert.deepEqual(clips(t), clips(1), `${t}: only existing static ornament clips, no row scan window`);
  }
});

test('26风格模块份额不会沿数值轴裁短或只留下中间归属', () => {
  const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id))];
  const shares = (style: typeof styles[number], t: number) => art('areas', model, style, { t })!.hits.filter(h => h.d?.[0]?.includes('里'));
  const moduleOf = (h: ReturnType<typeof shares>[number]) => h.d![0].split('里')[0];
  for (const style of styles) {
    const full = shares(style, 1);
    for (const t of [0.12, 0.2, 0.35, 0.5, 0.8]) {
      const current = shares(style, t);
      for (const hit of current) {
        const final = full.find(h => h.k === hit.k && h.d![0] === hit.d![0]);
        assert.ok(final, `${style.id}/${t}: real contribution`);
        assert.ok(Math.abs(hit.w - final.w) < 0.025, `${style.id}/${t}/${hit.d![0]}: full true width`);
      }
      // 色版套印按真实贡献方分组；其余模块的所有份额同时以真实宽度进入。
      if (style.id !== 'ukiyoe') for (const module of new Set(current.map(moduleOf))) {
        assert.equal(current.filter(h => moduleOf(h) === module).length, full.filter(h => moduleOf(h) === module).length, `${style.id}/${t}/${module}: all shares together`);
      }
    }
  }
});
