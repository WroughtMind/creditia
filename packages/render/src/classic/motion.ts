// 作品自己标出可运动的内容组；这里只负责时间采样，不猜 CSS 尺寸或拆散名字/数值。
import type { Node, Out } from './ui.ts';
import type { Theme } from './themes.ts';

export type MotionKind = 'row' | 'title' | 'bar-x' | 'bar-y' | 'cell' | 'intensity' | 'area' | 'radial' | 'print' | 'sheet' | 'signature' | 'card' | 'credits' | 'clap';
export interface MotionCue { kind: MotionKind; order?: number; total?: number; travel?: number; grid?: number; columns?: number; row?: number; column?: number; rows?: number; level?: number; run?: number; runs?: number }

/** 只加非渲染元数据；静态树的 props、尺寸、顺序均不变。 */
export function motion(node: Node, kind: MotionKind, order = 0, total = 1): Node {
  if (node) node.motionCue = { kind, order, total } satisfies MotionCue;
  return node;
}
/** 调用方传入的就是同一语义集合，例如名单、版本、项目，不遍历叶子猜阅读顺序。 */
export const sequence = (nodes: Node[], kind: MotionKind = 'row'): Node[] => nodes.map((node, i) => motion(node, kind, i, nodes.length));
/** 纸张/标题卡等完整载体；其中的数据图仍可保留自己的局部动作。 */
export function stage(out: Out, kind: MotionKind, travel?: number): Out {
  // 这是作品明确交出的版面区块（标题栏、卡片、纸张），不是叶子或 CSS 属性推断。
  const value = out.tree.props.children;
  const sections = (Array.isArray(value) ? value : [value]).filter((node) => node && typeof node === 'object' && node.props);
  sections.forEach((node, i) => { motion(node, kind, i, sections.length); if (travel) node.motionCue.travel = travel; });
  return out;
}

const clamp = (x: number) => Math.max(0, Math.min(1, x));
const round = (x: number) => Math.round(x * 1000) / 1000;
const digital = (t: Theme) => ['terminal', 'crt', 'arcade'].includes(t.family);
const ink = (t: Theme) => ['editorial', 'broadsheet', 'mono', 'poster', 'playbill'].includes(t.family);
const cinematic = (t: Theme) => ['film', 'cinema', 'gala'].includes(t.family);

/** 时间按内容组分配，密集格子用短出现窗口，名单保留读字的节奏。 */
function phase(time: number, cue: MotionCue, theme: Theme): number {
  if (time >= 1) return 1;
  if (cue.kind === 'intensity' && cue.row !== undefined && cue.column !== undefined) {
    const row = cue.row, col = cue.column, rows = cue.rows ?? 7, cols = cue.columns ?? 24;
    const day = row / Math.max(1, rows - 1), hour = col / Math.max(1, cols - 1);
    const pane = Math.floor(col / 6), panes = Math.ceil(cols / 6);
    let rank = hour, span = .18;
    switch (theme.family) {
      case 'minimal': if (theme.dark) rank = (Math.abs(col - (cols - 1) / 2) - (cols % 2 ? 0 : .5)) / Math.max(1, (cols - 1) / 2 - (cols % 2 ? 0 : .5)); break;
      case 'film': case 'cinema': case 'editorial': case 'playbill': rank = day; span = .3; break;
      case 'glass': rank = pane / Math.max(1, panes - 1); span = .32; break;
      case 'gala': rank = (Math.abs(pane - (panes - 1) / 2) - (panes % 2 ? 0 : .5)) / Math.max(1, (panes - 1) / 2 - .5); span = .34; break;
      case 'broadsheet': case 'mono': rank = Math.min(2, Math.floor((1 - (cue.level ?? 0)) * 3)) / 2; span = .36; break;
      case 'poster': rank = 0; span = .7; break;
      case 'terminal': rank = day; span = .16; break;
      case 'crt': rank = (row * cols + col) / Math.max(1, rows * cols - 1); span = .12; break;
      case 'arcade': rank = (row * panes + pane) / Math.max(1, rows * panes - 1); span = .2; break;
      case 'neon': rank = Math.max(0, cue.run ?? 0) / Math.max(1, (cue.runs ?? 1) - 1); span = .2; break;
    }
    return clamp((time - clamp(rank) * (1 - span)) / span);
  }
  const count = cue.total ?? 1, order = cue.order ?? 0;
  const span = ['cell', 'intensity'].includes(cue.kind) ? 0.18 : cue.kind === 'print' ? 0.22 : cinematic(theme) ? 0.64 : 0.56;
  // 动作顺序属于材质：剧场两侧、颁奖礼中心和街机交错格，不能只换缓动曲线。
  let rank = count > 1 ? order / (count - 1) : 0;
  // 记录与排名行按真实阅读顺序进入；中心展开属于卡片/图形，不倒置列表的主次。
  if (!['print', 'row'].includes(cue.kind) && count > 1 && Number.isInteger(order) && Number.isInteger(count)) {
    if (cue.kind === 'card' && cue.columns && ['playbill', 'gala'].includes(theme.family)) {
      rank = Math.floor(order / cue.columns) / Math.max(1, Math.ceil(count / cue.columns) - 1);
    }
    else if (['playbill', 'gala'].includes(theme.family) || theme.id === 'minimal-dark') {
      const middle = (count - 1) / 2, nearest = count % 2 ? 0 : 0.5;
      rank = (Math.abs(order - middle) - nearest) / Math.max(1, middle - nearest);
    }
    else if (theme.family === 'arcade' && cue.kind === 'cell') rank = (order % 2 ? Math.ceil(count / 2) : 0) / count + Math.floor(order / 2) / count;
  }
  return clamp((time - rank * (1 - span)) / span);
}
export function progress(time: number, cue: MotionCue, theme: Theme): number {
  if (time >= 1) return 1;
  const p = phase(time, cue, theme);
  if (theme.family === 'terminal' && ['row', 'print', 'title', 'signature'].includes(cue.kind)) return p > 0 ? 1 : 0;
  const eased = ink(theme) ? p : cinematic(theme) ? p * p * (3 - 2 * p) : 1 - (1 - p) ** 3;
  return digital(theme) ? Math.floor(eased * 16) / 16 : eased;
}

