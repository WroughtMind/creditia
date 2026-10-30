// 材质负责“怎样出现”，作品负责“什么先出现”。只作用于调用方明确交出的内容或图形。
import { el, rect, path, r, type Scene, type Attrs, type TextBounds } from './svg.ts';
import type { Style, Box } from './style.ts';
import { drawGroup, phase, type RevealDirection } from './motion.ts';
import { paperSurface } from './paper-motion.ts';

export type MaterialRole = 'content' | 'title' | 'mark' | 'paper' | 'data';
type Bounds = Pick<Box, 'x' | 'y' | 'w' | 'h'>;
export const MATERIAL_MOTIONS: Record<string, string> = {
  'minimal-light': '基线对齐', 'minimal-dark': '暗面亮边', glass: '折光聚焦', film: '投影入片', cinema: '银幕开场',
  editorial: '排版基线', broadsheet: '分栏压印', playbill: '对开幕布', gala: '聚光揭幕', mono: '轮廓显影',
  poster: '整版压印', terminal: '字符输出', crt: '扫描余辉', neon: '灯管通电', arcade: '像素拼装',
  ink: '落墨晕开', ukiyoe: '分色套版', woodblock: '印版压墨', baroque: '中轴展饰', grimoire: '印记描绘',
  manuscript: '逐行书写', pastel: '成对展开', cutout: '纸片拼贴', meadow: '顺势生长', manga: '分镜入格', cyber: '分区扫描',
};
const identity = (st: Pick<Style, 'id'>) => st.id === 'minimal' ? 'minimal-light' : st.id === 'noir' ? 'minimal-dark' : st.id;
const clamp = (x: number) => Math.max(0, Math.min(1, x));
const counts = new WeakMap<Scene, number>();
function unique(s: Scene, name: string) {
  const i = counts.get(s) ?? 0; counts.set(s, i + 1); return `material-${name}-${i}`;
}

/** 字组始终完整可读；终端的扫描线与海报的网点只改变整片着色，不改图形的外廓。 */
function printedSurface(s: Scene, k: number, b: Bounds, kind: 'terminal' | 'poster', valueAxis?: 'x' | 'y'): string {
  const pitch = kind === 'terminal' ? 4 : 6;
  const marks = valueAxis
    ? rect(0, 0, valueAxis === 'x' ? pitch : pitch * (.2 + .8 * k), valueAxis === 'y' ? pitch : pitch * (.2 + .8 * k), { fill: '#fff' })
    : kind === 'terminal'
    ? rect(0, 0, pitch, pitch * (0.2 + 0.8 * k), { fill: '#fff' })
    : el('circle', { cx: pitch / 2, cy: pitch / 2, r: pitch * (0.12 + 0.6 * k), fill: '#fff' });
  const texture = s.url(unique(s, `${kind}-ink`), id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width: pitch, height: pitch }, marks));
  return s.url(unique(s, `${kind}-surface`), id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: b.x - 8, y: b.y - 8, width: b.w + 16, height: b.h + 16 },
    rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: '#fff', opacity: 0.45 + 0.55 * phase(k, 0.65, 1) }),
    rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: texture })));
}

/** 色面自身的成形：保留完整底色与真实轮廓，纹理只改变印面密度。 */
function illustratedSurface(s: Scene, k: number, b: Bounds, kind: 'pastel' | 'cutout' | 'meadow' | 'manga' | 'cyber', role: MaterialRole): string {
  const text = role === 'content' || role === 'title';
  const floor = text ? .72 : kind === 'cyber' ? .64 : kind === 'manga' ? .68 : .52;
  const solid = floor + (1 - floor) * phase(k, .48, 1);
  if (kind === 'cutout') {
    // 只保留一处真实纸面的折光，压平后消失；重复的斜纹会读成无关装饰。
    const fold = .12 + .76 * k, shade = .66 + .34 * phase(k, .35, 1);
    const light = s.url(unique(s, 'paper-fold'), id => el('linearGradient', { id, x1: 0, y1: 0, x2: 1, y2: 1 },
      ...[[0, 1], [fold - .015, 1], [fold + .015, shade], [1, shade]].map(([offset, opacity]) =>
        el('stop', { offset, stopColor: '#fff', stopOpacity: opacity }))));
    return s.url(unique(s, 'paper-pressure'), id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: b.x - 8, y: b.y - 8, width: b.w + 16, height: b.h + 16 },
      rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: light })));
  }
  if (kind === 'meadow') {
    const points = Array.from({ length: 17 }, (_, i) => {
      const x = b.x - 8 + (b.w + 16) * i / 16;
      const y = b.y + b.h * (1 - k) + Math.sin(i / 16 * Math.PI * 1.6 + k) * Math.sin(Math.PI * k) * Math.min(26, b.h * .12) - 10 * phase(k, .8, 1);
      return `${i ? 'L' : 'M'}${r(x)} ${r(y)}`;
    }).join('');
    const soft = s.url(unique(s, 'meadow-wet-edge'), id => el('filter', { id, x: '-20%', y: '-20%', width: '140%', height: '140%' },
      el('feGaussianBlur', { stdDeviation: Math.min(6, b.h * .04) })));
    return s.url(unique(s, 'meadow-wash'), id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: b.x - 8, y: b.y - 8, width: b.w + 16, height: b.h + 16 },
      rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: '#fff', opacity: solid }),
      path(`${points}L${r(b.x + b.w + 8)} ${r(b.y + b.h + 8)}H${r(b.x - 8)}Z`, { fill: '#fff', filter: soft })));
  }
  let width = 12, height = 12, marks = '';
  if (kind === 'pastel') {
    // 固定的细粉触点逐渐相接；不随帧换噪点，也不用胶片的大颗粒曝光。
    width = height = 14;
    marks = [[2, 3, 1.8], [8, 2, 1.2], [5, 9, 1.5], [12, 11, 1.1]].map(([cx, cy, size]) =>
      el('ellipse', { cx, cy, rx: size + k * 2.4, ry: size * .65 + k * 1.8, fill: '#fff', opacity: .72 })).join('');
  } else if (kind === 'manga') {
    width = height = 12;
    const radius = .9 + 7.8 * k;
    marks = [[3, 3], [9, 9], [-3, 9], [9, -3], [15, 3], [3, 15]].map(([cx, cy]) =>
      el('circle', { cx, cy, r: radius, fill: '#fff' })).join('');
  } else {
    width = height = 16;
    // 低亮完整轮廓上，电路格先接通再填实；没有交替的大块黑色缺口。
    marks = path('M0 0H16M0 8H16M0 0V16M8 0V16', { fill: 'none', stroke: '#fff', strokeWidth: .9 });
    marks += [[0, 0], [8, 8], [8, 0], [0, 8]].map(([x, y], i) => {
      const side = 8 * phase(k, i * .08, .52 + i * .08);
      return rect(x, y, side, side, { fill: '#fff' });
    }).join('');
  }
  const print = s.url(unique(s, `${kind}-contact`), id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width, height }, marks));
  return s.url(unique(s, `${kind}-formed`), id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: b.x - 8, y: b.y - 8, width: b.w + 16, height: b.h + 16 },
    rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: '#fff', opacity: solid }),
    rect(b.x - 8, b.y - 8, b.w + 16, b.h + 16, { fill: print })));
}

