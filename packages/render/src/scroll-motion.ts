/** 固定视窗读取同一张长图；三端只改变窗口的位置，不重画或重排文字。 */
export interface ScrollKey { offset: number; y: number; easing?: 'steps(1, end)' }
export interface ScrollMotion { kind: 'scroll'; viewW: number; viewH: number; secs: number; keys: ScrollKey[] }
export type ScrollReading = 'continuous' | 'sections' | 'lines';
/** 投影/光学表面连续运动，纸本按章节阅读，字符屏按真实行距进纸。 */
export const SCROLL_READING: Record<string, ScrollReading> = {
  minimal: 'continuous', 'minimal-light': 'continuous', noir: 'continuous', 'minimal-dark': 'continuous',
  glass: 'continuous', film: 'continuous', cinema: 'continuous', mono: 'continuous', neon: 'continuous',
  editorial: 'sections', broadsheet: 'sections', playbill: 'sections', gala: 'sections', poster: 'sections',
  ink: 'sections', ukiyoe: 'sections', woodblock: 'sections', baroque: 'sections', grimoire: 'sections',
  manuscript: 'sections', pastel: 'sections', cutout: 'sections', meadow: 'sections', manga: 'sections',
  terminal: 'lines', crt: 'lines', arcade: 'lines', cyber: 'lines',
};
export function scrollMotion(style: string, viewW: number, viewH: number, height: number, sections: { y: number; h: number }[], lines: number[]): ScrollMotion {
  const travel = Math.max(0, height - viewH), mode = SCROLL_READING[style];
  if (!mode) throw new Error(`Unknown scroll material: ${style}`);
  const keys: ScrollKey[] = [{ offset: 0, y: 0 }];
  let time = 0, y = 0;
  const add = (next: number, seconds: number) => { time += seconds; y = next; keys.push({ offset: time, y }); };
  if (mode === 'continuous') add(travel, travel / 90);
  else if (mode === 'sections') {
    // 章节顶端落在视窗上方留白内；段落越长，完整停留的阅读时间越长。
    for (const section of sections) {
      const next = Math.max(0, Math.min(travel, section.y - viewH * 0.2));
      if (next > y) add(next, (next - y) / 90);
      add(y, Math.min(section.h, viewH) / 90);
    }
    if (y < travel) add(travel, (travel - y) / 90);
  } else {
    // 每个真实文字行进入底边后再进一行，不按固定像素或帧数量化。
    const stops = [...new Set(lines.map(line => Math.max(0, Math.min(travel, line - viewH + 40))))].sort((a, b) => a - b);
    for (const next of [...stops, travel]) {
      if (next <= y) continue;
      keys[keys.length - 1].easing = 'steps(1, end)';
      add(next, (next - y) / 90);
    }
  }
  if (!time) { keys.push({ offset: 1, y: 0 }); time = 1; }
  else for (const key of keys) key.offset /= time;
  return { kind: 'scroll', viewW, viewH, secs: Math.max(14, time), keys };
}
/** 与 WAAPI/CSS 的线性段、行末阶梯完全相同的采样。 */
export function scrollY(keys: ScrollKey[], progress: number): number {
  const t = Math.max(0, Math.min(1, progress));
  for (let i = 1; i < keys.length; i++) {
    const a = keys[i - 1], b = keys[i];
    if (t < b.offset) return a.easing ? a.y : a.y + (b.y - a.y) * (t - a.offset) / (b.offset - a.offset);
  }
  return keys.at(-1)!.y;
}
/** CSS 百分比以长图自身高度计算，App 和 HTML 共用相同关键帧。 */
export function scrollTransforms(keys: ScrollKey[], height: number) {
  return keys.map(({ offset, y, easing }) => ({ offset, transform: `translateY(${-y / height * 100}%)`, ...(easing ? { easing } : {}) }));
}
