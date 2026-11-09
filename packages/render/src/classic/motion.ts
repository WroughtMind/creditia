// 作品自己标出可运动的内容组；这里只负责时间采样，不猜 CSS 尺寸或拆散名字/数值。
import type { Node, Out } from './ui.ts';
import type { Theme } from './themes.ts';
import { measure, wrap } from '../svg.ts';
import type { Seen } from './hits.ts';

export type MotionKind = 'row' | 'title' | 'bar-x' | 'bar-y' | 'cell' | 'intensity' | 'area' | 'radial' | 'print' | 'neon-print' | 'neon-guide' | 'neon-lane' | 'lane-rule' | 'sheet' | 'signature' | 'card' | 'credits' | 'clap';
export interface MotionCue { kind: MotionKind; order?: number; total?: number; travel?: number; grid?: number; plane?: 'count' | 'area' | 'time'; columns?: number; row?: number; column?: number; rows?: number; level?: number; run?: number; runs?: number; lane?: string; guide?: { x?: number; y?: number; width: number; textChild?: number } }

interface TerminalCursorTarget { group: Node; texts: { node: Node; inherited: Record<string, any> }[] }
/** 只记录作者明确交出的字组；嵌套字组各自负责自己的字，不重复在整张卡片上放读头。 */
export function terminalCursorTargets(tree: Node): TerminalCursorTarget[] {
  const groups: TerminalCursorTarget[] = [];
  let textId = 0;
  const visit = (node: Node, inherited: Record<string, any> = {}, target?: TerminalCursorTarget) => {
    if (!node?.props) return;
    const style = { ...inherited, ...node.props.style };
    if (['row', 'title', 'print', 'signature', 'credits'].includes(node.motionCue?.kind)) {
      node.key ??= `creditia-output:${groups.length}`;
      target = { group: node, texts: [] };
      groups.push(target);
    }
    if (target && typeof node.props.children === 'string') {
      node.key ??= `creditia-output-text:${textId++}`;
      target.texts.push({ node, inherited });
    }
    const textStyle = Object.fromEntries(['fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'lineHeight', 'letterSpacing', 'textAlign', 'textTransform', 'whiteSpace'].filter(key => style[key] !== undefined).map(key => [key, style[key]]));
    for (const child of [node.props.children].flat()) visit(child, textStyle, target);
  };
  visit(tree);
  return groups;
}

export function placeTerminalCursors(groups: TerminalCursorTarget[], seen: Seen[]): void {
  for (const { group, texts } of groups) {
    const box = seen.find(node => node.key === group.key);
    if (!box) continue;
    const lines: { left: number; top: number; width: number; height: number; size: number }[] = [];
    for (const { node, inherited } of texts) {
      const laid = seen.find(item => item.key === node.key);
      if (!laid?.textContent?.trim() || /^[■·\s]+$/.test(laid.textContent)) continue;
      const style = { ...inherited, ...node.props.style };
      const size = Number(style.fontSize), spacing = Number(style.letterSpacing ?? 0);
      if (!size || !style.fontFamily) continue;
      const font = { family: style.fontFamily, weight: Number(style.fontWeight ?? 400), italic: style.fontStyle === 'italic' };
      let text = laid.textContent;
      if (style.textTransform === 'uppercase') text = text.toUpperCase();
      else if (style.textTransform === 'lowercase') text = text.toLowerCase();
      const pre = style.whiteSpace === 'pre' || style.whiteSpace === 'pre-wrap';
      if (!pre) text = text.replace(/\s+/g, ' ').trim();
      let rows = pre ? text.split('\n') : [text];
      const widthOf = (line: string) => measure(line, font, size, spacing / size);
      const lineHeight = typeof style.lineHeight === 'number' ? style.lineHeight * size : 0;
      if (rows.some(line => widthOf(line) > laid.width + 1)) {
        if (style.whiteSpace === 'nowrap' || style.whiteSpace === 'pre' || !lineHeight) continue;
        rows = rows.flatMap(line => wrap(line, font, size, laid.width, spacing / size));
      }
      // Satori 报的是文本布局盒。只在断行与最终盒高相符时使用分行，无法确认的折行不猜字尾。
      if (rows.some(line => widthOf(line) > laid.width + 1)
        || (rows.length > 1 && (!lineHeight || Math.abs(laid.height - rows.length * lineHeight) > 2))
        || (rows.length === 1 && laid.height > (lineHeight || size * 1.6) + 2)) continue;
      const height = laid.height / rows.length;
      rows.forEach((line, i) => {
        const width = widthOf(line);
        if (!line.trim()) return;
        const offset = style.textAlign === 'center' ? (laid.width - width) / 2 : style.textAlign === 'right' || style.textAlign === 'end' ? laid.width - width : 0;
        lines.push({ left: laid.left + offset, top: laid.top + i * height, width, height, size });
      });
    }
    // 同行多字段只在最右一个后停留，不把标签和真值各造一个光标。
    const rows: typeof lines = [];
    for (const line of lines.sort((a, b) => a.top - b.top || a.left - b.left)) {
      const old = rows.findIndex(row => Math.abs(row.top + row.height - line.top - line.height) < 3);
      if (old < 0) rows.push(line);
      else if (line.left + line.width > rows[old].left + rows[old].width) rows[old] = line;
    }
    group.motionCursor = rows.flatMap(row => {
      const w = Math.max(3, Math.min(12, row.size * .3)), h = Math.max(1.5, row.size * .075);
      const end = row.left + row.width - box.left, y = row.top + row.height - box.top;
      if (end + w + 3 <= box.width) return [{ x: end + 3, y: y - h - 2, w, h }];
      // 右侧没有净空时，只有真正的下方留白才容纳下划读头；不能覆盖末字或下伸笔画。
      if (y + h + 2 <= box.height) return [{ x: Math.max(0, end - w), y: y + 2, w, h }];
      return [];
    });
  }
}

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
  if (cue.kind === 'neon-lane' || cue.kind === 'lane-rule') return time;
  if (theme.family === 'neon' && cue.kind === 'bar-x' && cue.lane) {
    const start = (cue.order ?? 0) / Math.max(1, (cue.total ?? 1) - 1);
    return clamp((time - start * .8) / .2);
  }
  if (cue.kind === 'bar-x' && cue.lane && ['editorial', 'mono', 'cinema', 'playbill', 'gala', 'crt'].includes(theme.family)) {
    const start = (cue.order ?? 0) / Math.max(1, (cue.total ?? 1) - 1);
    return clamp((time - start * .72) / .28);
  }
  if (cue.kind === 'intensity' && cue.row !== undefined && cue.column !== undefined) {
    const row = cue.row, col = cue.column, rows = cue.rows ?? 7, cols = cue.columns ?? 24;
    const day = row / Math.max(1, rows - 1), hour = col / Math.max(1, cols - 1);
    const pane = Math.floor(col / 6), panes = Math.ceil(cols / 6);
    let rank = hour, span = .18;
    switch (theme.family) {
      case 'minimal': if (theme.dark) rank = (Math.abs(col - (cols - 1) / 2) - (cols % 2 ? 0 : .5)) / Math.max(1, (cols - 1) / 2 - (cols % 2 ? 0 : .5)); break;
      case 'film': case 'editorial': rank = day; span = .3; break;
      case 'cinema': rank = Math.floor(col / 12); span = .58; break;
      case 'playbill': rank = 0; span = .76; break;
      case 'glass': rank = pane / Math.max(1, panes - 1); span = .32; break;
      case 'gala': rank = day; span = .3; break;
      case 'broadsheet': rank = day; span = .3; break;
      case 'mono': rank = Math.min(2, Math.floor((1 - (cue.level ?? 0)) * 3)) / 2; span = .36; break;
      case 'poster': rank = 0; span = .7; break;
      case 'terminal': rank = day; span = .16; break;
      case 'crt': rank = day; span = .24; break;
      case 'arcade': rank = (row * panes + pane) / Math.max(1, rows * panes - 1); span = .2; break;
      case 'neon': rank = Math.max(0, cue.run ?? 0) / Math.max(1, (cue.runs ?? 1) - 1); span = .2; break;
    }
    return clamp((time - clamp(rank) * (1 - span)) / span);
  }
  const count = cue.total ?? 1, order = cue.order ?? 0;
  const span = ['cell', 'intensity'].includes(cue.kind) ? 0.18 : ['print', 'neon-print', 'neon-guide'].includes(cue.kind) ? 0.22 : cinematic(theme) ? 0.64 : 0.56;
  // 动作顺序属于材质：剧场两侧、颁奖礼中心和街机交错格，不能只换缓动曲线。
  let rank = count > 1 ? order / (count - 1) : 0;
  // 记录与排名行按真实阅读顺序进入；中心展开属于卡片/图形，不倒置列表的主次。
  if (!['print', 'neon-print', 'neon-guide', 'row', 'credits', 'title'].includes(cue.kind) && !(theme.family === 'playbill' && cue.kind === 'bar-y') && count > 1 && Number.isInteger(order) && Number.isInteger(count)) {
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
  if (cue.kind === 'neon-print' || cue.kind === 'neon-guide' || cue.kind === 'neon-lane' || cue.kind === 'lane-rule' || (cue.kind === 'bar-x' && cue.lane)) return p;
  if (theme.family === 'terminal' && ['row', 'print', 'title', 'signature'].includes(cue.kind)) return p;
  if (['crt', 'arcade'].includes(theme.family) && ['row', 'print', 'title', 'signature', 'credits'].includes(cue.kind)) return Math.floor(p * 16) / 16;
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
    case 'film': return paper
      ? { opacity: clamp(p / .24), y: -14 * rest, blur: 2.2 * rest * rest }
      : { opacity: clamp(p / .3), y: 7 * rest, light: .75 + .25 * p, blur: 2.2 * rest * rest };
    case 'cinema': return { opacity: clamp(p / .32), y: (cue.travel ?? (title ? 34 : 28)) * rest, blur: 2.4 * rest * rest };
    case 'editorial': return { opacity: clamp(p * 5), x: -10 * rest, y: 0 };
    case 'broadsheet': return { opacity: clamp(p / .12), x: -8 * rest, outline: 1.6 * rest };
    case 'mono': return { opacity: clamp(p * 4), outline: 1.1 * rest };
    case 'poster': return { opacity: p === 0 ? 0 : 1, y: -10 * Math.max(0, 1 - p / 0.42), rotate: paper ? -0.8 * rest : 0, trail: 2.2 * Math.max(0, 1 - p / 0.7) };
    case 'playbill': return { opacity: clamp(p * 5), y: title ? -12 * rest : (paper ? 12 : 8) * rest };
    case 'gala': return { opacity: clamp(p / .22), y: 12 * rest, light: 1 + 0.7 * Math.sin(Math.PI * p), outline: 2.4 * rest };
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
  // 纸张和卡片保留原有底色；电影卡片只收焦边缘，不用亮度滤镜把白纸变灰。
  if (['sheet', 'card'].includes(cue.kind) && theme.family === 'film' && e.blur) style.filter = `blur(${round(e.blur)}px)`;
  if (theme.family === 'editorial' && !['sheet', 'card'].includes(cue.kind) && p > 0) {
    // 校样线属于完整行的下沿，不横穿字面，也不改变盒子尺寸。
    const alpha = Math.round(.55 * Math.sin(Math.PI * p) * 255).toString(16).padStart(2, '0');
    const proof = `linear-gradient(0deg, ${theme.accent}${alpha} 0px, ${theme.accent}${alpha} 1px, transparent 1px)`;
    style.backgroundImage = [proof, style.backgroundImage].filter(Boolean).join(', ');
  }
  if (!['sheet', 'card'].includes(cue.kind)) {
    const filter = [e.blur ? `blur(${round(e.blur)}px)` : '', e.light && e.light !== 1 ? `brightness(${round(e.light)})` : ''].filter(Boolean).join(' ');
    if (filter) style.filter = filter;
    if (e.outline) style.textShadow = `${round(e.outline)}px 0 0 ${theme.accent}, ${round(-e.outline)}px 0 0 ${theme.accent}, 0 ${round(e.outline)}px 0 ${theme.accent}, 0 ${round(-e.outline)}px 0 ${theme.accent}`;
    else if (e.trail) style.textShadow = `${round(e.trail)}px 0 ${round(e.trail / 2)}px ${theme.accent}`;
    if (theme.family === 'crt' && p > 0 && !style.backgroundImage) {
      // 刷新头经过完整字组的下沿，不把扫描窗压在名字/百分数上。
      const head = -12 + 124 * p, a = Math.round(170 * Math.sin(Math.PI * p)).toString(16).padStart(2, '0');
      style.backgroundImage = [`linear-gradient(90deg, transparent ${round(head - 12)}%, ${theme.accent}${a} ${round(head)}%, transparent ${round(head + 3)}%)`, style.backgroundImage].filter(Boolean).join(', ');
      style.backgroundSize = '100% 2px'; style.backgroundPosition = '0 100%'; style.backgroundRepeat = 'no-repeat';
    }
    if (theme.family === 'arcade' && p > 0) {
      // 两个整字像素色面在原字上对位；保持字组完整，不逐字打印或裁字。
      const step = p >= .88 ? 0 : Math.ceil((1 - p / .88) * 3) * 2;
      style.textShadow = `${step}px 0 0 ${theme.accent}, 0 ${step}px 0 ${theme.faint}`;
    }
  }
}

/** 完整字组/真实数据面保留连通底色，材质在其表面形成。 */
function printAndScreenMask(p: number, kind: string, text = false, horizontalValue = false): string {
  const floor = text ? kind === 'gala' ? .55 : .76 : .46, base = round(floor + (1 - floor) * clamp((p - .62) / .38));
  if (kind === 'broadsheet') {
    const pitch = 8, ink = round(pitch * (.12 + .88 * clamp(p / .92)));
    return `repeating-linear-gradient(0deg, black 0px, black ${ink}px, rgba(0,0,0,${base}) ${ink}px, rgba(0,0,0,${base}) ${pitch}px)`;
  }
  if (kind === 'arcade') {
    const level = Math.floor(p * 8) / 8;
    if (horizontalValue) {
      // 所有横坐标保留同样的真实份额，像素只沿条面厚度逐排接通。
      const ink = round(8 * Math.min(1, Math.ceil(p * 8) / 8));
      return `repeating-linear-gradient(0deg, black 0px, black ${ink}px, rgba(0,0,0,${base}) ${ink}px, rgba(0,0,0,${base}) 8px)`;
    }
    const marks = [[0, 0], [4, 4], [4, 0], [0, 4]].map(([x, y], i) => `<rect x="${x}" y="${y}" width="4" height="4" fill="white" opacity="${level >= i / 4 ? 1 : 0}"/>`).join('');
    return `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="white" opacity="${base}"/>${marks}</svg>`)}")`;
  }
  const at = -30 + 160 * p, feather = kind === 'gala' ? 30 : 4.5;
  return `linear-gradient(${kind === 'gala' && !horizontalValue ? 110 : 180}deg, rgba(0,0,0,${base}) ${round(at - feather)}%, black ${round(at)}%, rgba(0,0,0,${base}) ${round(at + feather)}%)`;
}