/** 印墨、金属反光、磷光和像素都在完整原面内形成，底层保持连通。
 * mask 的范围取真实绘制包围盒；小字的浏览器字宽不能被排版估值截断。 */
function printAndScreenSurface(s: Scene, k: number, b: Bounds, kind: 'broadsheet' | 'gala' | 'crt' | 'arcade', role: MaterialRole, valueAxis?: 'x' | 'y'): string {
  const text = role === 'content' || role === 'title', floor = text ? kind === 'gala' ? .55 : .76 : .46;
  const base = floor + (1 - floor) * phase(k, .62, 1);
  let surface = '';
  if (kind === 'gala' || kind === 'crt') {
    const at = -.3 + 1.6 * k, feather = kind === 'gala' ? .3 : .045;
    const stops = [[0, base], [at - feather, base], [at, 1], [at + feather, base], [1, base]]
      .map(([offset, opacity]) => [clamp(offset), opacity]).sort((a, b) => a[0] - b[0]);
    const alongX = valueAxis === 'y' || (!valueAxis && kind === 'gala');
    const fill = s.url(unique(s, `${kind}-contact-light`), id => el('linearGradient', { id, x1: 0, y1: 0, x2: alongX ? 1 : 0, y2: alongX ? 0 : 1 },
      ...stops.map(([offset, opacity]) => el('stop', { offset, stopColor: '#fff', stopOpacity: opacity }))));
    surface = rect(0, 0, 1, 1, { fill });
  } else {
    const px = Math.min(.5, 8 / Math.max(1, b.w)), py = Math.min(.5, 8 / Math.max(1, b.h));
    let marks = '';
    if (kind === 'broadsheet') {
      // 横向细印线先接纸，再压成连续墨层；带底墨，不把窄柱分裂成两根。
      const coverage = .12 + .88 * phase(k, 0, .92);
      marks = rect(0, 0, px, py * coverage, { fill: '#fff' });
    } else {
      const level = Math.floor(k * 8) / 8;
      marks = [[0, 0], [px / 2, py / 2], [px / 2, 0], [0, py / 2]].map(([x, y], i) =>
        rect(x, y, px / 2, py / 2, { fill: '#fff', opacity: level >= i / 4 ? 1 : 0 })).join('');
    }
    const fill = s.url(unique(s, `${kind}-contact-ink`), id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width: px, height: py }, marks));
    surface = rect(0, 0, 1, 1, { fill: '#fff', opacity: base }) + rect(0, 0, 1, 1, { fill });
  }
  return s.url(unique(s, `${kind}-contact`), id => el('mask', { id, maskUnits: 'objectBoundingBox', maskContentUnits: 'objectBoundingBox', x: 0, y: 0, width: 1, height: 1 }, surface));
}

/** 印版、黑白墨面、银幕曝光与节目纸折面的连续表面；量条沿非数值轴形成。 */
function editionSurface(s: Scene, id: string, k: number, b: Bounds, axis?: 'x' | 'y'): string {
  const base = id === 'mono' ? .76 + .24 * phase(k, .64, 1) : .52 + .48 * phase(k, .64, 1);
  const across = axis === 'y' || (!axis && id === 'mono');
  let surface: string;
  if (id === 'mono') {
    const pitch = Math.min(.5, 6 / Math.max(1, across ? b.w : b.h));
    const ink = pitch * (.12 + .88 * k);
    const fill = s.url(unique(s, 'mono-plate'), name => el('pattern', { id: name, patternUnits: 'userSpaceOnUse', width: across ? pitch : 1, height: across ? 1 : pitch },
      rect(0, 0, across ? ink : 1, across ? 1 : ink, { fill: '#fff' })));
    surface = rect(0, 0, 1, 1, { fill: '#fff', opacity: base }) + rect(0, 0, 1, 1, { fill });
  } else {
    let stops: number[][];
    if (id === 'cinema') {
      const half = .5 * k, soft = .14 * (1 - k);
      stops = [[0, base], [.5 - half - soft, base], [.5 - half, 1], [.5 + half, 1], [.5 + half + soft, base], [1, base]];
    } else if (id === 'playbill') {
      // 对折节目纸整面先可读，中间折光随纸面压平；没有分裂成两片的透明缺口。
      const fold = .2 * (1 - k), shade = .6 + .4 * phase(k, .3, 1);
      stops = [[0, 1], [.5 - fold, 1], [.5, shade], [.5 + fold, 1], [1, 1]];
    } else {
      const front = 1.2 * k;
      stops = [[0, 1], [front - .12, 1], [front, base], [1, base]];
    }
    const fill = s.url(unique(s, `${id}-press`), name => el('linearGradient', { id: name, x1: 0, y1: 0, x2: across ? 1 : 0, y2: across ? 0 : 1 },
      ...stops.map(([offset, opacity]) => [clamp(offset), opacity]).sort((a, b) => a[0] - b[0]).map(([offset, opacity]) => el('stop', { offset, stopColor: '#fff', stopOpacity: opacity }))));
    surface = rect(0, 0, 1, 1, { fill });
  }
  return s.url(unique(s, `${id}-impression`), name => el('mask', { id: name, maskUnits: 'objectBoundingBox', maskContentUnits: 'objectBoundingBox', x: 0, y: 0, width: 1, height: 1 }, surface));
}