interface Entry { x?: number; y?: number; opacity?: number; rotate?: number; blur?: number; light?: number; outline?: number; trail?: number }
/** 完整文字/纸面/卡片的材质动作。名字不拆字、不裁切，绝不把百分比当计数器。 */
function entry(cue: MotionCue, p: number, theme: Theme): Entry {
  const rest = 1 - p, side = (cue.order ?? 0) % 2 ? 1 : -1;
  const paper = ['sheet', 'card'].includes(cue.kind), title = cue.kind === 'title';
  switch (theme.family) {
    case 'minimal': return theme.dark
      ? { opacity: clamp(p * 1.5), light: 0.45 + 0.55 * p, outline: rest * 0.4 }
      : { opacity: p, x: title ? 0 : -8 * rest };
    case 'glass': return { opacity: p, y: -10 * rest, blur: 2.4 * rest, light: 1 + 0.24 * Math.sin(Math.PI * p) };
    case 'film': return { opacity: p, y: p < 0.32 ? 2 * (1 - p / 0.32) : 0, light: 0.45 + 0.55 * p, blur: 0.65 * rest };
    case 'cinema': return { opacity: p, y: (cue.travel ?? (title ? 34 : 22)) * rest };
    case 'editorial': return { opacity: clamp(p * 2), x: -24 * rest, y: 0 };
    case 'broadsheet': return { opacity: clamp(p * 3), trail: 1.8 * rest, light: 0.7 + 0.3 * p };
    case 'mono': return { opacity: clamp(p * 4), outline: 1.1 * rest, light: 0.4 + 0.6 * p };
    case 'poster': return { opacity: p === 0 ? 0 : 1, y: -10 * Math.max(0, 1 - p / 0.42), rotate: paper ? -0.8 * rest : 0, trail: 2.2 * Math.max(0, 1 - p / 0.7) };
    case 'playbill': return { opacity: clamp(p * 2), x: paper || title ? 0 : side * 26 * rest, y: title ? -18 * rest : 0 };
    case 'gala': return { opacity: p, y: 12 * rest, light: 1 + 0.7 * Math.sin(Math.PI * p), outline: 0.8 * Math.sin(Math.PI * p) };
    case 'terminal': return { opacity: p > 0 ? 1 : 0 };
    case 'crt': return { opacity: clamp(p * 3), trail: 6 * rest, light: 0.5 + 0.5 * p + 0.5 * Math.sin(Math.PI * p), blur: 0.7 * rest };
    case 'arcade': return { opacity: p > 0 ? 1 : 0, x: paper ? 0 : side * Math.ceil(rest * 3) * 4, y: paper ? -Math.ceil(rest * 3) * 4 : 0 };
    case 'neon': return { opacity: clamp(p * 5), outline: 1.7 * rest, light: 0.45 + 0.55 * clamp((p - 0.2) / 0.8) };
  }
}

function entryStyle(style: Record<string, any>, cue: MotionCue, p: number, theme: Theme) {
  const e = entry(cue, p, theme);
  style.opacity = (style.opacity ?? 1) * (e.opacity ?? 1);
  const transform = [style.transform, e.x || e.y ? `translate(${round(e.x ?? 0)}px, ${round(e.y ?? 0)}px)` : '', e.rotate ? `rotate(${round(e.rotate)}deg)` : ''].filter(Boolean).join(' ');
  if (transform) style.transform = transform;
  // 纸张和卡片保留原有底色；曝光/墨迹只作用于其内部内容，不能把白纸滤成灰纸。
  if (!['sheet', 'card'].includes(cue.kind)) {
    const filter = [e.blur ? `blur(${round(e.blur)}px)` : '', e.light && e.light !== 1 ? `brightness(${round(e.light)})` : ''].filter(Boolean).join(' ');
    if (filter) style.filter = filter;
    if (e.outline) style.textShadow = `${round(e.outline)}px 0 0 ${theme.accent}, ${round(-e.outline)}px 0 0 ${theme.accent}, 0 ${round(e.outline)}px 0 ${theme.accent}, 0 ${round(-e.outline)}px 0 ${theme.accent}`;
    else if (e.trail) style.textShadow = `${round(e.trail)}px 0 ${round(e.trail / 2)}px ${theme.accent}`;
  }
}

