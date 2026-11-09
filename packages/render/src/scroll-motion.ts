/** 固定阅读轴与语义章节；三端从一次 RGBA 图集轻量合成完整字组，不逐帧栅格化长卷。 */
export interface ScrollKey { offset: number; y: number; easing?: 'steps(1, end)' }
export interface ScrollLayer { x: number; y: number; w: number; h: number; paint: string; role: 'chapter' | 'role' | 'credits' | 'line'; chapter: number; start?: number; duration?: number }
export interface ScrollMaterial { style: string; layers: ScrollLayer[] }
export interface ScrollMotion { kind: 'scroll'; viewW: number; viewH: number; secs: number; keys: ScrollKey[]; material?: ScrollMaterial }
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

/** 字组进入真实视窗后，在既有阅读停驻之前完成材质动作；不改阅读轴与总时长。 */
export function scrollLayerTimings(motion: ScrollMotion): ScrollLayer[] {
  if (!motion.material) return [];
  const at = (target: number) => {
    if (target <= 0) return 0;
    for (let i = 1; i < motion.keys.length; i++) {
      const a = motion.keys[i - 1], b = motion.keys[i];
      if (b.y >= target && b.y > a.y) return (a.easing ? b.offset : a.offset + (b.offset - a.offset) * (target - a.y) / (b.y - a.y)) * motion.secs;
    }
    return motion.secs;
  };
  const timed = motion.material.layers.map((layer) => {
    // 整个真实字组先进入可读视窗，再压印/归位；不能在底缘外把动作做完。
    const visibleBottom = Math.min(layer.h, motion.viewH - 80);
    const start = at(Math.max(0, layer.y + visibleBottom - motion.viewH + 48));
    const style = motion.material!.style;
    const sameChapter = motion.material!.layers.filter(other => other.chapter === layer.chapter);
    const order = sameChapter.indexOf(layer);
    const delay = ['ukiyoe', 'pastel', 'baroque'].includes(style) && layer.role === 'credits' ? .18 : start === 0 ? order * .065 : 0;
    const duration = style === 'cutout' ? .95 : ['manga', 'cyber'].includes(style) ? .42 : ['ink', 'meadow', 'pastel'].includes(style) ? .95 : .8;
    return { ...layer, start: Math.min(start + delay, Math.max(0, motion.secs - duration)), duration };
  });
  for (const chapter of new Set(timed.map(layer => layer.chapter))) {
    const role = timed.find(layer => layer.chapter === chapter && layer.role === 'role');
    const firstCredit = timed.find(layer => layer.chapter === chapter && (layer.role === 'line' || layer.role === 'credits'));
    if (role && firstCredit) {
      // 跨过阅读停驻时也保持“谁担任什么”相连，不让角色先亮出后等数秒才有姓名。
      const together = Math.max(role.start, firstCredit.start);
      role.start = firstCredit.start = together;
      let previous = together;
      for (const line of timed.filter(layer => layer.chapter === chapter && layer.role === 'line')) {
        line.start = Math.max(line.start, previous); previous = line.start;
      }
    }
  }
  return timed;
}

/** 移动段以 20fps 采样，阅读停驻和逐行进纸只需一张保持帧。 */
export function scrollSamples(motion: ScrollMotion, minimum = 0) {
  const interval = motion.secs * 1000 / Math.max(minimum, Math.ceil(motion.secs * 20));
  const times: number[] = [];
  for (let i = 1; i < motion.keys.length; i++) {
    const a = motion.keys[i - 1], b = motion.keys[i];
    const duration = (b.offset - a.offset) * motion.secs * 1000;
    if (duration <= 0) continue;
    const count = a.y === b.y || a.easing ? 1 : Math.ceil(duration / interval);
    for (let j = 0; j < count; j++) times.push(a.offset + (b.offset - a.offset) * j / count);
  }
  times.push(1);
  // 停驻本身不重画；只有仍在合版/归位的章节需要保持同样 20fps 的采样。
  for (const layer of scrollLayerTimings(motion)) {
    for (let ms = layer.start! * 1000; ms < (layer.start! + layer.duration!) * 1000; ms += interval) if (ms >= 0 && ms < motion.secs * 1000) times.push(ms / (motion.secs * 1000));
    times.push(Math.min(1, (layer.start! + layer.duration!) / motion.secs));
  }
  const ordered = [...new Set(times)].sort((a, b) => a - b).reduce<number[]>((result, time) => {
    // 新材质节点与原移动采样若相隔不到20ms，合为一个样本；不写浏览器会扩成100ms的零延迟GIF帧。
    if (motion.material && result.length && (time - result.at(-1)!) * motion.secs * 1000 < Math.min(20, interval / 2)) {
      if (time === 1) result[result.length - 1] = 1;
    } else result.push(time);
    return result;
  }, []);
  return ordered.map((progress, i) => ({ progress, delay: ((ordered[i + 1] ?? 1) - progress) * motion.secs * 1000 }));
}
