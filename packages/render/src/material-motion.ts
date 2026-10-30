// 材质负责“怎样出现”，作品负责“什么先出现”。只作用于调用方明确交出的内容或图形。
import { el, rect, path, r, type Scene, type Attrs } from './svg.ts';
import type { Style, Box } from './style.ts';
import { drawGroup, phase, type RevealDirection } from './motion.ts';

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
function optical(s: Scene, st: Style, k: number, kind: 'ink' | 'focus' | 'outline' | 'neon' | 'crt' | 'press' | 'paper' | 'register', b: Bounds, solid = false): string {
  const rest = 1 - k;
  let primitives = '';
  if (kind === 'ink') primitives = (solid ? el('feMorphology', { operator: 'dilate', radius: r(0.28 * rest) }) : '') + el('feGaussianBlur', { stdDeviation: r(0.12 * rest) });
  if (kind === 'focus') primitives = el('feGaussianBlur', { stdDeviation: r(2.4 * rest * rest) });
  if (kind === 'crt') primitives = el('feGaussianBlur', { stdDeviation: r(1.6 * rest), result: 'glow' }) + el('feMerge', {}, el('feMergeNode', { in: 'glow' }), el('feMergeNode', { in: 'SourceGraphic' }));
  if (kind === 'press') primitives = solid ? el('feMorphology', { operator: 'dilate', radius: r(0.25 * Math.sin(Math.PI * k)) }) : el('feGaussianBlur', { stdDeviation: r(0.12 * (1 - k)) });
  if (kind === 'paper') primitives = el('feDropShadow', { dx: r(3 * rest), dy: r(5 * rest), stdDeviation: r(2 * rest), floodColor: st.c.ink, floodOpacity: r(0.3 * rest) });
  if (kind === 'register') primitives = el('feOffset', { in: 'SourceAlpha', dx: r(-3 * rest), dy: r(rest), result: 'plate' })
    + el('feFlood', { floodColor: st.c.accent, floodOpacity: r(rest * 0.5), result: 'ink' })
    + el('feComposite', { in: 'ink', in2: 'plate', operator: 'in', result: 'underprint' })
    + el('feMerge', {}, el('feMergeNode', { in: 'underprint' }), el('feMergeNode', { in: 'SourceGraphic' }));
  if (kind === 'outline' || kind === 'neon') {
    primitives = el('feMorphology', { in: 'SourceAlpha', operator: 'erode', radius: kind === 'neon' ? 0.8 : 0.55, result: 'core' })
      + el('feComposite', { in: 'SourceAlpha', in2: 'core', operator: 'out', result: 'edge' })
      + el('feFlood', { floodColor: kind === 'neon' ? st.c.accent : st.c.ink, result: 'color' })
      + el('feComposite', { in: 'color', in2: 'edge', operator: 'in', result: 'outline' })
      + el('feComponentTransfer', { in: 'SourceGraphic', result: 'fill' }, el('feFuncA', { type: 'linear', slope: r(phase(k, 0.28, 0.9)) }));
    if (kind === 'neon') primitives += el('feGaussianBlur', { in: 'outline', stdDeviation: 2.2, result: 'halo' });
    primitives += el('feMerge', {}, kind === 'neon' && el('feMergeNode', { in: 'halo' }), el('feMergeNode', { in: 'outline' }), el('feMergeNode', { in: 'fill' }));
  }
  const pad = 8, x = Math.max(0, b.x - pad), y = Math.max(0, b.y - pad);
  const width = Math.max(1, Math.min(s.w, b.x + b.w + pad) - x), height = Math.max(1, Math.min(s.h, b.y + b.h + pad) - y);
  return s.url(unique(s, kind), id => el('filter', { id, filterUnits: 'userSpaceOnUse', x, y, width, height }, primitives));
}

