import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { art, buildModel, STYLES } from '../src/index.ts';
import { THEMES } from '../src/classic/themes.ts';
import { bridgeStyle } from '../src/classic/bridge.ts';

const model = buildModel(JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8')));
const styles = [...THEMES.map(bridgeStyle), ...STYLES.filter(s => !['minimal', 'noir', 'glass', 'cinema', 'terminal'].includes(s.id))];
// 引号只修复浏览器对数字词字体名的 CSS 解析；静态结构仍对照原基线，像素等价另有 render.test。
const normalize = (svg: string) => svg.replace(/\bc[0-9a-z]+-/g, 'scene-').replace(/&quot;Press Start 2P&quot;/g, 'Press Start 2P');

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
  const marks = (svg: string, id: string, plotTop: number) => {
    const body = normalize(svg).replace(/<defs>[\s\S]*?<\/defs>/g, '');
    const groups: string[] = [], found: { mark: string; ancestors: string[] }[] = [];
    for (const [tag] of body.matchAll(/<\/?g\b[^>]*>|<(?:circle|rect)\b[^>]*\/>/g)) {
      if (tag.startsWith('</g')) groups.pop();
      else if (tag.startsWith('<g')) groups.push(tag);
      else if (tag.startsWith(id === 'clock' ? '<circle ' : '<rect ') && /\bopacity="/.test(tag)
        && Number(new RegExp(`\\b${id === 'clock' ? 'cy' : 'y'}="([^"]+)"`).exec(tag)?.[1]) >= plotTop) {
        found.push({ mark: tag, ancestors: groups.flatMap(group => group.match(/\b(?:transform|filter|clip-path|mask|opacity)="[^"]*"/g) ?? []) });
      }
    }
    return found;
  };
  for (const st of styles.filter(s => ['terminal', 'crt', 'neon'].includes(s.id))) for (const id of ['clock', 'calendar']) {
    const end = art(id, model, st)!;
    const activityHits = end.hits.filter(hit => id === 'clock' ? / \d{1,2}:00$/.test(hit.t) : /^\d{4}-\d{2}-\d{2}$/.test(hit.t));
    const plotTop = Math.min(...activityHits.map(hit => hit.y));
    assert.ok(Number.isFinite(plotTop), 'real activity positions define the plot; header read heads are not data');
    const full = new Map(marks(end.svg, id, plotTop).map(item => [item.mark, item.ancestors]));
    for (const t of [0.1, 0.3, 0.5, 0.7, 0.9]) {
      const svg = art(id, model, st, { t })!.svg;
      for (const { mark, ancestors } of marks(svg, id, plotTop)) {
        // 未落定时的 1.2px 网点是原来的位置底托，不表示活动量。
        if (id === 'clock' && / r="1.2"/.test(mark)) continue;
        assert.ok(full.has(mark), `${st.id}/${id}/${t}: ${mark}`);
        if (st.id === 'crt' && id === 'calendar') {
          assert.ok(ancestors.every(a => /^transform="translate\([\d. -]+\)"$/.test(a)), 'complete weekday rows may move rigidly; intensity, clipping and scale do not change');
        } else assert.deepEqual(ancestors, full.get(mark), `${st.id}/${id}/${t}: activity value has no added parent transform or opacity`);
      }
    }
  }
});

test('作息与日历搬动完整语义组，组内面积与活动浓度保持原值', () => {
  for (const st of styles.filter(s => ['pastel', 'cutout', 'meadow', 'manga', 'cyber', 'broadsheet', 'gala', 'crt', 'arcade', 'ukiyoe'].includes(s.id))) for (const id of ['clock', 'calendar']) {
    if (id === 'clock' && ['broadsheet', 'gala', 'crt', 'arcade'].includes(st.id)) continue; // 产品使用另一套原生 clock，另有真实格测试。
    const end = art(id, model, st)!;
    const hits = end.hits.filter(hit => id === 'clock' ? / \d{1,2}:00$/.test(hit.t) : /^\d{4}-\d{2}-\d{2}$/.test(hit.t));
    const top = Math.min(...hits.map(h => h.y)), bottom = Math.max(...hits.map(h => h.y + h.h));
    const marks = (svg: string) => {
      const groups: string[] = [], result: { mark: string; groups: string[] }[] = [];
      const body = normalize(svg).replace(/<defs>[\s\S]*?<\/defs>/g, '');
      for (const [tag] of body.matchAll(/<\/?g\b[^>]*>|<(?:circle|rect)\b[^>]*\/>/g)) {
        if (tag.startsWith('</g')) groups.pop();
        else if (tag.startsWith('<g')) groups.push(tag);
        else if (tag.startsWith(id === 'clock' ? '<circle ' : '<rect ') && /\bopacity="/.test(tag)) {
          const y = Number(new RegExp(`\\b${id === 'clock' ? 'cy' : 'y'}="([^"]+)"`).exec(tag)?.[1]);
          if (y >= top && y < bottom) result.push({ mark: tag, groups: [...groups] });
        }
      }
      return result;
    };
    const full = new Set(marks(end.svg).map(v => v.mark));
    for (const t of [.08, .2, .45, .7, .92]) for (const item of marks(art(id, model, st, { t })!.svg)) {
      assert.ok(full.has(item.mark), `${st.id}/${id}@${t}: complete original activity mark`);
      for (const group of item.groups) {
        assert.doesNotMatch(group, /\b(?:opacity|mask|clip-path)="|\bscale\(/, `${st.id}/${id}@${t}: no new intensity or area encoding`);
        if (/\bfilter=/.test(group)) assert.equal(st.id, 'cutout', 'only the existing paper shadow accompanies the group');
      }
    }
  }
});

test('活动语义组归位不把相邻真值圆点或日期合并成更大色块', () => {
  // 曾在真实 500px 播放看到：纸行右端压上一日、寻址周列压入上一周。
  // 检查实际 SVG 的变换后轮廓，而非仅检查原始 circle/rect 属性。
  for (const st of styles.filter(s => ['pastel', 'cutout', 'meadow', 'manga', 'cyber', 'broadsheet', 'gala', 'crt', 'arcade', 'ukiyoe'].includes(s.id))) for (const id of ['clock', 'calendar']) {
    if (id === 'clock' && ['broadsheet', 'gala', 'crt', 'arcade'].includes(st.id)) continue;
    if (id === 'calendar' && st.id === 'cutout') continue; // 已有单纸枚旋转，非本轮整周路径。
    const end = art(id, model, st)!;
    const hits = end.hits.filter(hit => id === 'clock' ? / \d{1,2}:00$/.test(hit.t) : /^\d{4}-\d{2}-\d{2}$/.test(hit.t));
    const top = Math.min(...hits.map(h => h.y)), bottom = Math.max(...hits.map(h => h.y + h.h));
    for (const t of [.05, .1, .15, .2, .3, .4, .5, .6, .7, .8, .9]) {
      const body = art(id, model, st, { t })!.svg.replace(/<defs>[\s\S]*?<\/defs>/g, '');
      const groups: string[] = [], marks: { x: number; y: number; w: number; h: number; radius: number }[] = [];
      const number = (tag: string, key: string) => Number(new RegExp(`\\b${key}="([^"]+)"`).exec(tag)?.[1]);
      for (const [tag] of body.matchAll(/<\/?g\b[^>]*>|<(?:circle|rect)\b[^>]*\/>/g)) {
        if (tag.startsWith('</g')) { groups.pop(); continue; }
        if (tag.startsWith('<g')) { groups.push(/\btransform="([^"]+)"/.exec(tag)?.[1] ?? ''); continue; }
        if (!tag.startsWith(id === 'clock' ? '<circle ' : '<rect ') || !/\bopacity="/.test(tag)) continue;
        let x = number(tag, id === 'clock' ? 'cx' : 'x'), y = number(tag, id === 'clock' ? 'cy' : 'y');
        if (y < top || y >= bottom) continue;
        const radius = id === 'clock' ? number(tag, 'r') : 0;
        if (id === 'clock' && radius < 1.5) continue;
        for (const group of [...groups].reverse()) for (const transform of [...group.matchAll(/(translate|rotate)\(([^)]+)\)/g)].reverse()) {
          const [a, b = 0, c = 0] = transform[2].split(/[ ,]+/).map(Number);
          if (transform[1] === 'translate') { x += a; y += b; }
          else { const angle = a * Math.PI / 180, dx = x - b, dy = y - c; x = b + dx * Math.cos(angle) - dy * Math.sin(angle); y = c + dx * Math.sin(angle) + dy * Math.cos(angle); }
        }
        marks.push({ x, y, w: number(tag, 'width'), h: number(tag, 'height'), radius });
      }
      for (let i = 0; i < marks.length; i++) for (let j = i + 1; j < marks.length; j++) {
        const a = marks[i], b = marks[j];
        const overlap = id === 'clock' ? a.radius + b.radius - Math.hypot(a.x - b.x, a.y - b.y)
          : Math.min(Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x), Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));
        assert.ok(overlap <= .01, `${st.id}/${id}@${t}: two real marks overlap by ${overlap}`);
      }
    }
  }
});

test('浮世绘作息以真实星期整版套位，小时命中与原始行内容共同移动', () => {
  const st = styles.find(s => s.id === 'ukiyoe')!;
  const hits = (t: number) => art('clock', model, st, { t })!.hits.filter(h => / \d{1,2}:00$/.test(h.t));
  const final = hits(1), byDay = new Map<string, typeof final>();
  for (const hit of final) {
    const day = hit.t.split(' ')[0];
    byDay.set(day, [...(byDay.get(day) ?? []), hit]);
  }
  for (const t of [.05, .2, .4, .6, .8, .95]) {
    const visible = hits(t);
    for (const [day, end] of byDay) {
      const row = visible.filter(h => h.t.split(' ')[0] === day);
      if (!row.length) continue;
      assert.deepEqual(row.map(h => h.t), end.map(h => h.t), 'an entering ink plate contains the entire real weekday');
      const dx = row[0].x - end[0].x, dy = row[0].y - end[0].y;
      assert.ok(dx >= -14.01 && dx <= .01 && dy >= -2.01 && dy <= .01);
      for (let i = 0; i < row.length; i++) {
        assert.ok(Math.abs(row[i].x - end[i].x - dx) < .01 && Math.abs(row[i].y - end[i].y - dy) < .01);
        assert.equal(row[i].w, end[i].w); assert.equal(row[i].h, end[i].h);
      }
    }
  }
});