/** 堆叠柱的高度与色段都是数据：只沿柱的厚度显形，不能从底部吃掉上层份额。 */
function columnStyle(style: Record<string, any>, cue: MotionCue, p: number, theme: Theme) {
  if (p <= 0) { style.opacity = 0; return; }
  const pct = round(p * 100), rest = 100 - pct;
  const fromLeft = (amount = pct) => { style.clipPath = `inset(0 ${round(100 - amount)}% 0 0)`; };
  const fromCenter = () => { style.clipPath = `inset(0 ${round(rest / 2)}% 0 ${round(rest / 2)}%)`; };
  const edgeInk = (amount: number, soft = 0) => {
    const edge = round(amount * 50), feather = Math.min(soft, edge, Math.max(0, 50 - edge));
    style.maskImage = `linear-gradient(90deg, black 0%, black ${edge}%, transparent ${round(edge + feather)}%, transparent ${round(100 - edge - feather)}%, black ${100 - edge}%, black 100%)`;
  };
  switch (theme.family) {
    case 'minimal': if (theme.dark) { fromCenter(); style.opacity = .3 + .7 * p; } else fromLeft(); break;
    case 'glass':
      style.maskImage = `linear-gradient(90deg, black 0%, black ${round(Math.max(0, pct - Math.min(16, rest)))}%, transparent ${pct}%)`;
      style.filter = `brightness(${round(1 + .4 * (1 - p))})`; break;
    case 'film':
      // 单次曝光从右侧进入；两侧各显一条会把同一版本误看成两根柱。
      style.maskImage = `linear-gradient(270deg, black 0%, black ${round(Math.max(0, pct - Math.min(8, rest)))}%, transparent ${pct}%)`;
      style.filter = `brightness(${round(.45 + .55 * p)})`; break;
    case 'cinema': fromCenter(); style.filter = `brightness(${round(.65 + .35 * p)})`; break;
    case 'editorial': fromLeft(); break;
    case 'broadsheet':
      // 纵向印线贯穿每一层，同一列各色得到相同的墨量。
      style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${round(clamp(p / .96) * 5)}px, transparent ${round(clamp(p / .96) * 5)}px, transparent 5px)`; break;
    case 'mono': edgeInk(clamp(p / .96)); style.opacity = .2 + .8 * p; break;
    case 'poster': style.opacity = 1; break;
    case 'playbill': fromCenter(); break;
    case 'gala': {
      const left = rest / 2, right = 100 - left, feather = Math.min(12, pct / 2, left);
      style.maskImage = `linear-gradient(90deg, transparent ${round(left - feather)}%, black ${round(left)}%, black ${round(right)}%, transparent ${round(right + feather)}%)`;
      style.filter = `brightness(${round(1 + .45 * Math.sin(Math.PI * p))})`; break;
    }
    case 'terminal': {
      // 终端按完整字格列打印，每一列包含同样的全部色段；不再裁掉字格行。
      const width = cue.travel ?? 96, grid = cue.grid ?? 12;
      style.clipPath = `inset(0 ${round(width - Math.floor(p * width / grid) * grid)}px 0 0)`; break;
    }
    case 'crt':
      style.maskImage = `linear-gradient(90deg, black 0%, black ${round(Math.max(0, pct - Math.min(7, rest)))}%, rgba(0,0,0,.35) ${pct}%, transparent ${round(Math.min(100, pct + Math.min(4, pct, rest)))}%)`;
      style.filter = `brightness(${round(1.25 - .25 * p)})`; break;
    case 'arcade': fromLeft(Math.floor(p * 8) / 8 * 100); break;
    case 'neon': edgeInk(clamp(p / .85)); style.opacity = .18 + .82 * p; style.filter = `brightness(${round(.55 + .45 * p)})`; break;
  }
}

/** 横条的长度/位置分别是份额与时间区间：材质只沿厚度形成，左右数据端点始终是真值。 */
function graphicStyle(style: Record<string, any>, cue: MotionCue, p: number, theme: Theme) {
  if (cue.kind === 'bar-y') { columnStyle(style, cue, p, theme); return; }
  if (p <= 0) { style.opacity = 0; return; }
  const rest = round((1 - p) * 100), pct = round(p * 100), alpha = style.opacity ?? 1;
  const rule = (amount = pct) => { style.clipPath = `inset(${round(100 - amount)}% 0 0 0)`; };
  const aperture = () => { style.clipPath = `inset(${round(rest / 2)}% 0 ${round(rest / 2)}% 0)`; };
  const edgeInk = (amount: number, soft = 0) => {
    const edge = round(amount * 50), feather = Math.min(soft, edge, Math.max(0, 50 - edge));
    style.maskImage = `linear-gradient(180deg, black 0%, black ${edge}%, transparent ${round(edge + feather)}%, transparent ${round(100 - edge - feather)}%, black ${100 - edge}%, black 100%)`;
  };
  switch (theme.family) {
    case 'minimal': if (theme.dark) { aperture(); style.opacity = alpha * (.3 + .7 * p); } else rule(); break;
    case 'glass':
      style.maskImage = `linear-gradient(180deg, black 0%, black ${round(Math.max(0, pct - Math.min(16, rest)))}%, transparent ${pct}%)`;
      style.filter = `brightness(${round(1 + .4 * (1 - p))})`; break;
    case 'film': edgeInk(p, 8); style.filter = `brightness(${round(.45 + .55 * p)})`; break;
    case 'cinema': aperture(); style.filter = `brightness(${round(.65 + .35 * p)})`; break;
    case 'editorial': rule(); break;
    case 'broadsheet':
      style.maskImage = `repeating-linear-gradient(0deg, black 0px, black ${round(clamp(p / .96) * 5)}px, transparent ${round(clamp(p / .96) * 5)}px, transparent 5px)`; break;
    case 'mono': edgeInk(clamp(p / .96)); style.opacity = alpha * (.2 + .8 * p); break;
    case 'poster': style.opacity = alpha; break;
    case 'playbill': aperture(); break;
    case 'gala': {
      const top = rest / 2, bottom = 100 - top, feather = Math.min(12, pct / 2, top);
      style.maskImage = `linear-gradient(180deg, transparent ${round(top - feather)}%, black ${round(top)}%, black ${round(bottom)}%, transparent ${round(bottom + feather)}%)`;
      style.filter = `brightness(${round(1 + .45 * Math.sin(Math.PI * p))})`; break;
    }
    case 'terminal': style.opacity = alpha; break;
    case 'crt':
      style.maskImage = `linear-gradient(180deg, black 0%, black ${round(Math.max(0, pct - Math.min(7, rest)))}%, rgba(0,0,0,.35) ${pct}%, transparent ${round(Math.min(100, pct + Math.min(4, pct, rest)))}%)`;
      style.filter = `brightness(${round(1.25 - .25 * p)})`; break;
    case 'arcade': rule(Math.floor(p * 8) / 8 * 100); break;
    case 'neon':
      style.opacity = alpha * (.18 + .82 * p);
      style.filter = `brightness(${round(.55 + .45 * p)})`;
      if (typeof style.backgroundColor === 'string' && /^#[0-9a-f]{6}$/i.test(style.backgroundColor)) {
        const color = style.backgroundColor;
        style.backgroundColor = color + Math.round(clamp((p - .38) / .62) * 255).toString(16).padStart(2, '0');
        style.boxShadow = `inset 0 0 0 1px ${color}`;
      }
      break;
  }
}

function imageSource(src: string, edit: (svg: string) => string): string {
  const encoded = src.slice(src.indexOf(',') + 1);
  const svg = typeof Buffer !== 'undefined' ? Buffer.from(encoded, 'base64').toString() : new TextDecoder().decode(Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0)));
  const result = edit(svg);
  if (typeof Buffer !== 'undefined') return 'data:image/svg+xml;base64,' + Buffer.from(result).toString('base64');
  let bin = ''; for (const byte of new TextEncoder().encode(result)) bin += String.fromCharCode(byte);
  return 'data:image/svg+xml;base64,' + btoa(bin);
}

function ringMotion(svg: string, p: number, theme: Theme): string {
  if (theme.family === 'terminal') {
    // 一个归属弧的字格一起打印；沿角度切片会暂时把真实份额画短。
    const size = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 420);
    const rect = /<rect x="([\d.-]+)" y="([\d.-]+)" width="([\d.-]+)" height="([\d.-]+)" fill="url\(#(tg\d+)\)"\/>/g;
    const starts = new Map<string, number>();
    for (const match of svg.matchAll(rect)) {
      for (let x = Number(match[1]); x < Number(match[1]) + Number(match[3]); x += 12) {
        const angle = (Math.atan2(Number(match[2]) + 6 - size / 2, x + 6 - size / 2) / (2 * Math.PI) + 1.25) % 1;
        starts.set(match[5], Math.min(starts.get(match[5]) ?? 1, angle));
      }
    }
    const groups = [...starts].sort((a, b) => a[1] - b[1]).map(([id]) => id);
    return svg.replace(rect, (whole, _x, _y, _w, _h, id) => p * groups.length > groups.indexOf(id) ? whole : '');
  }
  const arcs = /<circle\b[^>]*stroke-dasharray="[^"]+"[^>]*\/>/g;
  const all = [...svg.matchAll(arcs)].map(match => match[0]);
  if (!all.length) return svg;
  if (['editorial', 'playbill', 'poster'].includes(theme.family)) {
    // 印版、节目单和海报落下的是完整真实弧；角度和起点从不插值。
    let index = 0;
    return svg.replace(arcs, circle => {
      const group = theme.family === 'playbill' ? Math.floor(index++ / 2) : index++;
      const groups = theme.family === 'playbill' ? Math.ceil(all.length / 2) : all.length;
      return (theme.family === 'poster' ? p > 0 : p * groups > group) ? circle : circle.replace('<circle ', '<circle opacity="0" ');
    });
  }
  const attr = (name: string) => Number(new RegExp(`\\b${name}="([\\d.]+)"`).exec(all[0])?.[1]);
  const cx = attr('cx'), cy = attr('cy'), radius = attr('r'), width = attr('stroke-width');
  const band = (r: number, w: number, color = 'white') => `<circle cx="${cx}" cy="${cy}" r="${round(r)}" fill="none" stroke="${color}" stroke-width="${round(w)}"/>`;
  const full = `<rect width="${cx * 2}" height="${cy * 2}" fill="white"/>`;
  let defs = '', mask = '', opacity = 1;
  switch (theme.family) {
    case 'minimal':
      mask = theme.dark ? band(radius, width) + band(radius, width * (1 - p), 'black') : band(radius - width * (1 - p) / 2, width * p); break;
    case 'glass': mask = band(radius + width * (1 - p) / 2, width * p); break;
    case 'film':
      // 全弧一起曝光，边缘乳剂先显，再向整片厚度凝结；不用角度缺口制造碎片。
      mask = band(radius, width) + band(radius, width * (1 - p), 'black') + `<g opacity="${round(p * p)}">${full}</g>`;
      opacity = .45 + .55 * p; break;
    case 'cinema': mask = band(radius, width * p); break;
    case 'broadsheet': {
      const ink = clamp(p / .96);
      defs = `<pattern id="cm-ring-print" patternUnits="userSpaceOnUse" width="6" height="6"><rect width="6" height="${round(ink * 6)}" fill="white"/></pattern>`;
      mask = `<rect width="${cx * 2}" height="${cy * 2}" fill="url(#cm-ring-print)"/>`; break;
    }
    case 'mono': mask = band(radius, width) + band(radius, width * (1 - clamp(p / .96)), 'black'); break;
    case 'gala': {
      const outer = radius + width / 2, half = width * p / 2, feather = Math.min(width * .18, half, width * (1 - p) / 2);
      const at = (r: number) => round(r / outer);
      defs = `<radialGradient id="cm-ring-light" gradientUnits="userSpaceOnUse" cx="${cx}" cy="${cy}" r="${outer}"><stop offset="${at(radius - half - feather)}" stop-color="black"/><stop offset="${at(radius - half)}" stop-color="white"/><stop offset="${at(radius + half)}" stop-color="white"/><stop offset="${at(radius + half + feather)}" stop-color="black"/></radialGradient>`;
      mask = `<rect width="${cx * 2}" height="${cy * 2}" fill="url(#cm-ring-light)"/>`; break;
    }
    case 'crt': {
      const scan = Math.floor(p * 16) / 16;
      mask = band(radius - width * (1 - scan) / 2, width * scan); break;
    }
    case 'arcade': mask = band(radius, width * Math.floor(p * 8) / 8); break;
    case 'neon': mask = band(radius, width * clamp(p / .85)); break;
  }
  // 连续材质在末段填实整张遮罩，避免边缘重复抗锯齿在撤罩时突然加深。
  if (!['crt', 'arcade'].includes(theme.family) && p > .85) mask += `<g opacity="${round(clamp((p - .85) / .15))}">${full}</g>`;
  if (p <= 0) mask = '';
  const definition = `<defs>${defs}<mask id="cm-ring" maskUnits="userSpaceOnUse" x="0" y="0" width="${cx * 2}" height="${cy * 2}">${mask}</mask></defs>`;
  return svg.replace(/(<svg[^>]*>)/, '$1' + definition).replace(arcs, circle => circle.replace('<circle ', `<circle mask="url(#cm-ring)" opacity="${round(opacity)}" `));
}

/** 面积图的轴和网格不动；只给明确的面积层加印刷、光学或格子成形。 */
function areaMotion(svg: string, w: number, h: number, p: number, theme: Theme, grid?: number): string {
  if (grid) {
    // 时间轴逐列打印整叠字格；每列保留原 y/h，不把真实份额缩到基线再取整。
    const edge = Math.floor(w * p / grid) * grid;
    return svg.replace(/<rect x="([\d.-]+)" y="([\d.-]+)" width="([\d.-]+)" height="([\d.-]+)" fill="url\(#(tg\d+)\)"\/>/g,
      (whole, x, _y, rw) => Number(x) + Number(rw) <= edge ? whole : '');
  }

  const rest = 1 - p, rect = (x: number, y: number, rw: number, rh: number) => `<rect x="${round(x)}" y="${round(y)}" width="${round(rw)}" height="${round(rh)}" fill="white"/>`;
  let shape = rect(0, 0, w * p, h), extra = '', attributes = '';
  switch (theme.family) {
    case 'minimal': if (theme.dark) { shape = rect(w * rest / 2, 0, w * p, h); attributes = ` opacity="${round(0.3 + 0.7 * p)}"`; } break;
    case 'glass':
      // 光沿数据时间轴前进，不能让斜向渐变的右下角在 p=1 时仍在黑区。
      extra = `<linearGradient id="cm-light" x1="0" y1="0" x2="1" y2="0"><stop offset="${Math.max(0, p - Math.min(0.14, rest))}" stop-color="white"/><stop offset="${p}" stop-color="black"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-light)"/>`; break;
    case 'film': {
      // 胶片从时间两端曝光，任一时刻的整列层比相同；不用上下硬窗切断河层。
      const edge = p / 2, feather = Math.min(.08, edge, rest / 2);
      extra = `<linearGradient id="cm-exposure"><stop offset="${round(edge)}" stop-color="white"/><stop offset="${round(edge + feather)}" stop-color="black"/><stop offset="${round(1 - edge - feather)}" stop-color="black"/><stop offset="${round(1 - edge)}" stop-color="white"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-exposure)"/>` + rect(0, 0, w, h).replace('fill="white"', `fill="white" opacity="${round(p * p)}"`);
      attributes = ` opacity="${round(.45 + .55 * p)}"`; break;
    }
    case 'cinema': shape = rect(w * rest / 2, 0, w * p, h); break;
    case 'editorial': {
      // 从折线本身取真实周列；每栏同时显现完整截面，不能由顶端切出悬浮的帽子。
      const data = /<g data-creditia-area="true">([\s\S]*?)<\/g>/.exec(svg)?.[1] ?? '';
      const first = /<path d="([^"]+)"/.exec(data)?.[1] ?? '';
      const weeks = [...new Set([...first.matchAll(/[ML]([\d.]+),[\d.-]+/g)].map(m => Number(m[1])))].sort((a, b) => a - b);
      const columns = weeks.length ? weeks : [w / 2];
      shape = columns.map((x, i) => {
        const left = i ? (columns[i - 1] + x) / 2 : 0, right = i + 1 < columns.length ? (x + columns[i + 1]) / 2 : w;
        return rect(left, 0, right - left + 0.02, h).replace('fill="white"', `fill="white" opacity="${round(clamp(p * columns.length - i))}"`);
      }).join(''); break;
    }
    case 'broadsheet':
      extra = `<pattern id="cm-ink" patternUnits="userSpaceOnUse" width="6" height="6">${rect(0, 0, 6, p * 6)}</pattern>`;
      shape = `<rect width="${w}" height="${h}" fill="white" opacity="${round(clamp((p - 0.65) / 0.35))}"/><rect width="${w}" height="${h}" fill="url(#cm-ink)"/>`; break;
    case 'mono':
      extra = `<pattern id="cm-ink" patternUnits="userSpaceOnUse" width="8" height="8">${rect(0, 0, p * 8, 8)}</pattern>`;
      shape = `<rect width="${w}" height="${h}" fill="white" opacity="${round(clamp((p - 0.65) / 0.35))}"/><rect width="${w}" height="${h}" fill="url(#cm-ink)"/>`; break;
    case 'poster': {
      // 每种墨色一版压上，面积与折线的最终位置从第一帧起就固定。
      let index = 0;
      const count = (svg.match(/<path /g) ?? []).length;
      return svg.replace(/<path /g, () => `<path opacity="${round(clamp((p * (count + 1) - index++) / 2))}" `);
    }
    case 'playbill': shape = rect(0, 0, w * p / 2, h) + rect(w * (1 - p / 2), 0, w * p / 2, h); break;
    case 'gala': {
      // 聚光沿时间中轴向两侧展开，每个时刻的全部层使用同一曝光；不留下中央孤岛。
      const left = rest / 2, right = 1 - left, feather = Math.min(.14, p / 2, left);
      extra = `<linearGradient id="cm-light"><stop offset="${round(left - feather)}" stop-color="black"/><stop offset="${round(left)}" stop-color="white"/><stop offset="${round(right)}" stop-color="white"/><stop offset="${round(right + feather)}" stop-color="black"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-light)"/>` + rect(0, 0, w, h).replace('fill="white"', `fill="white" opacity="${round(clamp((p - .85) / .15))}"`); break;
    }
    case 'crt':
      extra = `<linearGradient id="cm-light"><stop offset="${Math.max(0, p - 0.04)}" stop-color="white"/><stop offset="${p}" stop-color="#aaa"/><stop offset="${Math.min(1, p + 0.02)}" stop-color="black"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-light)"/>`; break;
    case 'arcade': {
      // 像素时段逐格打印完整截面，不能把每列从基线长高成假的峰值。
      const unit = 16, columns = Math.ceil(w / unit);
      shape = rect(0, 0, Math.floor(p * columns) * unit, h); break;
    }
    case 'neon': {
      // resvg 不采纳 pathLength；沿数据的时间轴通电，先描灯管再点亮底下的面积。
      const beam = `<defs data-creditia-motion="area"><clipPath id="cm-tube">${rect(-20, -20, w * clamp(p * 1.5) + 20, h + 40)}</clipPath></defs>`;
      return svg.replace('<g data-creditia-area="true">', `${beam}<g data-creditia-area="true">`)
        .replace(/<path ([^>]*?)\/>/g, (all, attrs) => attrs.includes('stroke=')
          ? `<path ${attrs} clip-path="url(#cm-tube)" opacity="${p > 0 ? 1 : 0}"/>`
          : `<path ${attrs} opacity="${round(clamp((p - 0.35) / 0.65))}"/>`);
    }
    case 'terminal': break;
  }
  const defs = `<defs data-creditia-motion="area">${extra}<mask id="cm-area" maskUnits="userSpaceOnUse" x="0" y="0" width="${w}" height="${h}">${shape}</mask></defs>`;
  return svg.replace('<g data-creditia-area="true">', `${defs}<g data-creditia-area="true" mask="url(#cm-area)"${attributes}>`);
}