/** 活动强度是数据：原半径、原面积、原色完整入场；语义组可以刚性归位。 */
export function activityEnter(s: Scene, st: Style, value: number, b: Bounds, draw: () => void, round: boolean, groupSide?: -1 | 1): void {
  const k = clamp(value);
  if (k <= 0) return;
  if (k >= 1) { draw(); return; }
  const id = identity(st), rest = 1 - k;
  if (['pastel', 'cutout', 'meadow', 'manga', 'cyber'].includes(id)) {
    if (groupSide !== undefined) {
      // 调用方交给这里真实半日、星期或周列，而不是把单个小圆点无限放大位移。
      // 组内的所有强度标记保留原尺寸与原浓度，只有整组的刚性位置/角度变化。
      const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
      let x = 0, y = 0, angle = 0;
      const attrs: Attrs = {};
      if (id === 'pastel') {
        // 半日向两边展开，整周则上下成对归位，避免横挤相邻日期列。
        x = round ? groupSide * Math.min(42, b.w * .55) * rest * rest : 0;
        y = round ? 0 : groupSide * 24 * rest * rest;
      } else if (id === 'cutout') {
        // 日行从尚未排入的下方落贴，统一倾向避免右端压进已落定的上一日。
        x = groupSide * 12 * rest; y = 24 * rest;
        angle = -1.6 * rest;
        attrs.filter = optical(s, st, k, 'paper', b, 'data');
      } else if (id === 'meadow') {
        // 周列沿单向柔弧排入，横向回摆会挤掉已经落定的相邻日期间隙。
        x = round ? -36 * rest : 18 * rest;
        y = -24 * rest * rest;
      } else if (id === 'manga') {
        // 整块时间分镜先沿斜线入版，再沿横向卡到原栅格；没有倾斜或切碎日期格。
        x = -28 * rest; y = -24 * phase(rest, .5, 1);
      } else if (id === 'cyber') {
        // 一整组寻址两次：先对齐列，再锁定行；不通过改亮度表示虚假的活动量。
        x = k < .42 ? round ? -20 : 20 : 0; y = k < .72 ? -16 : 0;
      }
      attrs.transform = `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}`;
      const firstHit = s.hits.length;
      drawGroup(s, attrs, draw);
      const a = r(angle) * Math.PI / 180, cos = Math.cos(a), sin = Math.sin(a);
      for (const hit of s.hits.slice(firstHit)) {
        const corners = [[hit.x, hit.y], [hit.x + hit.w, hit.y], [hit.x, hit.y + hit.h], [hit.x + hit.w, hit.y + hit.h]]
          .map(([px, py]) => ({ x: cx + (px - cx) * cos - (py - cy) * sin + r(x), y: cy + (px - cx) * sin + (py - cy) * cos + r(y) }));
        hit.x = Math.min(...corners.map(p => p.x)); hit.y = Math.min(...corners.map(p => p.y));
        hit.w = Math.max(...corners.map(p => p.x)) - hit.x; hit.h = Math.max(...corners.map(p => p.y)) - hit.y;
      }
      return;
    }
    // 一整枚活动单位在自己的格内归位；半径、面积、着色强度都直接沿用真值。
    const cx = b.x + b.w / 2, cy = b.y + b.h / 2, side = cx < s.w / 2 ? -1 : 1;
    let x = 0, y = 0, angle = 0;
    const attrs: Attrs = {};
    if (id === 'pastel') x = side * b.w * .16 * rest;
    if (id === 'cutout') {
      x = side * b.w * .12 * rest; y = -b.h * .16 * rest;
      angle = round ? 0 : side * 9 * rest; attrs.filter = optical(s, st, k, 'paper', b, 'data');
    }
    if (id === 'meadow') { x = b.w * .16 * Math.sin(Math.PI * k); y = b.h * .18 * rest * rest; }
    if (id === 'manga') { x = -b.w * .18 * rest; y = -b.h * .12 * rest; angle = round ? 0 : -8 * rest; }
    if (id === 'cyber') {
      const unit = Math.min(2, Math.min(b.w, b.h) * .12);
      x = k < .45 ? unit : 0; y = k < .75 ? -unit : 0;
    }
    attrs.transform = `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}`;
    drawGroup(s, attrs, draw); return;
  }
  if (['editorial', 'mono', 'cinema', 'playbill'].includes(id)) {
    if (groupSide !== undefined) {
      // 日历明确交出整星期行或整周列。强度不参与落墨，所有原色日期格刚性同行。
      const x = id === 'editorial' ? 20 * rest : 0;
      const y = id === 'mono' ? -18 * rest * rest
        : id === 'cinema' ? 24 * rest * rest
        : id === 'playbill' ? groupSide * 18 * rest : 0;
      const firstHit = s.hits.length;
      drawGroup(s, { transform: `translate(${r(x)} ${r(y)})` }, draw);
      for (const hit of s.hits.slice(firstHit)) { hit.x += r(x); hit.y += r(y); }
      return;
    }
    const cx = b.x + b.w / 2, cy = b.y + b.h / 2, side = cx < s.w / 2 ? -1 : 1;
    const x = id === 'editorial' ? -Math.min(3, b.w * .18) * rest : id === 'playbill' ? side * Math.min(3, b.w * .16) * rest : 0;
    const y = id === 'mono' ? -Math.min(3, b.h * .18) * rest : id === 'cinema' ? Math.min(5, b.h * .3) * rest : 0;
    const angle = id === 'playbill' && !round ? side * 6 * rest : 0;
    drawGroup(s, { transform: `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}` }, draw); return;
  }
  if (['ink', 'ukiyoe', 'woodblock', 'baroque', 'grimoire', 'manuscript'].includes(id)) {
    // 热力的浓度和半径不能参与着墨插值。完整标记只在格内轻落，纸面接触痕在其外侧。
    // 这里诚实共享整枚印入；各材质的完整标题/字组另承担明显的湿墨、套印、压墨和校注。
    const cx = b.x + b.w / 2, cy = b.y + b.h / 2, side = cx < s.w / 2 ? -1 : 1;
    let x = 0, y = 0, angle = 0;
    if (id === 'ukiyoe') x = -Math.min(1.8, b.w * .12) * rest;
    if (id === 'woodblock') y = -Math.min(1.8, b.h * .12) * rest;
    if (id === 'ink') y = -Math.min(1, b.h * .08) * rest * rest;
    if (id === 'grimoire') { y = -Math.min(1.2, b.h * .08) * rest; angle = round ? 0 : 4 * rest; }
    if (id === 'baroque') { x = side * Math.min(1.5, b.w * .1) * rest; angle = round ? 0 : -side * 5 * rest; }
    if (id === 'manuscript') s.add(path(`M${r(b.x)} ${r(b.y + b.h + 1.3)}h${r(b.w * k)}`, {
      fill: 'none', stroke: st.c.accent, strokeWidth: .65, strokeOpacity: .38 * Math.sin(Math.PI * k),
    }));
    if (id === 'ink' || id === 'woodblock' || id === 'grimoire') s.add(el('ellipse', {
      cx, cy: b.y + b.h + 1.3, rx: Math.max(.5, Math.min(5, b.w * .32)), ry: .55 * rest,
      fill: st.c.soft, opacity: (id === 'woodblock' ? .26 : .15) * Math.sin(Math.PI * k),
    }));
    drawGroup(s, { transform: `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}` }, draw); return;
  }
  if (['broadsheet', 'gala', 'crt', 'arcade'].includes(id)) {
    if (groupSide !== undefined) {
      // 日期按真实整周列或星期行成组，保留每格原色浓度及面积。
      const x = id === 'crt' ? 28 * rest * rest : 0;
      const y = id === 'broadsheet' ? -20 * rest
        : id === 'gala' ? groupSide * 22 * rest * rest
        : id === 'arcade' ? -Math.ceil(rest * 3) * 8 : 0;
      const first = s.hits.length;
      drawGroup(s, { transform: `translate(${r(x)} ${r(y)})` }, draw);
      for (const hit of s.hits.slice(first)) { hit.x += x; hit.y += y; }
      return;
    }
    // 原色、原面积的整枚单位在自己的间隙内落位；不把强度当作曝光渐变。
    const side = b.x + b.w / 2 < s.w / 2 ? -1 : 1;
    let x = 0, y = 0;
    if (id === 'broadsheet') y = -Math.min(2, b.h * .14) * rest * rest;
    if (id === 'gala') y = -Math.min(3, b.h * .18) * rest;
    if (id === 'arcade') x = side * Math.min(2, b.w * .12) * Math.ceil(rest * 2) / 2;
    if (id === 'crt') y = -Math.min(1, b.h * .1) * Math.floor(rest * 3) / 3;
    drawGroup(s, { transform: `translate(${r(x)} ${r(y)})` }, draw); return;
  }
  draw();
}