/** 完整内容进场，绝不拿裁切边扫过文字。k=0 也绘制回调，保留排版度量。 */
export function materialEnter(s: Scene, st: Style, value: number, b: Bounds, draw: () => void, role: MaterialRole = 'content'): void {
  const k = clamp(value);
  if (k >= 1) { draw(); return; }
  if (k <= 0) { const first = s.hits.length; drawGroup(s, { visibility: 'hidden' }, draw); s.hits.splice(first); return; }
  const id = identity(st), rest = 1 - k, data = role === 'data';
  const dx = Math.min(14, b.w * 0.05), dy = Math.min(12, b.h * 0.12);
  const cx = b.x + b.w / 2, cy = b.y + b.h / 2;
  const attrs: Attrs = { opacity: k, visibility: k <= 0 ? 'hidden' : undefined };
  let x = 0, y = 0, angle = 0;
  const lean = Math.min(2.4, Math.atan2(Math.min(6, b.h * 0.1), Math.max(1, b.w / 2)) * 180 / Math.PI);
  switch (id) {
    case 'minimal-light': y = 4 * rest; break;
    case 'minimal-dark': attrs.opacity = phase(k, 0.08, 0.72); break;
    case 'glass': y = -dy * rest; attrs.filter = optical(s, st, k, 'focus', b); break;
    case 'film': y = 2 * rest; attrs.opacity = k < 0.15 ? k * 2 : 0.3 + 0.7 * phase(k, 0.15, 1); break;
    case 'cinema': y = dy * rest; attrs.opacity = Math.sin(k * Math.PI / 2) ** 2; break;
    case 'editorial': x = -dx * rest; attrs.opacity = phase(k, 0, 0.35); break;
    case 'broadsheet': attrs.opacity = phase(k, 0, 0.22); attrs.filter = optical(s, st, k, 'press', b, role === 'mark'); break;
    case 'playbill': x = (cx < s.w / 2 ? -1 : 1) * dx * rest; attrs.opacity = phase(k, 0, 0.5); break;
    case 'gala': y = -dy * rest; attrs.opacity = phase(k, 0.12, 0.65); break;
    case 'mono': attrs.opacity = phase(k, 0, 0.25); attrs.filter = optical(s, st, k, 'outline', b); break;
    case 'poster': angle = -lean * rest; attrs.opacity = phase(k, 0, 0.18); attrs.filter = optical(s, st, k, 'press', b, role === 'mark'); break;
    case 'terminal': attrs.opacity = k > 0.06 ? 1 : 0; break;
    case 'crt': attrs.opacity = phase(k, 0, 0.35); attrs.filter = optical(s, st, k, 'crt', b); break;
    case 'neon': attrs.opacity = phase(k, 0, 0.22); attrs.filter = optical(s, st, k, 'neon', b); break;
    case 'arcade': x = Math.round(dx * rest / 4) * -4; attrs.opacity = k > 0.06 ? 1 : 0; break;
    case 'ink': attrs.opacity = Math.sqrt(k); attrs.filter = optical(s, st, k, 'ink', b, role === 'mark'); break;
    case 'ukiyoe': attrs.opacity = phase(k, 0, 0.28); if (!data) attrs.filter = optical(s, st, k, 'register', b); break;
    case 'woodblock': attrs.opacity = phase(k, 0, 0.2); attrs.filter = optical(s, st, k, 'press', b, role === 'mark'); y = -2 * rest; break;
    case 'baroque': y = -dy * rest; angle = (cx < s.w / 2 ? -1 : 1) * lean * rest; break;
    case 'grimoire': attrs.opacity = phase(k, 0.06, 0.5); attrs.filter = optical(s, st, k, 'outline', b); break;
    case 'manuscript': attrs.opacity = phase(k, 0.08, 0.24); x = -3 * rest; break;
    case 'pastel': x = (cx < s.w / 2 ? -1 : 1) * dx * rest; y = -2 * Math.sin(k * Math.PI); break;
    case 'cutout': x = (cx < s.w / 2 ? -1 : 1) * dx * rest; y = -dy * rest; angle = lean * rest; attrs.opacity = 1; attrs.filter = optical(s, st, k, 'paper', b); break;
    case 'meadow': x = dx * Math.sin(k * Math.PI) * rest * 0.35; y = dy * rest; break;
    case 'manga': x = -dx * rest; y = dy * rest * 0.4; attrs.opacity = phase(k, 0, 0.15); break;
    case 'cyber': x = Math.round(dx * rest / 3) * 3; attrs.opacity = phase(k, 0, 0.25); break;
  }
  if (data) { x = y = angle = 0; }
  if (role === 'paper' && id !== 'cutout') delete attrs.filter; // 纸上的行另有落墨；整张纸仅保留自己的投影。
  if (x || y || angle) attrs.transform = `translate(${r(x)} ${r(y)})${angle ? ` rotate(${r(angle)} ${r(cx)} ${r(cy)})` : ''}`;
  const first = s.hits.length;
  drawGroup(s, attrs, draw);
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
  if (role !== 'mark' && ['editorial', 'manuscript'].includes(id)) s.add(stroke(b.x, b.y + b.h, b.x + b.w * k, b.y + b.h));
  if (id === 'broadsheet') s.add(stroke(b.x, b.y, b.x, b.y + b.h * k));
  if (id === 'minimal-dark') s.add(stroke(b.x, b.y + b.h, b.x + b.w * phase(k, 0, 0.6), b.y + b.h));
  if (id === 'film') s.add(stroke(b.x, b.y + b.h * rest / 2, b.x + b.w, b.y + b.h * rest / 2));
  if (id === 'crt') s.add(stroke(b.x, b.y + b.h * k, b.x + b.w, b.y + b.h * k));
  if (id === 'terminal' && role !== 'mark') s.add(rect(b.x + b.w - Math.min(6, b.h * 0.16), b.y + b.h * 0.82, Math.min(6, b.h * 0.16), Math.min(3, b.h * 0.08), { fill: st.c.accent, opacity: rest }));
}

/** 图形的材质展开。文字应调用 materialEnter；这里允许纸、墨、光、像素各自形成边界。 */
export function materialReveal(s: Scene, st: Style, value: number, b: Bounds, direction: RevealDirection, draw: () => void): void {
  const k = clamp(value);
  if (k >= 1) { draw(); return; }
  if (k <= 0) { const first = s.hits.length; drawGroup(s, { visibility: 'hidden' }, draw); s.hits.splice(first); return; }
  const id = identity(st);
  if (['mono', 'neon', 'poster', 'manuscript', 'cutout'].includes(id)) {
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
      const unit = Math.max(7, Math.min(12, Math.min(b.w, b.h) / 3));
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
  drawGroup(s, { ...(mask ? { mask: clip } : { clipPath: clip }), opacity: k <= 0 ? 0 : undefined, filter: soft ? optical(s, st, k, id === 'ink' ? 'ink' : 'focus', b) : undefined }, draw);
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