/** 徽章是 20–28 像素高的完整签名。只动字组/边线，不能把整张徽章缩小。 */
export function animateBadge(svg: string, time: number, theme: Theme): string {
  if (time >= 1) return svg;
  const w = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 200), h = Number(/height="([\d.]+)"/.exec(svg)?.[1] ?? 24);
  const count = (svg.match(/<text\b/g) ?? []).length;
  let i = 0;
  let sampled = svg.replace(/<text\b[^>]*>[\s\S]*?<\/text>/g, (text) => {
    const cue: MotionCue = { kind: 'signature', order: i++, total: count };
    const p = progress(time, cue, theme), e = entry(cue, p, theme);
    // 小徽章留白很窄，位移按实际可用空间收束。文字始终是完整名字/比例。
    const x = Math.max(-4, Math.min(4, e.x ?? 0)), y = Math.max(-1, Math.min(1, e.y ?? 0));
    const transform = x || y ? ` transform="translate(${round(x)} ${round(y)})"` : '';
    const ink = /fill="([^"]+)"/.exec(text)?.[1] ?? theme.accent;
    const outline = e.outline ? ` stroke="${ink}" stroke-width="${round(e.outline * 0.25)}" paint-order="stroke"` : '';
    const face = ['mono', 'neon'].includes(theme.family) && p < 0.45 ? text.replace(/fill="[^"]+"/g, 'fill="none"') : text;
    const ghost = e.trail ? `<g aria-hidden="true" opacity="${round((e.opacity ?? 1) * (1 - p) * 0.35)}" transform="translate(${round(Math.min(2, e.trail))} 0)">${text}</g>` : '';
    return `${ghost}<g data-creditia-badge="text" opacity="${round(e.opacity ?? 1)}"${transform}${outline}>${face}</g>`;
  });
  const p = progress(time, { kind: 'card' }, theme), rest = 1 - p;
  // 细边框沿原路径描出；实体色条保持原有厚度，只按家族的方向展现。
  sampled = sampled.replace(/<rect\b([^>]*stroke="[^\"]+"[^>]*)\/>/g, (_, attrs) => {
    const rw = Number(/\bwidth="([\d.]+)"/.exec(attrs)?.[1] ?? w), rh = Number(/\bheight="([\d.]+)"/.exec(attrs)?.[1] ?? h);
    const perimeter = 2 * (rw + rh);
    return `<rect ${attrs} stroke-dasharray="${perimeter}" stroke-dashoffset="${round(perimeter * rest)}"/>`;
  });
  let ornament = '';
  if (theme.family === 'glass') ornament = `<path d="M${round(w * p - 18)} 0 l14 0 -8 ${h} -14 0Z" fill="white" opacity="${round(Math.sin(Math.PI * p) * 0.25)}"/>`;
  if (theme.family === 'crt') ornament = `<rect x="${round(w * p)}" width="1" height="${h}" fill="${theme.accent}" opacity="${round(Math.sin(Math.PI * p) * 0.6)}"/>`;
  if (theme.family === 'terminal') ornament = `<rect x="${w - 8}" y="${h - 6}" width="4" height="2" fill="${theme.accent}" opacity="${p < 0.8 ? 1 : 0}"/>`;
  if (theme.family === 'arcade') ornament = `<rect x="${round(Math.floor(p * (w / 4)) * 4)}" y="${h - 3}" width="4" height="2" fill="${theme.accent}"/>`;
  // 装饰置于字之前，保持对比和可读性。它们在静态终帧全部移除。
  if (ornament) sampled = sampled.replace('<g data-creditia-badge="text"', `${ornament}<g data-creditia-badge="text"`);
  return sampled;
}