/** 单元的铺版顺序；index 是行优先，返回 0..1。只改变出现先后，不移动数据。 */
export function materialOrder(st: Pick<Style, 'id'>, index: number, total: number, columns = 1): number {
  if (total <= 1) return 0;
  const cols = Math.max(1, Math.min(total, columns)), rows = Math.ceil(total / cols);
  const row = Math.floor(index / cols), col = index % cols, id = identity(st);
  const linear = index / (total - 1), column = (col * rows + row) / Math.max(1, cols * rows - 1);
  const fromCenter = (at: number, n: number) => {
    const nearest = n % 2 ? 0 : 0.5;
    return (Math.abs(at - (n - 1) / 2) - nearest) / Math.max(1, (n - 1) / 2 - nearest);
  };
  const center = fromCenter(col, cols);
  switch (id) {
    case 'minimal-dark': return column;
    case 'glass': return clamp((center + fromCenter(row, rows)) / 2);
    case 'cinema': return clamp(center * 0.75 + row / Math.max(1, rows - 1) * 0.25);
    case 'crt': case 'manuscript': return row / Math.max(1, rows - 1);
    case 'ink': case 'manga': return clamp(((cols - col - 1) * rows + row) / Math.max(1, cols * rows - 1));
    case 'ukiyoe': return clamp((row + col * 0.45) / Math.max(1, rows - 1 + (cols - 1) * 0.45));
    case 'woodblock': return clamp(((row + col) % 2) * 0.52 + linear * 0.48);
    case 'arcade': return clamp((col % 2) * 0.5 + column * 0.5);
    case 'cutout': case 'gala': return clamp(center * 0.6 + row / Math.max(1, rows - 1) * 0.4);
    case 'pastel': case 'baroque': case 'playbill': return clamp(row / Math.max(1, rows - 1) * 0.65 + center * 0.35);
    case 'meadow': return clamp((rows - row - 1 + col * 0.6) / Math.max(1, rows - 1 + (cols - 1) * 0.6));
    case 'cyber': return clamp((row + col) / Math.max(1, rows + cols - 2));
    case 'broadsheet': case 'editorial': case 'film': return column;
    case 'neon': case 'grimoire': return clamp(fromCenter(index, total));
    case 'poster': return clamp(Math.floor(linear * 4) / 4);
    default: return linear;
  }
}

