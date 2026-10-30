// 作品自己标出可运动的内容组；这里只负责时间采样，不猜 CSS 尺寸或拆散名字/数值。
import type { Node, Out } from './ui.ts';
import type { Theme } from './themes.ts';

export type MotionKind = 'row' | 'title' | 'bar-x' | 'bar-y' | 'cell' | 'intensity' | 'area' | 'radial' | 'print' | 'sheet' | 'signature' | 'card' | 'credits' | 'clap';
export interface MotionCue { kind: MotionKind; order?: number; total?: number; travel?: number; grid?: number; columns?: number }

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
  const count = cue.total ?? 1, order = cue.order ?? 0;
  const span = ['cell', 'intensity'].includes(cue.kind) ? 0.18 : cue.kind === 'print' ? 0.22 : cinematic(theme) ? 0.64 : 0.56;
  // 动作顺序属于材质：剧场两侧、颁奖礼中心和街机交错格，不能只换缓动曲线。
  let rank = count > 1 ? order / (count - 1) : 0;
  if (count > 1 && Number.isInteger(order) && Number.isInteger(count)) {
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

/** 非文字图形的成形方式；每个家族用自身的印刷、光学或像素机制。 */
function graphicStyle(style: Record<string, any>, cue: MotionCue, p: number, theme: Theme) {
  const rest = round((1 - p) * 100), pct = round(p * 100), vertical = cue.kind === 'bar-y';
  const forward = () => { style.clipPath = vertical ? `inset(${rest}% 0 0 0)` : `inset(0 ${rest}% 0 0)`; };
  const aperture = () => { style.clipPath = vertical ? `inset(${rest}% 0 0 0)` : `inset(0 ${rest / 2}% 0 ${rest / 2}%)`; };
  switch (theme.family) {
    case 'minimal': if (theme.dark) { aperture(); style.opacity = 0.3 + 0.7 * p; } else forward(); break;
    case 'glass':
      style.maskImage = `linear-gradient(${vertical ? 0 : 110}deg, black 0%, black ${Math.max(0, pct - 16)}%, transparent ${pct}%)`;
      style.filter = `brightness(${round(1 + (1 - p) * 0.4)})`;
      break;
    case 'film': style.clipPath = `inset(${rest / 2}% 0 ${rest / 2}% 0)`; style.opacity = p; break;
    case 'cinema': aperture(); style.filter = `brightness(${round(0.65 + p * 0.35)})`; break;
    case 'editorial': forward(); style.transform = [style.transform, `translate(0px, ${round((1 - p) * 3)}px)`].filter(Boolean).join(' '); break;
    case 'broadsheet': style.maskImage = `repeating-linear-gradient(0deg, black 0px, black ${round(p * 5)}px, transparent ${round(p * 5)}px, transparent 5px)`; break;
    case 'mono':
      style.opacity = 0.2 + 0.8 * p;
      style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${round(p * 6)}px, transparent ${round(p * 6)}px, transparent 6px)`;
      break;
    case 'poster': style.opacity = p > 0 ? 1 : 0; style.transform = [style.transform, `translate(${round((1 - p) * 2)}px, ${round(-(1 - p) * 5)}px)`].filter(Boolean).join(' '); break;
    case 'playbill': aperture(); break;
    case 'gala':
      style.clipPath = `polygon(50% ${50 - pct}%, ${50 + pct}% 50%, 50% ${50 + pct}%, ${50 - pct}% 50%)`;
      style.filter = `brightness(${round(1 + Math.sin(Math.PI * p) * 0.45)})`; break;
    case 'terminal': forward(); break;
    case 'crt':
      style.maskImage = `linear-gradient(${vertical ? 0 : 90}deg, black 0%, black ${Math.max(0, pct - 7)}%, rgba(0,0,0,.35) ${pct}%, transparent ${Math.min(100, pct + 4)}%)`;
      style.filter = `brightness(${round(1.25 - p * 0.25)})`; break;
    case 'arcade':
      style.clipPath = vertical ? `inset(${round((1 - Math.floor(p * 8) / 8) * 100)}% 0 0 0)` : `inset(0 ${round((1 - Math.floor(p * 8) / 8) * 100)}% 0 0)`;
      break;
    case 'neon':
      style.opacity = 0.18 + 0.82 * p;
      style.filter = `brightness(${round(0.55 + p * 0.45)})`;
      if (typeof style.backgroundColor === 'string' && /^#[0-9a-f]{6}$/i.test(style.backgroundColor)) {
        const color = style.backgroundColor;
        style.backgroundColor = color + Math.round(clamp((p - 0.38) / 0.62) * 255).toString(16).padStart(2, '0');
        style.boxShadow = `inset 0 0 0 1px ${color}`;
      }
      break;
  }
  // 柱形始终立在数据基线上。材质改变表面如何出现，不把柱子悬在中央或移离坐标。
  if (vertical) {
    forward();
    if (theme.family === 'editorial' || theme.family === 'poster') delete style.transform;
    if (cue.grid && cue.travel) style.clipPath = `inset(${round(Number(style.height) - Math.floor(p * cue.travel / cue.grid) * cue.grid)}px 0 0 0)`;
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
    const size = Number(/width="([\d.]+)"/.exec(svg)?.[1] ?? 420), unit = 12;
    // 字符环按整格打印；扇形裁窗会把扫过边缘的方块切成三角形。
    return svg.replace(/<rect x="([\d.-]+)" y="([\d.-]+)" width="([\d.-]+)" height="([\d.-]+)" fill="url\(#(tg\d+)\)"\/>/g, (_, x, y, w, h, fill) => {
      const cells: string[] = [];
      for (let left = Number(x); left < Number(x) + Number(w); left += unit) {
        const angle = (Math.atan2(Number(y) + unit / 2 - size / 2, left + unit / 2 - size / 2) / (2 * Math.PI) + 1.25) % 1;
        if (angle <= p) cells.push(`<rect x="${left}" y="${y}" width="${unit}" height="${h}" fill="url(#${fill})"/>`);
      }
      return cells.join('');
    });
  }
  if (['glass', 'mono', 'broadsheet'].includes(theme.family)) {
    // 光学表面沿环的厚度凝结；黑白版先显内外轮廓；报刊按网线压印。
    // 数据弧的半径、角度和起点始终不动，mask 只露出其现有墨迹。
    const circle = /<circle\b[^>]*stroke-dasharray="[^"]+"[^>]*\/>/.exec(svg)?.[0];
    if (!circle) return svg;
    const attr = (name: string) => Number(new RegExp(`\\b${name}="([\\d.]+)"`).exec(circle)?.[1]);
    const cx = attr('cx'), cy = attr('cy'), radius = attr('r'), width = attr('stroke-width');
    let defs = '', mask = '';
    if (theme.family === 'glass') mask = `<circle cx="${cx}" cy="${cy}" r="${round(radius + width * (1 - p) / 2)}" fill="none" stroke="white" stroke-width="${round(width * p)}"/>`;
    else if (theme.family === 'mono') mask = `<circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="white" stroke-width="${width}"/><circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="black" stroke-width="${round(width * (1 - p))}"/>`;
    else {
      defs = `<pattern id="cm-ring-print" patternUnits="userSpaceOnUse" width="6" height="6"><rect width="6" height="${round(p * 6)}" fill="white"/></pattern>`;
      mask = `<rect width="${cx * 2}" height="${cy * 2}" fill="url(#cm-ring-print)"/>`;
    }
    const definition = `<defs>${defs}<mask id="cm-ring" maskUnits="userSpaceOnUse" x="0" y="0" width="${cx * 2}" height="${cy * 2}">${mask}</mask></defs>`;
    return svg.replace(/(<svg[^>]*>)/, '$1' + definition).replace(/<circle\b[^>]*stroke-dasharray="[^"]+"[^>]*\/>/g, c => c.replace('<circle ', '<circle mask="url(#cm-ring)" '));
  }
  // 每条弧保留自己的圆心、半径和粗细；从弧段本身成形，不用矩形窗口切圆环。
  const count = (svg.match(/stroke-dasharray=/g) ?? []).length / (theme.family === 'neon' ? 2 : 1);
  let i = 0;
  return svg.replace(/<circle\b[^>]*stroke-dasharray="([\d.]+) ([\d.]+)"[^>]*\/>/g, (circle, length, circumference) => {
    const index = i++ % count, span = Number(length), offset = Number(/stroke-dashoffset="([\d.-]+)"/.exec(circle)?.[1] ?? 0);
    const order = index / Math.max(1, count - 1);
    let local = clamp(p * 1.5 - order * 0.5), opacity = 1, center = false;
    switch (theme.family) {
      case 'minimal': center = theme.dark; if (theme.dark) local = p; break;
      case 'film': {
        // 每段胶片从两端的切口向内曝光，不让整根弧仅变透明度。
        const end = span * local / 2;
        return circle.replace(/stroke-dasharray="[^"]+"/, `stroke-dasharray="${round(end)} ${round(span - 2 * end)} ${round(end)} ${circumference}"`);
      }
      case 'cinema': center = true; break;
      case 'editorial': local = clamp(p * count - index); break;
      case 'poster': opacity = local > 0 ? 1 : 0; local = 1; break;
      case 'playbill': center = true; local = clamp(p * 1.5 - Math.abs(order - 0.5)); break;
      case 'gala': center = true; local = p; opacity = Math.min(1, p * 2); break;
      case 'crt': local = Math.floor(local * 24) / 24; opacity = 0.5 + 0.5 * p; break;
      case 'arcade': local = Math.floor(local * 8) / 8; break;
      case 'neon': break;
    }
    return circle.replace(/stroke-dasharray="[^"]+"/, `stroke-dasharray="${round(span * local)} ${circumference}"`)
      .replace(/stroke-dashoffset="[^"]+"/, `stroke-dashoffset="${round(offset - (center ? span * (1 - local) / 2 : 0))}"`)
      .replace('<circle ', `<circle opacity="${round(opacity)}" `);
  });
}

/** 面积图的轴和网格不动；只给明确的面积层加印刷、光学或格子成形。 */
function areaMotion(svg: string, w: number, h: number, p: number, theme: Theme, grid?: number): string {
  if (grid) return svg.replace(/<rect x="([\d.-]+)" y="([\d.-]+)" width="([\d.-]+)" height="([\d.-]+)" fill="url\(#(tg\d+)\)"\/>/g, (_, x, y, rw, rh, fill) => {
    const top = h - Math.round((h - Number(y)) * p / grid) * grid;
    const bottom = h - Math.round((h - Number(y) - Number(rh)) * p / grid) * grid;
    return `<rect x="${x}" y="${top}" width="${rw}" height="${bottom - top}" fill="url(#${fill})"/>`;
  });
  const rest = 1 - p, rect = (x: number, y: number, rw: number, rh: number) => `<rect x="${round(x)}" y="${round(y)}" width="${round(rw)}" height="${round(rh)}" fill="white"/>`;
  let shape = rect(0, 0, w * p, h), extra = '', attributes = '';
  switch (theme.family) {
    case 'minimal': if (theme.dark) { shape = rect(w * rest / 2, 0, w * p, h); attributes = ` opacity="${round(0.3 + 0.7 * p)}"`; } break;
    case 'glass':
      extra = `<linearGradient id="cm-light" x1="0" y1="0" x2="1" y2=".25"><stop offset="${Math.max(0, p - 0.14)}" stop-color="white"/><stop offset="${p}" stop-color="black"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-light)"/>`; break;
    case 'film': shape = rect(0, h * rest / 2, w, h * p); attributes = ` opacity="${round(p)}"`; break;
    case 'cinema': shape = rect(w * rest / 2, 0, w * p, h); break;
    case 'editorial': {
      // 杂志按版面栏依次排入：完整曲线留在自己的栏位，不拉伸数据。
      const columns = 12, width = w / columns;
      shape = Array.from({ length: columns }, (_, i) => rect(i * width, 0, width, h * clamp(p * columns - i))).join(''); break;
    }
    case 'broadsheet':
      extra = `<pattern id="cm-ink" patternUnits="userSpaceOnUse" width="1" height="6">${rect(0, 0, 1, p * 6)}</pattern>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-ink)"/>`; break;
    case 'mono':
      extra = `<pattern id="cm-ink" patternUnits="userSpaceOnUse" width="8" height="1">${rect(0, 0, p * 8, 1)}</pattern>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-ink)"/>`; break;
    case 'poster': {
      // 每种墨色一版压上，面积与折线的最终位置从第一帧起就固定。
      let index = 0;
      const count = (svg.match(/<path /g) ?? []).length;
      return svg.replace(/<path /g, () => `<path opacity="${round(clamp((p * (count + 1) - index++) / 2))}" `);
    }
    case 'playbill': shape = rect(0, 0, w * p / 2, h) + rect(w * (1 - p / 2), 0, w * p / 2, h); break;
    case 'gala': shape = `<polygon points="${w / 2},${h / 2 - h * p} ${w / 2 + w * p},${h / 2} ${w / 2},${h / 2 + h * p} ${w / 2 - w * p},${h / 2}" fill="white"/>`; break;
    case 'crt':
      extra = `<linearGradient id="cm-light"><stop offset="${Math.max(0, p - 0.04)}" stop-color="white"/><stop offset="${p}" stop-color="#aaa"/><stop offset="${Math.min(1, p + 0.02)}" stop-color="black"/></linearGradient>`;
      shape = `<rect width="${w}" height="${h}" fill="url(#cm-light)"/>`; break;
    case 'arcade': {
      const unit = 16, columns = Math.ceil(w / unit), rows = Math.ceil(h / unit);
      // 逐列落格，格子是裁窗而非缩放，因此不会压扁图像。
      shape = Array.from({ length: columns }, (_, x) => rect(x * unit, h - Math.floor(clamp(p * 1.3 - x / columns * 0.3) * rows) * unit, unit, h)).join(''); break;
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
          // 透明度就是数据；扫描到一个格子时，按最终强度整格显示。
          style.opacity = p > 0 ? (style.opacity ?? 1) : 0;
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
          else if (theme.family === 'glass') { style.opacity = 0.12 + 0.88 * p; style.transform = [style.transform, `scale(${round(0.7 + 0.3 * p)})`].filter(Boolean).join(' '); }
          else if (theme.family === 'poster') { style.opacity = p > 0 ? 1 : 0; translate((cue.order ?? 0) % 2 ? 4 * rest : -4 * rest, -5 * rest); }
          else if (theme.family === 'playbill') translate(((cue.order ?? 0) % 2 ? 1 : -1) * 14 * rest, 0);
          else if (theme.family === 'gala') style.transform = [style.transform, `rotate(${round(45 * rest)}deg) scale(${round(0.65 + 0.35 * p)})`].filter(Boolean).join(' ');
          else if (theme.family === 'editorial') translate(-10 * rest, 0);
          else if (theme.family === 'broadsheet') { style.opacity = p > 0.15 ? 1 : p / 0.15; translate(0, -2 * rest); }
          else if (theme.family === 'mono') style.maskImage = `repeating-linear-gradient(90deg, black 0px, black ${round(p * 4)}px, transparent ${round(p * 4)}px, transparent 4px)`;
          else if (theme.family === 'neon') { style.opacity = 0.12 + 0.88 * p; style.boxShadow = `inset 0 0 0 1px ${theme.accent}`; }
          else if (theme.family === 'crt') style.opacity = clamp(p * 2);
          else if (theme.family === 'terminal') style.opacity = p > 0 ? 1 : 0;
          else if (theme.family === 'film') translate(0, 3 * rest);
          else if (theme.family === 'cinema') translate(0, 12 * rest);
          else if (theme.dark) style.transform = [style.transform, `scale(${round(0.92 + 0.08 * p)})`].filter(Boolean).join(' ');
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