/** 对已排版的克隆树采样。只改绘制属性；宽高、margin、gap 和文本不参与动画。 */
export function animate(tree: Node, time: number, theme: Theme): void {
  if (time >= 1) return;
  const visit = (node: Node, clock: number) => {
    if (!node || typeof node !== 'object' || !node.props) return;
    const cue: MotionCue | undefined = node.motionCue;
    if (cue) {
      const p = progress(clock, cue, theme);
      const style = { ...node.props.style };
      const translate = (x: number, y: number) => { style.transform = [style.transform, `translate(${round(x)}px, ${round(y)}px)`].filter(Boolean).join(' '); };
      // 到达终点时连临时样式也移除；避免 inset(0) 裁掉光晕、边框和描边。
      if (p < 1) switch (cue.kind) {
        case 'bar-x': case 'bar-y':
          // 字符条是完整文字，不能用连续裁窗切出半个方块字。
          if (typeof node.props.children === 'string') style.opacity = p > 0 ? (style.opacity ?? 1) : 0;
          else graphicStyle(style, cue, p, theme);
          break;
        case 'intensity':
          // 强度就是数据，绝不渐变/模糊/缩放；有坐标的 clock 单元按完整行、时段板块或真实墨色版归位。
          style.opacity = p > 0 || (theme.family === 'neon' && cue.level === 0) ? (style.opacity ?? 1) : 0;
          if (cue.row !== undefined && cue.column !== undefined && p > 0) {
            const rest = 1 - p;
            if (theme.family === 'film' || theme.family === 'editorial') translate(-4 * rest, 0);
            else if (theme.family === 'glass' || theme.family === 'gala') translate(0, -4 * rest);
            else if (theme.family === 'broadsheet' || theme.family === 'mono') translate(0, -2 * rest);
            else if (theme.family === 'poster') translate(0, -5 * rest);
            else if (theme.family === 'cinema') translate(0, 5 * rest);
            else if (theme.family === 'playbill') translate((cue.column < (cue.columns ?? 24) / 2 ? -1 : 1) * 4 * rest, 0);
            else if (theme.family === 'arcade') translate(0, -Math.ceil(rest * 2) * 3);
          }
          break;
        case 'area':
          if (typeof node.props.src === 'string') node.props = { ...node.props, src: imageSource(node.props.src, (svg) => areaMotion(svg, Number(node.props.width), Number(node.props.height), p, theme, cue.grid)) };
          break;
        case 'radial':
          if (typeof node.props.src === 'string') {
            node.props = { ...node.props, src: imageSource(node.props.src, (svg) => ringMotion(svg, p, theme)) };
          }
          break;
        case 'clap': style.transform = [style.transform, `rotate(${round(-18 * (1 - p))}deg)`].filter(Boolean).join(' '); break;
        case 'cell': {
          const rest = 1 - p;
          style.opacity = p;
          if (theme.family === 'arcade') { style.opacity = p > 0 ? 1 : 0; translate(0, -Math.ceil(rest * 4) * 4); }
          else if (theme.family === 'glass') { style.opacity = 0.12 + 0.88 * p; translate(0, -6 * rest); }
          else if (theme.family === 'poster') { style.opacity = p > 0 ? 1 : 0; translate((cue.order ?? 0) % 2 ? 4 * rest : -4 * rest, -5 * rest); }
          else if (theme.family === 'playbill') translate(((cue.order ?? 0) % 2 ? 1 : -1) * 14 * rest, 0);
          else if (theme.family === 'gala') { translate(0, 6 * rest); style.filter = `brightness(${round(1 + .25 * Math.sin(Math.PI * p))})`; }
          else if (theme.family === 'editorial') translate(-10 * rest, 0);
          else if (theme.family === 'broadsheet') { style.opacity = p > 0.15 ? 1 : p / 0.15; translate(0, -2 * rest); }
          else if (theme.family === 'mono') style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${round(p * 4)}px, transparent ${round(p * 4)}px, transparent 4px)`;
          else if (theme.family === 'neon') { style.opacity = 0.12 + 0.88 * p; style.boxShadow = `inset 0 0 0 1px ${theme.accent}`; }
          else if (theme.family === 'crt') style.opacity = clamp(p * 2);
          else if (theme.family === 'terminal') style.opacity = p > 0 ? 1 : 0;
          else if (theme.family === 'film') translate(0, 3 * rest);
          else if (theme.family === 'cinema') translate(0, 12 * rest);
          else if (theme.dark) style.filter = `brightness(${round(.65 + .35 * p)})`;
          break;
        }
        default: entryStyle(style, cue, p, theme); break;
      }
      node.props = { ...node.props, style };
    }
    const kids = node.props.children;
    const childClock = cue ? phase(clock, cue, theme) : clock;
    for (const kid of Array.isArray(kids) ? kids : [kids]) visit(kid, childClock);
  };
  visit(tree, clamp(time));
}