/** 滤镜只覆盖当前内容及光晕，长字幕不会为每一行分配整张画布。 */
function optical(s: Scene, st: Style, k: number, kind: 'ink' | 'focus' | 'projection' | 'outline' | 'neon' | 'crt' | 'press' | 'paper' | 'register', b: Bounds, role: MaterialRole = 'content'): string {
  const rest = 1 - k, solid = role === 'mark';
  let primitives = '';
  if (kind === 'ink') primitives = (solid ? el('feMorphology', { operator: 'dilate', radius: r(0.28 * rest) }) : '') + el('feGaussianBlur', { stdDeviation: r(0.12 * rest) });
  if (kind === 'focus') {
    primitives = el('feGaussianBlur', { stdDeviation: r(2.4 * rest * rest), result: role === 'data' ? 'focused' : undefined });
    if (role === 'data') {
      // 光在真实玻璃面上反射；完整归属色一直保留，不用亮窗裁短份额。
      primitives += el('feGaussianBlur', { in: 'SourceAlpha', stdDeviation: .8, result: 'relief' })
        + el('feSpecularLighting', { in: 'relief', surfaceScale: 1, specularConstant: r(.65 * Math.sin(Math.PI * k)), specularExponent: 16, lightingColor: '#fff', result: 'light' },
          el('fePointLight', { x: r(b.x + b.w * k), y: r(b.y + b.h * .3), z: r(Math.max(b.w, b.h) * .45) }))
        + el('feComposite', { in: 'light', in2: 'SourceAlpha', operator: 'in', result: 'refraction' })
        + el('feComposite', { in: 'focused', in2: 'refraction', operator: 'arithmetic', k2: 1, k3: .6 });
    }
  }
  // 投影沿片门方向脱焦，再整组收束；不拿横线扫过名字，也不拆成重叠的第二份文字。
  if (kind === 'projection') primitives = el('feGaussianBlur', { stdDeviation: `${r(.25 * rest)} ${r(2.8 * rest * rest)}` });
  if (kind === 'crt') primitives = el('feGaussianBlur', { stdDeviation: r(1.6 * rest), result: 'glow' }) + el('feMerge', {}, el('feMergeNode', { in: 'glow' }), el('feMergeNode', { in: 'SourceGraphic' }));
  if (kind === 'press') primitives = solid ? el('feMorphology', { operator: 'dilate', radius: r(0.25 * Math.sin(Math.PI * k)) }) : el('feGaussianBlur', { stdDeviation: r(0.12 * (1 - k)) });
  if (kind === 'paper') primitives = el('feDropShadow', { dx: r(3 * rest), dy: r(5 * rest), stdDeviation: r(2 * rest), floodColor: st.c.ink, floodOpacity: r(0.3 * rest) });
  if (kind === 'register') primitives = el('feOffset', { in: 'SourceAlpha', dx: r(-3 * rest), dy: r(rest), result: 'plate' })
    + el('feFlood', { floodColor: st.c.accent, floodOpacity: r(rest * 0.5), result: 'ink' })
    + el('feComposite', { in: 'ink', in2: 'plate', operator: 'in', result: 'underprint' })
    + el('feMerge', {}, el('feMergeNode', { in: 'underprint' }), el('feMergeNode', { in: 'SourceGraphic' }));
  if (kind === 'outline' || kind === 'neon') {
    primitives = el('feMorphology', { in: 'SourceAlpha', operator: 'erode', radius: kind === 'neon' ? role === 'data' ? Math.max(0.8, Math.min(6, Math.min(b.w, b.h) * 0.08)) : 0.8 : 0.55, result: 'core' })
      + el('feComposite', { in: 'SourceAlpha', in2: 'core', operator: 'out', result: 'edge' })
      // 数据轮廓沿原来的归属色通电；不能把多色份额暂时染成同一个强调色。
      + (role === 'data'
        ? el('feComposite', { in: 'SourceGraphic', in2: 'edge', operator: 'in', result: 'outline' })
        : el('feFlood', { floodColor: kind === 'neon' ? st.c.accent : st.c.ink, result: 'color' })
          + el('feComposite', { in: 'color', in2: 'edge', operator: 'in', result: 'outline' }))
      + el('feComponentTransfer', { in: 'SourceGraphic', result: 'fill' }, el('feFuncA', { type: 'linear', slope: r(phase(k, 0.28, 0.9)) }));
    if (kind === 'neon') primitives += el('feGaussianBlur', { in: 'outline', stdDeviation: 2.2, result: 'halo' });
    primitives += el('feMerge', {}, kind === 'neon' && el('feMergeNode', { in: 'halo' }), el('feMergeNode', { in: 'outline' }), el('feMergeNode', { in: 'fill' }));
  }
  const pad = 8, x = Math.max(0, b.x - pad), y = Math.max(0, b.y - pad);
  const width = Math.max(1, Math.min(s.w, b.x + b.w + pad) - x), height = Math.max(1, Math.min(s.h, b.y + b.h + pad) - y);
  return s.url(unique(s, kind), id => el('filter', { id, filterUnits: 'userSpaceOnUse', x, y, width, height }, primitives));
}

