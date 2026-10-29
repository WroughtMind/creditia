// creditia 的标志：一个五瓣的星号 *。排版里 * 就是「出处在这儿」，也是各家 AI 标志共同的形。
// 每瓣一个工具色；给了份额，瓣的长短就是各家写了多少，每个项目长出自己的 *。
// App、官网、README、App 图标和作品角落的小章都从这里画。

/** 标准星号的五瓣：Claude Code、Codex、Cursor、Kimi、Grok，从正上方顺时针 */
export const MARK_COLORS = ['#D97757', '#10A37F', '#4C8DF6', '#8C7CFF', '#E8B730'];
const TOOL_COLOR: Record<string, string> = { 'claude-code': '#D97757', codex: '#10A37F', cursor: '#4C8DF6', kimi: '#8C7CFF', grok: '#E8B730' };
/** 未确认和其余工具 */
export const MARK_GREY = '#8A8478';

export interface Petal {
  color: string; /** 相对长度 0–1，默认 1 */ len?: number; /** 0–1，默认 1 */ opacity?: number;
  /** 这一瓣的方向，按圈算（0 是正上方，顺时针）；默认按瓣数等分。App 里两个项目的 * 互相形变时用 */
  at?: number;
}

const f = (n: number) => +n.toFixed(3);

/** 一瓣：尖从中心附近伸出，两条切线收进末端那颗圆 */
function petal(cx: number, cy: number, a: number, len: number, rb: number, gap: number): string {
  const ux = Math.cos(a), uy = Math.sin(a), px = -uy, py = ux;
  const ox = cx + ux * gap, oy = cy + uy * gap, d = len - gap - rb;
  const bx = ox + ux * d, by = oy + uy * d;
  const th = Math.PI / 2 + Math.asin(Math.min(0.99, rb / d)), pts: number[][] = [[ox, oy]];
  for (let k = 0; k <= 24; k++) { const t = th - (2 * th * k) / 24; pts.push([bx + rb * (Math.cos(t) * ux + Math.sin(t) * px), by + rb * (Math.cos(t) * uy + Math.sin(t) * py)]); }
  return 'M' + pts.map(([x, y]) => `${f(x)} ${f(y)}`).join('L') + 'Z';
}

export interface MarkOpts {
  /** 瓣，默认标准五色 */
  petals?: Petal[];
  /** 单色：每瓣都用 ink，瓣在中心连成一个实心的 * */
  ink?: string;
  cx?: number; cy?: number;
  /** 最长一瓣的长度（24 的画布上） */
  R?: number;
  /** 小尺寸（20px 上下）的光学修正：中心空隙小一点、瓣头圆一点，缩小了不糊 */
  optical?: boolean;
}

/** 只要路径，放进别人的 <svg> / <g> 里 */
export function markPaths(o: MarkOpts = {}): string {
  const { cx = 12, cy = 12, R = 10.7, ink, optical } = o;
  const petals: Petal[] = o.petals ?? MARK_COLORS.map((color) => ({ color }));
  const n = petals.length, gap = ink ? 0 : R * (optical ? 0.035 : 0.07), head = optical ? 0.25 : 0.224;
  // 每瓣占自己的扇区；工具变多时收窄圆头，不合并工具或让相邻瓣盖住彼此。
  const spread = Math.sin(Math.PI / Math.max(2, n));
  return petals.map((p, i) => {
    const len = R * (p.len ?? 1);
    if (len <= gap || p.opacity === 0) return '';
    // 生长动画中圆头也要跟着缩小，让尖端始终处于圆外，切线才有实数解。
    const rb = Math.min(R * head * Math.sqrt(p.len ?? 1), (len - gap) * spread / (1 + spread));
    const op = p.opacity != null && p.opacity < 1 ? ` fill-opacity="${+p.opacity.toFixed(3)}"` : '';
    const a = -Math.PI / 2 + 2 * Math.PI * (p.at ?? i / n);
    return `<path d="${petal(cx, cy, a, len, rb, gap)}" fill="${ink ?? p.color}"${op}/>`;
  }).join('');
}

export function markSvg(o: MarkOpts & { size?: number } = {}): string {
  const s = o.size ? ` width="${o.size}" height="${o.size}"` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"${s}>${markPaths(o)}</svg>`;
}

export const toolColor = (tool: string) => TOOL_COLOR[tool] ?? MARK_GREY;

/**
 * 一个项目自己的 *：各工具按行数排，最多的在正上方，顺时针递减；
 * 每个有贡献的工具都保留一瓣；瓣长按相对行数开方，再留出短瓣的可见长度。
 * 不到五个工具时补几瓣淡灰的短瓣，形还是一个 *。
 * 作品里用这个风格自己的颜色（color），App 里用工具色。
 */
export function sharePetals(tools: { tool: string; lines: number; color?: string }[], grey = MARK_GREY): Petal[] {
  const by = new Map<string, { lines: number; color?: string }>();
  for (const t of tools) if (t.lines > 0) { const e = by.get(t.tool); by.set(t.tool, { lines: (e?.lines ?? 0) + t.lines, color: e?.color ?? t.color }); }
  const list = [...by].sort((a, b) => b[1].lines - a[1].lines);
  const max = list[0]?.[1].lines ?? 0;
  const out: Petal[] = list.map(([k, v]) => ({ color: v.color ?? toolColor(k), len: 0.46 + 0.54 * Math.sqrt(v.lines / max) }));
  while (out.length < 5) out.push({ color: grey, len: 0.42, opacity: 0.35 });
  return out;
}