/** 完整格/树图块的片面成像。低密度底面始终连通，固定乳剂位置不逐帧闪噪。 */
function filmCellMask(p: number): string {
  const tile = 24, base = .48 + .52 * p, contrast = .25 * Math.sin(Math.PI * p);
  const noise = (i: number) => { const n = Math.sin((i + 1) * 127.1) * 43758.5453; return n - Math.floor(n); };
  const dots = Array.from({ length: 36 }, (_, i) => {
    const x = 4 * (i % 6 + .2 + .6 * noise(i * 3)), y = 4 * (Math.floor(i / 6) + .2 + .6 * noise(i * 3 + 1));
    const radius = .6 + 1.2 * noise(i * 3 + 2);
    return [-tile, 0, tile].flatMap(dx => [-tile, 0, tile].map(dy => [x + dx, y + dy]))
      .filter(([cx, cy]) => cx + radius >= 0 && cx - radius <= tile && cy + radius >= 0 && cy - radius <= tile)
      .map(([cx, cy]) => `<circle cx="${round(cx)}" cy="${round(cy)}" r="${round(radius)}" fill="white" opacity="${round(contrast)}"/>`).join('');
  }).join('');
  return `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${tile}" height="${tile}"><rect width="${tile}" height="${tile}" fill="white" opacity="${round(base)}"/>${dots}</svg>`)}")`;
}