/** 完整内容进场，绝不拿裁切边扫过文字。k=0 也绘制回调，保留排版度量。 */
export function materialEnter(s: Scene, st: Style, value: number, b: Bounds, draw: () => void, role: MaterialRole = 'content', valueAxis?: 'x' | 'y'): void {
  const k = clamp(value);
  if (k >= 1) { draw(); return; }
  if (k <= 0) { const first = s.hits.length; drawGroup(s, { visibility: 'hidden' }, draw); s.hits.splice(first); return; }
  const id = identity(st), rest = 1 - k, data = role === 'data';
  // 数据面使用完整低密度乳剂及细颗粒；小格、树图和双环不能只剩透明度曲线。
  if (id === 'film' && data) { materialReveal(s, st, k, b, 'right', draw); return; }
  const dx = Math.min(14, b.w * 0.05), dy = Math.min(12, b.h * 0.12);
  const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
  const attrs: Attrs = { opacity: k, visibility: k <= 0 ? 'hidden' : undefined };
  let x = 0, y = 0, angle = 0;
  const lean = Math.min(2.4, Math.atan2(Math.min(6, b.h * 0.1), Math.max(1, b.w / 2)) * 180 / Math.PI);
  switch (id) {
    case 'minimal-light': y = 4 * rest; break;
    case 'minimal-dark':
      attrs.opacity = phase(k, 0.08, 0.72);
      if (role === 'mark') attrs.filter = optical(s, st, k, 'outline', b, role);
      break;
    case 'glass':
      y = -dy * rest; attrs.filter = optical(s, st, k, 'focus', b, role);
      if (data) attrs.opacity = .55 + .45 * phase(k, 0, .65);
      break;
    case 'film':
      y = role === 'paper' ? -Math.min(14, b.h * .04) * rest : 5 * rest;
      attrs.opacity = phase(k, 0, .3); attrs.filter = optical(s, st, k, 'projection', b, role);
      break;
    case 'cinema':
      y = dy * rest; attrs.opacity = phase(k, 0, .2);
      if (data) attrs.mask = editionSurface(s, id, k, b, valueAxis);
      else attrs.filter = optical(s, st, k, 'projection', b, role);
      break;
    case 'editorial':
      x = -dx * rest; attrs.opacity = phase(k, 0, .15);
      if (data) attrs.mask = editionSurface(s, id, k, b, valueAxis);
      break;
    case 'broadsheet':
      attrs.opacity = phase(k, 0, .16); attrs.mask = printAndScreenSurface(s, k, b, id, role, valueAxis);
      attrs.filter = optical(s, st, k, 'press', b, role); break;
    case 'playbill':
      y = Math.min(10, b.h * .16) * rest; attrs.opacity = phase(k, 0, .16);
      if (data) attrs.mask = editionSurface(s, id, k, b, valueAxis);
      break;
    case 'gala':
      y = data ? 0 : -dy * rest; attrs.opacity = phase(k, 0, .22);
      attrs.mask = printAndScreenSurface(s, k, b, id, role, valueAxis); break;
    case 'mono':
      attrs.opacity = phase(k, 0, .16);
      if (data) attrs.mask = editionSurface(s, id, k, b, valueAxis);
      else if (role === 'content' || role === 'title') attrs.filter = s.url(unique(s, 'mono-letterpress'), name =>
        el('filter', { id: name, x: '-5%', y: '-20%', width: '110%', height: '140%' }, el('feMorphology', { operator: 'dilate', radius: r(.85 * rest) })));
      else attrs.filter = optical(s, st, k, 'outline', b, role);
      break;
    case 'poster':
      attrs.opacity = 1;
      if (data || role === 'mark') attrs.mask = printedSurface(s, k, b, 'poster', valueAxis);
      else attrs.filter = optical(s, st, k, 'register', b, role);
      break;
    case 'terminal': attrs.opacity = 1; attrs.mask = printedSurface(s, k, b, 'terminal', valueAxis); break;
    case 'crt':
      attrs.opacity = phase(k, 0, .18); attrs.filter = optical(s, st, k, 'crt', b);
      if (data || role === 'mark') attrs.mask = printAndScreenSurface(s, k, b, id, role, valueAxis); break;
    case 'neon': attrs.opacity = phase(k, 0, 0.22); attrs.filter = optical(s, st, k, 'neon', b, role); break;
    case 'arcade':
      x = data ? 0 : Math.round(dx * rest / 4) * -4; attrs.opacity = k > .04 ? 1 : 0;
      attrs.mask = printAndScreenSurface(s, k, b, id, role, valueAxis); break;
    case 'ink': Object.assign(attrs, paperSurface(s, id, k, b, !data && role !== 'mark', valueAxis)); break;
    case 'ukiyoe':
      if (data) Object.assign(attrs, paperSurface(s, id, k, b, false, valueAxis));
      else { attrs.opacity = phase(k, 0, .28); attrs.filter = optical(s, st, k, 'register', b); }
      break;
    case 'woodblock': Object.assign(attrs, paperSurface(s, id, k, b, !data && role !== 'mark', valueAxis)); y = -2 * rest; break;
    case 'baroque':
      Object.assign(attrs, paperSurface(s, id, k, b, !data && role !== 'mark', valueAxis));
      y = -dy * rest; angle = (cx < s.w / 2 ? -1 : 1) * lean * rest; break;
    case 'grimoire':
      // 小型真实蜡章仍从章边压入，字面和数据印面用细刻纹；不把所有内容都退回轮廓模板。
      if (role === 'mark') { attrs.opacity = phase(k, .06, .5); attrs.filter = optical(s, st, k, 'outline', b, role); }
      else Object.assign(attrs, paperSurface(s, id, k, b, !data, valueAxis));
      break;
    case 'manuscript':
      Object.assign(attrs, paperSurface(s, id, k, b, !data && role !== 'mark', valueAxis)); x = -3 * rest; break;
    case 'pastel':
      x = (cx < s.w / 2 ? -1 : 1) * dx * rest; y = -2 * Math.sin(k * Math.PI);
      attrs.opacity = phase(k, 0, .16); attrs.mask = illustratedSurface(s, k, b, id, role); break;
    case 'cutout':
      x = (cx < s.w / 2 ? -1 : 1) * dx * rest; y = -dy * rest; angle = lean * rest; attrs.opacity = 1;
      attrs.filter = optical(s, st, k, 'paper', b);
      if (data) attrs.mask = illustratedSurface(s, k, b, id, role); break;
    case 'meadow':
      x = dx * Math.sin(k * Math.PI) * rest * 0.35; y = dy * rest;
      attrs.opacity = phase(k, 0, .18); attrs.mask = illustratedSurface(s, k, b, id, role); break;
    case 'manga':
      x = -dx * rest; y = dy * rest * 0.4; attrs.opacity = phase(k, 0, .1);
      attrs.mask = illustratedSurface(s, k, b, id, role); break;
    case 'cyber':
      x = Math.round(dx * rest / 3) * 3; attrs.opacity = phase(k, 0, .1);
      if (data || role === 'mark') attrs.mask = illustratedSurface(s, k, b, id, role);
      else attrs.filter = optical(s, st, k, 'outline', b, 'data'); break;
  }
  if (data) { x = y = angle = 0; }
  if (role === 'paper' && id !== 'cutout') { delete attrs.filter; delete attrs.mask; } // 纸上的行另有落墨；整张纸仅保留自己的投影。
  if (x || y || angle) attrs.transform = `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}`;
  const first = s.hits.length;
  let written: TextBounds[] = [];
  drawGroup(s, attrs, () => {
    if ((id === 'manuscript' || id === 'terminal' || id === 'editorial') && !data && role !== 'paper' && role !== 'mark') written = s.captureTextBounds(draw);
    else draw();
  });
  for (let i = s.hits.length - 1; i >= first; i--) {
    if (k <= 0 || attrs.opacity === 0) { s.hits.splice(i, 1); continue; }
    // 内容旋转只用于轻微落纸；命中区取旋转后的外接框。
    const hit = s.hits[i], a = angle * Math.PI / 180, co = Math.cos(a), si = Math.sin(a);
    const pts = [[hit.x, hit.y], [hit.x + hit.w, hit.y], [hit.x, hit.y + hit.h], [hit.x + hit.w, hit.y + hit.h]]
      .map(([px, py]) => [cx + (px - cx) * co - (py - cy) * si + x, cy + (px - cx) * si + (py - cy) * co + y]);
    hit.x = r(Math.min(...pts.map(p => p[0]))); hit.y = r(Math.min(...pts.map(p => p[1])));
    hit.w = r(Math.max(...pts.map(p => p[0])) - hit.x); hit.h = r(Math.max(...pts.map(p => p[1])) - hit.y);
  }
  if (k <= 0 || data || role === 'paper') return;
  const fade = Math.sin(Math.PI * k) * 0.6, lineWidth = Math.min(1.3, b.h / 12);
  const stroke = (x1: number, y1: number, x2: number, y2: number) => el('line', { x1, y1, x2, y2, stroke: st.c.accent, strokeWidth: lineWidth, opacity: fade });
  if (role !== 'mark' && id === 'editorial') for (const row of written) {
    if (row.rotate || row.w <= row.h) continue;
    const yy = row.baseline + Math.max(2, row.size * .12) + y;
    s.add(stroke(row.x + x, yy, row.x + x + row.w * k, yy));
  }
  if (id === 'manuscript') for (const row of written) {
    // 注线只属于实际横行字面；居中标题、右齐数值、短署名都不借用整容器宽度。
    // 旋转竖排及圆弧题字没有这条水平基线，条码也不会产生无关批注。
    if (row.rotate || row.w <= row.h) continue;
    const baseline = row.baseline + Math.max(2, row.size * .12) + y;
    s.add(stroke(row.x + x, baseline, row.x + x + row.w * k, baseline));
  }
  if (id === 'terminal' && k < .9) {
    // 一次输出完整字组，读头停在实际字行末端；多行输出逐行提交，不能用容器右边冒充字尾。
    const rows = written.filter(row => !row.rotate && row.w > 0).sort((a, b) => a.baseline - b.baseline || a.x - b.x);
    const row = rows[Math.min(rows.length - 1, Math.floor(k / .9 * rows.length))];
    if (row) {
      const width = Math.max(3, row.size * .5), height = Math.max(1.5, row.size * .12);
      const end = row.x + row.w, below = end + width + 4 > s.w;
      s.add(rect(below ? end - width : end + 3, below ? row.baseline + 3 : row.baseline - height,
        width, height, { fill: st.c.accent, opacity: Math.floor(k * 12) % 3 === 2 ? .35 : 1 }));
    }
  }
  if (id === 'broadsheet') s.add(stroke(b.x, b.y, b.x, b.y + b.h * k));
  if (id === 'minimal-dark' && role !== 'mark') s.add(stroke(b.x, b.y + b.h, b.x + b.w * phase(k, 0, 0.6), b.y + b.h));
  // 字组已经完整点亮，回扫线走组边缘；穿过笔画会把小签名读成删除线。
  if (id === 'crt') {
    const scanY = role === 'mark' ? b.y + b.h * k : b.y + b.h;
    s.add(stroke(b.x, scanY, b.x + b.w * (role === 'mark' ? 1 : k), scanY));
  }
}

