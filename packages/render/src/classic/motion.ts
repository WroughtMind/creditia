// 作品自己标出可运动的内容组；这里只负责时间采样，不猜 CSS 尺寸或拆散名字/数值。
import type { Node, Out } from './ui.ts';
import type { Theme } from './themes.ts';

export type MotionKind = 'row' | 'title' | 'bar-x' | 'bar-y' | 'cell' | 'scan-x' | 'radial' | 'print' | 'paper' | 'credits' | 'clap';
export interface MotionCue { kind: MotionKind; order?: number; total?: number; travel?: number }

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
  const span = cue.kind === 'cell' ? 0.18 : cue.kind === 'print' ? 0.22 : cinematic(theme) ? 0.64 : 0.56;
  const start = count > 1 ? order / (count - 1) * (1 - span) : 0;
  return clamp((time - start) / span);
}
export function progress(time: number, cue: MotionCue, theme: Theme): number {
  if (time >= 1) return 1;
  const p = phase(time, cue, theme);
  if (cue.kind === 'print' || (digital(theme) && ['row', 'cell', 'title'].includes(cue.kind))) return p > 0 ? 1 : 0;
  const eased = ink(theme) ? p : cinematic(theme) ? p * p * (3 - 2 * p) : 1 - (1 - p) ** 3;
  return digital(theme) ? Math.floor(eased * 16) / 16 : eased;
}

/** 圆环的扇形窗口从十二点顺时针扫过；使用百分比，不改变环的最终几何。 */
function radial(p: number): string {
  const points = ['50% 50%', '50% -25%'];
  const segments = Math.max(1, Math.ceil(p * 64));
  for (let i = 1; i <= segments; i++) {
    const a = -Math.PI / 2 + p * Math.PI * 2 * i / segments;
    points.push(`${round(50 + Math.cos(a) * 75)}% ${round(50 + Math.sin(a) * 75)}%`);
  }
  return `polygon(${points.join(',')})`;
}

/** 对已排版的克隆树采样。只改绘制属性；宽高、margin、gap 和文本不参与动画。 */
export function animate(tree: Node, time: number, theme: Theme): void {
  if (time >= 1) return;
  const visit = (node: Node, clock: number) => {
    if (!node || typeof node !== 'object' || !node.props) return;
    const cue: MotionCue | undefined = node.motionCue;
    if (cue) {
      const p = progress(clock, cue, theme), rest = round((1 - p) * 100);
      const style = { ...node.props.style };
      const translate = (x: number, y: number) => { style.transform = [style.transform, `translate(${round(x)}px, ${round(y)}px)`].filter(Boolean).join(' '); };
      // 到达终点时连临时样式也移除；避免 inset(0) 裁掉光晕、边框和描边。
      if (p < 1) switch (cue.kind) {
        case 'bar-x': case 'scan-x': style.clipPath = `inset(0 ${rest}% 0 0)`; break;
        case 'bar-y': style.clipPath = `inset(${rest}% 0 0 0)`; break;
        case 'paper': style.clipPath = `inset(0 0 ${rest}% 0)`; break;
        case 'radial': style.clipPath = radial(p); break;
        case 'print': style.opacity = (style.opacity ?? 1) * p; break;
        case 'clap': style.transform = [style.transform, `rotate(${round(-18 * (1 - p))}deg)`].filter(Boolean).join(' '); break;
        case 'cell':
          style.opacity = (style.opacity ?? 1) * p;
          if (!digital(theme) && !ink(theme)) style.transform = [style.transform, `scale(${round(0.82 + 0.18 * p)})`].filter(Boolean).join(' ');
          break;
        case 'credits':
          style.opacity = (style.opacity ?? 1) * p;
          if (!digital(theme) && !ink(theme)) translate(0, (cue.travel ?? 30) * (1 - p));
          break;
        case 'row': case 'title':
          if (ink(theme)) style.clipPath = `inset(0 ${rest}% 0 0)`;
          else {
            style.opacity = (style.opacity ?? 1) * p;
            if (!digital(theme) && theme.family !== 'neon') translate(0, (cinematic(theme) ? 18 : 10) * (1 - p));
          }
          break;
      }
      node.props = { ...node.props, style };
    }
    const kids = node.props.children;
    const childClock = cue ? phase(clock, cue, theme) : clock;
    for (const kid of Array.isArray(kids) ? kids : [kids]) visit(kid, childClock);
  };
  visit(tree, clamp(time));
}