/** 整组字从首次出现就完整；细扫描线填实字面，不逐字切开名字或改变数据长宽。 */
function terminalScan(p: number): string {
  const ink = round(4 * (.2 + .8 * p)), base = round(.45 + .55 * clamp((p - .65) / .35));
  return `repeating-linear-gradient(0deg, black 0px, black ${ink}px, rgba(0,0,0,${base}) ${ink}px, rgba(0,0,0,${base}) 4px)`;
}
function posterInk(p: number, axis?: 'x' | 'y'): string {
  const ink = round(4 * (.15 + .85 * p)), base = round(.45 + .55 * clamp((p - .65) / .35));
  return `repeating-linear-gradient(${axis === 'x' ? 0 : axis === 'y' ? 90 : 45}deg, black 0px, black ${ink}px, rgba(0,0,0,${base}) ${ink}px, rgba(0,0,0,${base}) 4px)`;
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
      // 同一整高印压前沿作用于所有色段；连续底墨不把窄柱分裂成两根。
      style.maskImage = `linear-gradient(90deg, black ${pct}%, rgba(0,0,0,${round(.46 + .54 * p)}) ${round(Math.min(100, pct + 3 * (1 - p)))}%)`; break;
    case 'mono': {
      // 连通底墨上的竖排线全高一致；同一版本始终是一根柱，不出现中央断缝。
      const base = round(.76 + .24 * clamp((p - .5) / .5)), width = round(6 * (.12 + .88 * p));
      style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${width}px, rgba(0,0,0,${base}) ${width}px, rgba(0,0,0,${base}) 6px)`;
      break;
    }
    case 'poster': style.opacity = 1; style.maskImage = posterInk(p, 'y'); break;
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
  if (theme.family === 'neon' && cue.lane) {
    // 基线到达真实开始时，完整会话一次接通。短光晕属于原灯管，绝不裁短区间或移动时间坐标。
    const pulse = Math.sin(Math.PI * clamp(p / .65));
    if (p >= .65) return;
    style.filter = `brightness(${round(1 + .65 * pulse)})`;
    style.boxShadow = [style.boxShadow, `0 0 ${round(7 * pulse)}px ${cue.lane}${Math.round(210 * pulse).toString(16).padStart(2, '0')}`].filter(Boolean).join(', ');
    return;
  }
  if (cue.lane && ['editorial', 'mono', 'cinema', 'playbill'].includes(theme.family)) {
    // 每次会话是完整的真实时间区间；只把这枚细印面落到原时轴，端点绝不扫短。
    const rest = 1 - p;
    if (theme.family === 'editorial') {
      style.transform = `translateY(${round(-8 * rest)}px)`;
      style.boxShadow = `inset 0 -${round(2 * rest)}px 0 ${theme.ink}88`;
    } else if (theme.family === 'mono') {
      const base = round(.62 + .38 * p), edge = round(50 * p);
      style.maskImage = `linear-gradient(180deg, black 0%, black ${edge}%, rgba(0,0,0,${base}) ${edge}%, rgba(0,0,0,${base}) ${100 - edge}%, black ${100 - edge}%, black 100%)`;
    } else if (theme.family === 'cinema') {
      const half = 50 * p, soft = 18 * rest, base = round(.5 + .5 * p);
      style.maskImage = `linear-gradient(180deg, rgba(0,0,0,${base}) 0%, rgba(0,0,0,${base}) ${round(Math.max(0,50-half-soft))}%, black ${round(50-half)}%, black ${round(50+half)}%, rgba(0,0,0,${base}) ${round(Math.min(100,50+half+soft))}%, rgba(0,0,0,${base}) 100%)`;
    } else {
      style.transform = `translateY(${round(9 * rest)}px)`;
      style.backgroundImage = `linear-gradient(180deg, transparent ${round(50-20*rest)}%, ${theme.ink}${Math.round(60*rest).toString(16).padStart(2,'0')} 50%, transparent ${round(50+20*rest)}%)`;
    }
    return;
  }
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
    case 'broadsheet': style.maskImage = printAndScreenMask(p, 'broadsheet'); break;
    case 'mono': edgeInk(clamp(p / .96)); style.opacity = alpha * (.2 + .8 * p); break;
    case 'poster': style.opacity = alpha; style.maskImage = posterInk(p, 'x'); break;
    case 'playbill': aperture(); break;
    case 'gala': style.maskImage = printAndScreenMask(p, 'gala', false, true); break;
    case 'terminal': style.opacity = alpha; style.maskImage = terminalScan(p); break;
    case 'crt': {
      // 完整真值底面始终存在，电子刷新只在条内横向行进，不以透明窗截短份额。
      const head = -16 + 132 * p, alpha = Math.round(190 * Math.sin(Math.PI * p)).toString(16).padStart(2, '0');
      style.backgroundImage = [`linear-gradient(90deg, transparent ${round(head - 20)}%, #ffffff${alpha} ${round(head)}%, transparent ${round(head + 4)}%)`, style.backgroundImage].filter(Boolean).join(', ');
      break;
    }
    case 'arcade': style.maskImage = printAndScreenMask(p, 'arcade', false, true); break;
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
  const attr = (name: string) => Number(new RegExp(`\\b${name}="([\\d.]+)"`).exec(all[0])?.[1]);
  const cx = attr('cx'), cy = attr('cy'), radius = attr('r'), width = attr('stroke-width');
  const band = (r: number, w: number, color = 'white') => `<circle cx="${cx}" cy="${cy}" r="${round(r)}" fill="none" stroke="${color}" stroke-width="${round(w)}"/>`;
  const full = `<rect width="${cx * 2}" height="${cy * 2}" fill="white"/>`;
  let defs = '', mask = '', opacity = 1, filter = '';
  switch (theme.family) {
    case 'editorial': {
      // 原环是一张完整印版；压印前沿经过真实弧面，连续底墨保留全部角度。
      const front = 1.16 * p, base = .45 + .55 * p;
      defs = `<linearGradient id="cm-ring-proof" x1="0" y1="0" x2="0" y2="1"><stop offset="${round(clamp(front-.16))}" stop-color="white"/><stop offset="${round(clamp(front))}" stop-color="white" stop-opacity="${round(base)}"/></linearGradient>`;
      mask = full.replace('fill="white"','fill="url(#cm-ring-proof)"'); break;
    }
    case 'playbill': {
      // 对开节目纸的完整环面从折痕压平；折光不把归属弧切成两组开关。
      const fold = .24 * (1-p), shade = .5 + .5 * p;
      defs = `<linearGradient id="cm-ring-fold"><stop offset="${round(.5-fold)}" stop-color="white"/><stop offset=".5" stop-color="white" stop-opacity="${round(shade)}"/><stop offset="${round(.5+fold)}" stop-color="white"/></linearGradient>`;
      mask = full.replace('fill="white"','fill="url(#cm-ring-fold)"'); break;
    }
    case 'minimal':
      mask = theme.dark ? band(radius, width) + band(radius, width * (1 - p), 'black') : band(radius - width * (1 - p) / 2, width * p); break;
    case 'glass':
      // 真实完整弧先透过玻璃可见，再整层对焦；不会与极简共享同一硬厚度裁边。
      defs = `<filter id="cm-ring-focus" x="-5%" y="-5%" width="110%" height="110%"><feGaussianBlur stdDeviation="${round(2.4 * (1 - p) ** 2)}" result="focused"/><feGaussianBlur in="SourceAlpha" stdDeviation=".8" result="relief"/><feSpecularLighting in="relief" surfaceScale="1" specularConstant="${round(.65 * Math.sin(Math.PI * p))}" specularExponent="16" lighting-color="white" result="light"><fePointLight x="${round(cx * 2 * p)}" y="${round(cy - radius * .85)}" z="${round(radius * .5)}"/></feSpecularLighting><feComposite in="light" in2="SourceAlpha" operator="in" result="refraction"/><feComposite in="focused" in2="refraction" operator="arithmetic" k2="1" k3=".6"/></filter>`;
      mask = full; opacity = .55 + .45 * p; filter = ' filter="url(#cm-ring-focus)"'; break;
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
    case 'poster': {
      const size = 6, dot = size * (.12 + .6 * p);
      defs = `<pattern id="cm-ring-poster" patternUnits="userSpaceOnUse" width="${size}" height="${size}"><circle cx="3" cy="3" r="${round(dot)}" fill="white"/></pattern>`;
      mask = full.replace('fill="white"', `fill="white" opacity="${round(.45 + .55 * clamp((p - .65) / .35))}"`) + `<rect width="${cx * 2}" height="${cy * 2}" fill="url(#cm-ring-poster)"/>`;
      break;
    }
  }
  // 连续材质在末段填实整张遮罩，避免边缘重复抗锯齿在撤罩时突然加深。
  if (!['crt', 'arcade'].includes(theme.family) && p > .85) mask += `<g opacity="${round(clamp((p - .85) / .15))}">${full}</g>`;
  if (p <= 0) mask = '';
  const definition = `<defs>${defs}<mask id="cm-ring" maskUnits="userSpaceOnUse" x="0" y="0" width="${cx * 2}" height="${cy * 2}">${mask}</mask></defs>`;
  if (theme.family === 'glass') {
    // 整个玻璃环共用光源；不能把高光随每个 circle 的 -90° 角度变换一起转走。
    let index = 0;
    return svg.replace(/(<svg[^>]*>)/, '$1' + definition).replace(arcs, circle => {
      const first = index === 0, last = ++index === all.length;
      return `${first ? `<g mask="url(#cm-ring)" opacity="${round(opacity)}"${filter}>` : ''}${circle}${last ? '</g>' : ''}`;
    });
  }
  return svg.replace(/(<svg[^>]*>)/, '$1' + definition).replace(arcs, circle => circle.replace('<circle ', `<circle mask="url(#cm-ring)" opacity="${round(opacity)}"${filter} `));
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
      // 完整真值色版先接触，再由细网点压实；底墨保持整条河的连续轮廓。
      extra = `<pattern id="cm-poster-ink" patternUnits="userSpaceOnUse" width="6" height="6"><circle cx="3" cy="3" r="${round(6 * (.12 + .6 * p))}" fill="white"/></pattern>`;
      shape = rect(0, 0, w, h).replace('fill="white"', `fill="white" opacity="${round(.45 + .55 * clamp((p - .65) / .35))}"`) + `<rect width="${w}" height="${h}" fill="url(#cm-poster-ink)"/>`;
      break;
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
      const minimalCell = cue.kind === 'cell' && (cue.plane === 'count' || cue.plane === 'area') && ['minimal-light', 'minimal-dark'].includes(theme.id);
      if (minimalCell) {
        // 相邻文件/百分比格共享同一网格归位，不能各自位移而暂时互相盖住真实面积。
        const shift = 18 * (1 - clamp(time));
        translate(theme.dark ? -shift : 0, theme.dark ? 0 : shift);
        if (theme.dark) style.boxShadow = `inset 0 0 0 ${round(2.4 * (1 - clamp(time)))}px ${theme.ink}88`;
      }
      // 到达终点时连临时样式也移除；避免 inset(0) 裁掉光晕、边框和描边。
      if (p < 1) switch (cue.kind) {
        case 'lane-rule': {
          // 在原有泳道底线上读真实时间；它解释细区间的接通时刻，不另造数据轴或延长会话。
          const head = Math.min(1, p / .72) * 100, end = Math.min(1, (1-p)/.28), color = theme.accent;
          const alpha = Math.round(end * 210).toString(16).padStart(2,'0');
          const left = round(Math.max(0,head-16)), center = round(Math.max(0,head-3));
          if (p > 0) {
            style.backgroundImage = theme.family === 'gala'
              ? `linear-gradient(90deg, transparent ${round(head-24)}%, ${color}${Math.round(end*75).toString(16).padStart(2,'0')} ${round(head-12)}%, ${color}${alpha} ${round(head)}%, transparent ${round(head+24)}%)`
              : theme.family === 'crt'
              ? `linear-gradient(90deg, transparent ${round(head-14)}%, ${color}${Math.round(end*80).toString(16).padStart(2,'0')} ${round(head-3)}%, ${color}${alpha} ${round(head)}%, transparent ${round(head+1)}%)`
              : theme.family === 'cinema'
              ? `linear-gradient(90deg, transparent ${left}%, ${color}${alpha} ${center}%, transparent ${round(Math.min(100,head+3))}%)`
              : `linear-gradient(90deg, transparent ${left}%, ${color}${alpha} ${left}%, ${color}${alpha} ${round(head)}%, transparent ${round(head)}%)`;
            // Satori 的 1px pattern 在缩图时会消失；纹理 tile 加高，实际绘制仍裁在原 1px 底线。
            style.backgroundSize = '100% 4px'; style.backgroundRepeat = 'no-repeat';
          }
          break;
        }
        case 'neon-lane': {
          const head = Math.min(1, p / .8) * 100, alpha = Math.min(1, (1 - p) / .2);
          const color = cue.lane!, left = Math.max(0, head - 10), mid = Math.max(0, head - 2);
          const fade = Math.round(alpha * 255).toString(16).padStart(2, '0');
          const guide = cue.guide!;
          if (p > 0) {
            style.backgroundImage = `linear-gradient(90deg, transparent ${round(left)}%, ${color}${fade} ${round(mid)}%, ${color}${fade} ${round(head)}%, transparent ${round(Math.min(100, head + 1))}%)`;
            style.backgroundPosition = `${guide.x}px ${guide.y}px`;
            style.backgroundSize = `${guide.width}px 2px`;
            style.backgroundRepeat = 'no-repeat';
          }
          break;
        }
        case 'neon-print':
          // 同一条真实引导线抵达时整组字落定，不切开名字或数值。
          style.opacity = p >= .64 ? (style.opacity ?? 1) : 0;
          break;
        case 'neon-guide': {
          const start = round(82 * Math.min(1, p / .72));
          const alpha = p === 0 ? 0 : round(.8 * Math.min(1, (1 - p) / .24));
          const core = `${theme.accent}${Math.round(alpha * 255).toString(16).padStart(2, '0')}`;
          const pulse = `linear-gradient(90deg, transparent ${start}%, rgba(20,105,120,${alpha}) ${round(start + 5)}%, ${core} ${round(start + 9)}%, rgba(20,105,120,${alpha}) ${round(start + 13)}%, transparent ${round(start + 18)}%)`;
          if (p > 0) {
            style.backgroundImage = [pulse, style.backgroundImage].filter(Boolean).join(', ');
            if (cue.guide) {
              let { x = 0, y = 0, width } = cue.guide;
              if (cue.guide.textChild !== undefined) {
                // 热敏票明确交出的点引导字组；在英文化以后量实际文字，不猜它的数值长度。
                const kids = [node.props.children].flat(), text = kids[cue.guide.textChild];
                const content = String(text.props.children), dots = /^\.+/.exec(content)?.[0] ?? '';
                const font = { family: text.props.style.fontFamily }, size = text.props.style.fontSize;
                x = width - measure(content, font, size); width = measure(dots, font, size);
                x = Math.max(0, x); width = Math.min(width, cue.guide.width - x);
                y = kids.some((kid) => /[一-鿿]/.test(String(kid?.props?.children))) ? 22 : 20;
              }
              style.backgroundSize = `${round(width)}px 2px`;
              style.backgroundPosition = `${round(x)}px ${round(y)}px`;
              style.backgroundRepeat = 'no-repeat';
            }
          }
          break;
        }
        case 'bar-x': case 'bar-y':
          // 字符条是完整文字，不能用连续裁窗切出半个方块字。
          if (typeof node.props.children === 'string') {
            style.opacity = p > 0 ? (style.opacity ?? 1) : 0;
            if (theme.family === 'terminal' && p > 0) style.maskImage = terminalScan(p);
          }
          else graphicStyle(style, cue, p, theme);
          break;
        case 'intensity':
          // 强度就是数据，绝不渐变/模糊/缩放；有坐标的 clock 单元按完整行、时段板块或真实墨色版归位。
          style.opacity = p > 0 || (theme.family === 'neon' && cue.level === 0) ? (style.opacity ?? 1) : 0;
          if (cue.row !== undefined && cue.column !== undefined && p > 0) {
            const rest = 1 - p;
            if (theme.family === 'film') translate(-4 * rest, 0);
            else if (theme.family === 'editorial') translate(18 * rest, 0);
            else if (theme.family === 'glass') translate(0, -4 * rest);
            else if (theme.family === 'gala') translate((cue.column < (cue.columns ?? 24) / 2 ? 16 : 36) * rest, 0);
            else if (theme.family === 'broadsheet') translate(24 * rest, 0);
            else if (theme.family === 'crt') translate(32 * rest * rest, 0);
            else if (theme.family === 'mono') translate(0, -2 * rest);
            else if (theme.family === 'poster') translate(0, -5 * rest);
            else if (theme.family === 'cinema') translate(0, -22 * rest * rest);
            else if (theme.family === 'playbill') translate(0, (cue.column < (cue.columns ?? 24) / 2 ? -1 : 1) * 16 * rest);
            else if (theme.family === 'arcade') translate(Math.ceil(rest * 3) * 8, 0);
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
        // 原版已有 -6° 的静态开口；额外开合须留在画布上沿内。
        case 'clap': style.transform = [style.transform, `rotate(${round(-10 * (1 - p))}deg)`].filter(Boolean).join(' '); break;
        case 'cell': {
          const rest = 1 - p;
          if (minimalCell) { style.opacity = p > 0 ? (node.props.style?.opacity ?? 1) : 0; break; }
          if (cue.plane === 'time' && ['minimal-light', 'minimal-dark'].includes(theme.id)) {
            // 日列的横坐标与宽度表示实际日期，不能像拼图那样移动或缩窄。
            style.opacity = p > 0 ? (node.props.style?.opacity ?? 1) : 0;
            if (!theme.dark) style.clipPath = `inset(${round(100 * rest)}% 0 0 0)`;
            else if (p > 0) {
              // 原色日列先完整接通，接触光只经过它自己的表面和内缘。
              const head = -16 + 132 * p, glow = Math.round(112 * Math.sin(Math.PI * p)).toString(16).padStart(2, '0');
              const light = `linear-gradient(180deg, transparent ${round(100 * clamp((head - 14) / 100))}%, ${theme.ink}${glow} ${round(100 * clamp(head / 100))}%, transparent ${round(100 * clamp((head + 14) / 100))}%)`;
              style.backgroundImage = [light, style.backgroundImage].filter(Boolean).join(', ');
              style.boxShadow = `inset 0 0 0 ${round(.8 * rest)}px ${theme.ink}44`;
            }
            break;
          }
          style.opacity = p;
          if (theme.family === 'arcade') { style.opacity = p > 0 ? 1 : 0; style.maskImage = printAndScreenMask(p, 'arcade'); }
          else if (theme.family === 'glass') { style.opacity = 0.12 + 0.88 * p; translate(0, -6 * rest); }
          else if (theme.family === 'poster') { style.opacity = p > 0 ? 1 : 0; style.maskImage = posterInk(p); }
          else if (theme.family === 'playbill') { style.opacity = p > 0 ? 1 : 0; translate(0, 5 * rest); style.boxShadow = `0 ${round(2 * rest)}px 0 ${theme.ink}22`; }
          else if (theme.family === 'gala') { style.opacity = p > 0 ? 1 : 0; style.maskImage = printAndScreenMask(p, 'gala'); }
          else if (theme.family === 'editorial') { style.opacity = p > 0 ? 1 : 0; translate(-4 * rest, 0); style.boxShadow = `inset 0 -${round(1.2 * rest)}px 0 ${theme.accent}`; }
          else if (theme.family === 'broadsheet') { style.opacity = p > 0 ? 1 : 0; style.maskImage = printAndScreenMask(p, 'broadsheet'); }
          else if (theme.family === 'mono') { style.opacity = p > 0 ? 1 : 0; const base = round(.76 + .24 * clamp((p - .5) / .5)); style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${round(p * 4)}px, rgba(0,0,0,${base}) ${round(p * 4)}px, rgba(0,0,0,${base}) 4px)`; }
          else if (theme.family === 'neon') { style.opacity = 0.12 + 0.88 * p; style.boxShadow = `inset 0 0 0 1px ${theme.accent}`; }
          else if (theme.family === 'crt') { style.opacity = p > 0 ? 1 : 0; style.maskImage = printAndScreenMask(p, 'crt'); }
          else if (theme.family === 'terminal') { style.opacity = p > 0 ? 1 : 0; style.maskImage = terminalScan(p); }
          else if (theme.family === 'film') {
            style.opacity = clamp(p / .3);
            style.maskImage = filmCellMask(p);
            // Satori 的 CSS mask 固定于原位；数据块不能与它一起错位裁掉边缘。
          }
          else if (theme.family === 'cinema') { style.opacity = clamp(p / .2); translate(0, 8 * rest); style.filter = `blur(${round(1.8 * rest * rest)}px)`; }
          else if (theme.dark) style.filter = `brightness(${round(.65 + .35 * p)})`;
          break;
        }
        default: {
          entryStyle(style, cue, p, theme);
          if (theme.family === 'terminal' && p > 0 && p < .9 && node.motionCursor?.length) {
            const rows = node.motionCursor;
            const cursor = rows[Math.min(rows.length - 1, Math.floor(p / .9 * rows.length))];
            const alpha = Math.floor(p * 12) % 3 === 2 ? '59' : 'ff';
            style.backgroundImage = `linear-gradient(${theme.accent}${alpha}, ${theme.accent}${alpha})`;
            style.backgroundSize = `${round(cursor.w)}px ${round(cursor.h)}px`;
            style.backgroundPosition = `${round(cursor.x)}px ${round(cursor.y)}px`;
            style.backgroundRepeat = 'no-repeat';
          }
          break;
        }
      }
      node.props = { ...node.props, style };
    }
    const kids = node.props.children;
    const childClock = cue && cue.kind !== 'neon-guide' ? phase(clock, cue, theme) : clock;
    for (const kid of Array.isArray(kids) ? kids : [kids]) visit(kid, childClock);
  };
  visit(tree, clamp(time));
}