/** 图形的材质展开。文字应调用 materialEnter；这里允许纸、墨、光、像素各自形成边界。 */
export function materialReveal(s: Scene, st: Style, value: number, b: Bounds, direction: RevealDirection, draw: () => void): void {
  const k = clamp(value);
  if (k >= 1) { draw(); return; }
  if (k <= 0) { const first = s.hits.length; drawGroup(s, { visibility: 'hidden' }, draw); s.hits.splice(first); return; }
  const id = identity(st);
  if (['mono', 'neon', 'poster', 'manuscript', 'cutout', 'cyber', 'woodblock', 'grimoire'].includes(id)) {
    materialEnter(s, st, k, b, draw, 'data'); return;
  }
  const vertical = direction === 'down' || direction === 'up';
  const primary = vertical ? b.h : b.w, secondary = vertical ? b.w : b.h;
  const pos = (u: number, v: number): [number, number] => vertical
    ? [b.x + v, direction === 'up' ? b.y + b.h - u : b.y + u]
    : [direction === 'left' ? b.x + b.w - u : b.x + u, b.y + v];
  const windows: Bounds[] = [];
  const windowRect = (x: number, y: number, w: number, h: number) => { windows.push({ x, y, w, h }); return rect(x, y, w, h, { fill: '#fff' }); };
  const polygon = (uv: number[][]) => {
    const points = uv.map(([u, v]) => pos(u, v)), xs = points.map(p => p[0]), ys = points.map(p => p[1]);
    windows.push({ x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) });
    return path(points.map(([x, y], i) => `${i ? 'L' : 'M'}${r(x)} ${r(y)}`).join('') + 'Z', { fill: '#fff' });
  };
  const band = (u: number, v: number, w: number, h: number) => polygon([[u, v], [u + w, v], [u + w, v + h], [u, v + h]]);
  const center = () => band(primary * (1 - k) / 2, 0, primary * k, secondary);
  let shape = '', soft = false, mask = false;
  // 整幅材质成形保持图形外廓，不把印刷、曝光都做成同一种横向裁窗。
  const surface = (width: number, height: number, marks: string) => {
    mask = true; windows.push({ ...b });
    const fill = s.url(unique(s, `${id}-surface`), pid => el('pattern', { id: pid, patternUnits: 'userSpaceOnUse', width, height }, marks));
    // 最后一段墨面填实；避免缩小栅格的纹样缝隙一直留到 t=1 才突然补齐。
    const exposure = id === 'film';
    return rect(b.x, b.y, b.w, b.h, { fill: '#fff', opacity: exposure ? phase(k, 0, 0.92) : phase(k, 0.65, 1) })
      + rect(b.x, b.y, b.w, b.h, { fill, opacity: exposure ? 0.32 * Math.sin(Math.PI * k) : 1 });
  };

  switch (id) {
    case 'terminal': case 'arcade': {
      const unit = st.grid ?? 12, x0 = Math.floor(b.x / unit) * unit, y0 = Math.floor(b.y / unit) * unit;
      const cols = Math.ceil((b.x + b.w - x0) / unit), rows = Math.ceil((b.y + b.h - y0) / unit), cells = cols * rows;
      const parts: string[] = [];
      // 同一行相邻的亮格合成一个窗口，格子边界不变，长条也不用生成数千个 SVG 节点。
      for (let row = 0; row < rows; row++) {
        let start = -1;
        for (let col = 0; col <= cols; col++) {
          const visible = col < cols && k > materialOrder(st, row * cols + col, cells, cols);
          if (visible && start < 0) start = col;
          if (!visible && start >= 0) { parts.push(windowRect(x0 + start * unit, y0 + row * unit, (col - start) * unit, unit)); start = -1; }
        }
      }
      shape = parts.join(''); break;
    }
    case 'ink': case 'meadow': {
      const edge: number[][] = [[0, 0]];
      for (let i = 0; i <= 12; i++) {
        const v = secondary * i / 12;
        const wave = id === 'ink' ? Math.sin(i * 2.3) * 0.7 + Math.sin(i * 0.8) * 0.3 : Math.sin(i / 12 * Math.PI * 2 + k * Math.PI) * 0.8;
        edge.push([Math.max(0, Math.min(primary, primary * k + Math.min(14, primary * 0.07) * wave * Math.sin(k * Math.PI))), v]);
      }
      edge.push([0, secondary]); shape = polygon(edge); soft = id === 'ink'; break;
    }
    case 'film': {
      // 曝光在整幅乳剂上同时成像；固定颗粒位置，避免每帧噪声闪烁。
      const unit = Math.max(3, Math.min(5, Math.min(b.w, b.h) / 10));
      const noise = (i: number) => { const x = Math.sin((i + 1) * 127.1) * 43758.5453; return x - Math.floor(x); };
      const tile = unit * 6;
      const marks = Array.from({ length: 36 }, (_, i) => {
        const cx = unit * (i % 6 + 0.1 + noise(i * 3) * 0.8), cy = unit * (Math.floor(i / 6) + 0.1 + noise(i * 3 + 1) * 0.8);
        const radius = unit * k * (0.5 + noise(i * 3 + 2) * 0.7);
        // 边界颗粒跨 tile 连续，不能在曝光中留下周期性方框接缝。
        return [-tile, 0, tile].flatMap(dx => [-tile, 0, tile].map(dy => [cx + dx, cy + dy]))
          .filter(([x, y]) => x + radius >= 0 && x - radius <= tile && y + radius >= 0 && y - radius <= tile)
          .map(([x, y]) => el('circle', { cx: x, cy: y, r: radius, fill: '#fff' })).join('');
      }).join('');
      shape = surface(tile, tile, marks); break;
    }
    case 'broadsheet': {
      // 网线先接触纸面，再填实整版；不是几条不同延迟的扫窗。
      const pitch = Math.max(4, Math.min(12, b.h / 4));
      shape = surface(pitch, pitch, rect(0, 0, pitch, pitch * k, { fill: '#fff' })); break;
    }
    case 'woodblock': {
      const pitch = Math.max(7, Math.min(18, b.w / 8)), h = 36;
      const edge = pitch * k, cut = pitch * k * (1 - k) * 0.35;
      shape = surface(pitch, h, path(`M0 0H${r(edge)}L${r(edge - cut)} 12L${r(edge)} 25L${r(edge - cut / 2)} ${h}H0Z`, { fill: '#fff' })); break;
    }
    case 'grimoire': {
      const pitch = Math.max(10, Math.min(22, Math.min(b.w, b.h) / 2));
      const marks = el('path', { d: `M${-pitch} ${-pitch}L${pitch * 2} ${pitch * 2}`, stroke: '#fff', strokeWidth: pitch * k, fill: 'none' })
        + el('path', { d: `M${-pitch} ${pitch * 2}L${pitch * 2} ${-pitch}`, stroke: '#fff', strokeWidth: pitch * phase(k, 0.3, 0.9), fill: 'none' });
      shape = surface(pitch, pitch, marks); break;
    }
    case 'ukiyoe': {
      // 真实色层由作品分组交出；每一版是完整接触面，不伪造任意五个分色条。
      shape = band(0, 0, primary * k, secondary); break;
    }
    case 'manga': case 'cyber': {
      const skew = Math.min(primary * 0.18, secondary * (id === 'manga' ? 0.8 : 0.35)) * Math.sin(Math.PI * k);
      const front = primary * k;
      shape = polygon([[0, 0], [Math.max(0, front - skew), 0], [Math.min(primary, front + skew), secondary], [0, secondary]]);
      if (id === 'cyber') { windows.length = 0; shape = Array.from({ length: 4 }, (_, i) => band(0, secondary * i / 4, primary * phase(k, i % 2 * 0.12, 1 - (1 - i % 2) * 0.12), secondary / 4 + 0.1)).join(''); }
      break;
    }
    case 'crt': shape = windowRect(b.x, b.y, b.w, Math.min(b.h, Math.ceil(b.h * k / 4) * 4)); break;
    case 'pastel': {
      // 配对的单元来自作品本身；不拿两个抽象圆泡割裂曲线或真实份额。
      materialEnter(s, st, k, b, draw, 'data'); return;
    }
    case 'playbill': shape = windowRect(b.x, b.y, b.w * k / 2, b.h) + windowRect(b.x + b.w * (1 - k / 2), b.y, b.w * k / 2, b.h); break;
    case 'cinema': shape = vertical ? band(0, 0, primary * k, secondary) : center(); soft = true; break;
    case 'baroque': shape = windowRect(b.x + b.w * (1 - k) / 2, b.y, b.w * k, b.h); break;
    case 'gala': {
      mask = true;
      const radius = 0.72 * Math.min(1, k * 1.4);
      windows.push({ x: b.x + b.w * (0.5 - radius), y: b.y + b.h * (0.5 - radius), w: b.w * radius * 2, h: b.h * radius * 2 });
      const light = s.url(unique(s, 'spotlight'), gid => el('radialGradient', { id: gid, cx: 0.5, cy: 0.5, r: 0.72 },
        el('stop', { offset: clamp(k * 1.4 - 0.35), stopColor: '#fff' }),
        el('stop', { offset: Math.min(1, k * 1.4), stopColor: '#000' })));
      shape = rect(b.x, b.y, b.w, b.h, { fill: light }); break;
    }
    case 'editorial': shape = windowRect(b.x, b.y + b.h * (1 - k), b.w, b.h * k); break;
    case 'glass': soft = true; shape = band(0, 0, primary * k, secondary); break;
    default: shape = direction === 'center-x' ? center() : band(0, 0, primary * k, secondary);
  }
  const name = unique(s, id), clip = s.url(name, cid => mask
    ? el('mask', { id: cid, maskUnits: 'userSpaceOnUse', x: b.x, y: b.y, width: b.w, height: b.h }, shape)
    : el('clipPath', { id: cid, clipPathUnits: 'userSpaceOnUse' }, shape || rect(b.x, b.y, 0, 0)));
  const first = s.hits.length;
  drawGroup(s, { ...(mask ? { mask: clip } : { clipPath: clip }), opacity: k <= 0 ? 0 : undefined, filter: soft ? optical(s, st, k, id === 'ink' ? 'ink' : 'focus', b) : undefined },
    () => ['manga', 'meadow'].includes(id) ? materialEnter(s, st, k, b, draw, 'data') : draw());
  // 命中区跟随已经出现的窗口，斜边与曲线取可见部分的外接框。
  for (let i = s.hits.length - 1; i >= first; i--) {
    const hit = s.hits[i], intersections = windows.map(w => ({
      x: Math.max(hit.x, w.x), y: Math.max(hit.y, w.y),
      right: Math.min(hit.x + hit.w, w.x + w.w), bottom: Math.min(hit.y + hit.h, w.y + w.h),
    })).filter(w => w.right > w.x && w.bottom > w.y);
    if (!intersections.length) { s.hits.splice(i, 1); continue; }
    hit.x = r(Math.min(...intersections.map(w => w.x))); hit.y = r(Math.min(...intersections.map(w => w.y)));
    hit.w = r(Math.max(...intersections.map(w => w.right)) - hit.x); hit.h = r(Math.max(...intersections.map(w => w.bottom)) - hit.y);
  }
}
