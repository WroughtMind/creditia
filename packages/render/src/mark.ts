// creditia 的标志：一个五瓣的星号 *。排版里 * 就是「出处在这儿」，也是各家 AI 标志共同的形。
// 每瓣一个工具色；给了份额，瓣的长短就是各家写了多少，每个项目长出自己的 *。
// App、官网、README、App 图标和作品角落的小章都从这里画。

/** 标准星号的五瓣：Claude Code、Codex、Cursor、Kimi、Grok，从正上方顺时针 */
export const MARK_COLORS = ['#D97757', '#10A37F', '#4C8DF6', '#8C7CFF', '#E8B730'];
const TOOL_COLOR: Record<string, string> = { 'claude-code': '#D97757', codex: '#10A37F', cursor: '#4C8DF6', kimi: '#8C7CFF', grok: '#E8B730' };
/** 未确认和其余工具 */
export const MARK_GREY = '#8A8478';

export interface Petal { color: string; /** 相对长度 0–1，默认 1 */ len?: number; /** 0–1，默认 1 */ opacity?: number }

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
}

/** 只要路径，放进别人的 <svg> / <g> 里 */
export function markPaths(o: MarkOpts = {}): string {
  const { cx = 12, cy = 12, R = 10.7, ink } = o;
  const petals = o.petals ?? MARK_COLORS.map((color) => ({ color }));
  const n = petals.length, gap = ink ? 0 : R * 0.07;
  return petals.map((p, i) => {
    const len = R * (p.len ?? 1), rb = R * 0.224 * Math.sqrt(p.len ?? 1);
    const op = p.opacity != null && p.opacity < 1 ? ` fill-opacity="${p.opacity}"` : '';
    return `<path d="${petal(cx, cy, -Math.PI / 2 + (i * 2 * Math.PI) / n, len, rb, gap)}" fill="${ink ?? p.color}"${op}/>`;
  }).join('');
}

export function markSvg(o: MarkOpts & { size?: number } = {}): string {
  const s = o.size ? ` width="${o.size}" height="${o.size}"` : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"${s}>${markPaths(o)}</svg>`;
}

export const toolColor = (tool: string) => TOOL_COLOR[tool] ?? MARK_GREY;

/**
 * 一个项目自己的 *：各工具按行数排，最多的在正上方，顺时针递减；
 * 瓣长按份额开方（小份额也看得见）。不到五个工具时补几瓣淡灰的短瓣，形还是一个 *。
 * 作品里用这个风格自己的颜色（color），App 里用工具色。
 */
export function sharePetals(tools: { tool: string; lines: number; color?: string }[], grey = MARK_GREY): Petal[] {
  const by = new Map<string, { lines: number; color?: string }>();
  for (const t of tools) if (t.lines > 0) { const e = by.get(t.tool); by.set(t.tool, { lines: (e?.lines ?? 0) + t.lines, color: e?.color ?? t.color }); }
  let list: [string, { lines: number; color?: string }][] = [...by].sort((a, b) => b[1].lines - a[1].lines);
  if (list.length > 6) list = [...list.slice(0, 5), ['', { lines: list.slice(5).reduce((s, x) => s + x[1].lines, 0), color: grey }]];
  const max = list[0]?.[1].lines ?? 0;
  const out: Petal[] = list.map(([k, v]) => ({ color: v.color ?? (k ? toolColor(k) : grey), len: 0.46 + 0.54 * Math.sqrt(v.lines / max) }));
  while (out.length < 5) out.push({ color: grey, len: 0.42, opacity: 0.35 });
  return out;
}
