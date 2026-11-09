// 只改变画面怎样展开；数据、比例和最终排版始终由作品本身决定。
import { attrs, el, rect, r, type Attrs, type Scene } from './svg.ts';
import type { Box, Style } from './style.ts';

export const phase = (t: number, start = 0, end = 1): number =>
  Math.min(1, Math.max(0, (t - start) / Math.max(Number.EPSILON, end - start)));

/** 同一动作遵循材质的节奏；旧主题桥接保留原 id，不带 classic. 前缀。 */
export function styleProgress(st: Style, t: number, start = 0, end = 1): number {
  const k = phase(t, start, end);
  if (k === 0 || k === 1) return k;
  if (st.grid || ['terminal', 'crt', 'arcade'].includes(st.id)) return Math.floor(k * 16) / 16;
  if (['woodblock', 'ukiyoe', 'manuscript', 'cutout', 'poster'].includes(st.id)) return Math.floor(k * 12) / 12;
  if (['ink', 'meadow', 'cinema', 'film', 'baroque', 'gala', 'grimoire'].includes(st.id)) return k * k * (3 - 2 * k);
  if (['cyber', 'neon'].includes(st.id)) return 1 - (1 - k) * (1 - k);
  return k;
}

/** 一次展开的表现时长，单位毫秒；银幕滚动优先使用作品自己的 motion.secs。 */
export function motionDuration(id: string, variant?: string, style?: string): number {
  const duration = id === 'roll' ? 2400
    : id === 'receipt' ? variant === 'itemized' ? 2200 : 1900
    : ['versions', 'calendar', 'clock'].includes(id) ? 2200
    : ['stream', 'survival', 'areas', 'area', 'lanes', 'settle', 'evidence'].includes(id) ? 2000
    : id === 'asterisk' ? 1700
    : ['badge', 'byline'].includes(id) ? 1300 : 1600;
  return Math.round(duration * (style && ['film', 'cinema', 'playbill', 'gala', 'ink', 'meadow'].includes(style) ? 1.15 : 1));
}

export function drawGroup(s: Scene, a: Attrs, draw: () => void): void {
  s.add(`<g${attrs(a)}>`);
  try { draw(); } finally { s.add('</g>'); }
}

const clips = new WeakMap<Scene, number>();
export type RevealDirection = 'right' | 'left' | 'down' | 'up' | 'center-x';

/** 裁出完整图形，不重算数据几何。终帧不增加分组或定义，静态导出保持原样。 */
export function reveal(s: Scene, k: number, b: Pick<Box, 'x' | 'y' | 'w' | 'h'>, direction: RevealDirection, draw: () => void): void {
  if (k >= 1) { draw(); return; }
  k = Math.max(0, k);
  const w = direction === 'right' || direction === 'left' || direction === 'center-x' ? b.w * k : b.w;
  const h = direction === 'down' || direction === 'up' ? b.h * k : b.h;
  const x = b.x + (direction === 'left' ? b.w - w : direction === 'center-x' ? (b.w - w) / 2 : 0);
  const y = b.y + (direction === 'up' ? b.h - h : 0);
  const i = clips.get(s) ?? 0;
  clips.set(s, i + 1);
  const clipPath = s.url(`motion-${i}`, (id) => el('clipPath', { id, clipPathUnits: 'userSpaceOnUse' }, rect(x, y, w, h)));
  const firstHit = s.hits.length;
  drawGroup(s, { clipPath }, draw);
  // 播放期间只有已经露出的部分可悬停，隐藏文字不会提前抢到命中。
  for (let j = s.hits.length - 1; j >= firstHit; j--) {
    const hit = s.hits[j], x1 = Math.min(x + w, hit.x + hit.w), y1 = Math.min(y + h, hit.y + hit.h);
    hit.x = Math.max(x, hit.x); hit.y = Math.max(y, hit.y);
    hit.w = Math.max(0, x1 - hit.x); hit.h = Math.max(0, y1 - hit.y);
    if (!hit.w || !hit.h) s.hits.splice(j, 1);
  }
}

/** 刚性的纸片/台座进场；尺寸和数据比例不伸缩。 */
export function move(s: Scene, k: number, dx: number, dy: number, draw: () => void): void {
  if (k >= 1) { draw(); return; }
  const firstHit = s.hits.length, x = r(dx * (1 - k)), y = r(dy * (1 - k));
  drawGroup(s, { transform: `translate(${x} ${y})`, visibility: k <= 0 ? 'hidden' : undefined }, draw);
  if (k <= 0) s.hits.splice(firstHit);
  else for (const hit of s.hits.slice(firstHit)) { hit.x += x; hit.y += y; }
}
