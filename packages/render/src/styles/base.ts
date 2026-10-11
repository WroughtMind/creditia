// 基础三套：极简（白）、暗室（黑）、液态玻璃。克制、干净，品牌色只出现在 logo 和玻璃的光里。
import { defineStyle, grain, linear, radial, blur, BRAND, logo, type Style } from '../style.ts';
import { el, g, rect, circle, line, r, mix, alpha } from '../svg.ts';

const SANS = 'Inter, Noto Sans SC';

export const minimal: Style = defineStyle({
  id: 'minimal', name: '极简', en: 'Minimal', note: '白纸黑字，只留信息本身', dark: false,
  c: { bg: '#F4F3F0', paper: '#FFFFFF', ink: '#121212', soft: '#6E6B65', faint: '#ADA9A2', line: '#E7E5E0', accent: '#121212', accent2: '#8A8780' },
  series: ['#121212', '#4B4A47', '#76746F', '#9E9B95', '#C2BFB8', '#DAD7D1'],
  f: { display: { family: SANS, weight: 600 }, body: { family: SANS, weight: 400 }, num: { family: SANS, weight: 500 }, label: { family: SANS, weight: 500 }, mono: { family: 'JetBrains Mono, Noto Sans SC', weight: 400 } },
  radius: 99, labelTrack: 0.16,
  dot: () => `<circle cx="12" cy="12" r="11" fill="#F7F6F3" stroke="#121212" stroke-width="1"/><rect x="6" y="11.2" width="12" height="1.6" rx=".8" fill="#121212"/>`,
  ground(s, b) {
    s.bg = minimal.c.bg;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: minimal.c.bg }));
  },
  plate(s, b) {
    const sh = s.url('shadow', (id) => el('filter', { id, x: '-10%', y: '-10%', width: '120%', height: '130%' },
      el('feDropShadow', { dx: 0, dy: 10, stdDeviation: 18, floodColor: '#1A1A1A', floodOpacity: 0.07 })));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#FFFFFF', rx: 22, filter: sh }));
    return b;
  },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.3, { fill: 'none', stroke: minimal.c.faint, strokeWidth: 1.4, strokeDasharray: '2.5 2.5' })); return; }
    s.add(logo(key, cx, cy, size * 0.82, minimal.c.ink));
  },
});

export const noir: Style = defineStyle({
  id: 'noir', name: '暗室', en: 'Noir', note: '黑底白字，像深夜的编辑器', dark: true,
  c: { bg: '#0A0A0A', paper: '#141414', ink: '#F2F1EE', soft: '#8E8B85', faint: '#56544F', line: '#242422', accent: '#F2F1EE', accent2: '#8E8B85' },
  series: ['#F2F1EE', '#BDBBB6', '#8F8D88', '#6A6864', '#4B4A47', '#363533'],
  f: minimal.f, radius: 99, labelTrack: 0.16,
  dot: () => `<circle cx="12" cy="12" r="11" fill="#0E0E0E" stroke="#3A3A38" stroke-width="1"/><rect x="6" y="11.2" width="12" height="1.6" rx=".8" fill="#F2F1EE"/>`,
  ground(s, b) {
    s.bg = noir.c.bg;
    s.add(rect(b.x, b.y, b.w, b.h, { fill: noir.c.bg }));
    s.add(rect(b.x, b.y, b.w, b.h, { fill: radial(s, 'noir-vig', [[0, '#1C1C1B', 1], [1, '#0A0A0A', 1]], { cx: 0.5, cy: 0.1, r: 0.9 }) }));
  },
  plate(s, b) {
    s.add(rect(b.x, b.y, b.w, b.h, { fill: '#121212', rx: 22, stroke: '#232322', strokeWidth: 1 }));
    return b;
  },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.3, { fill: 'none', stroke: noir.c.faint, strokeWidth: 1.4, strokeDasharray: '2.5 2.5' })); return; }
    s.add(logo(key, cx, cy, size * 0.82, noir.c.ink));
  },
});

/** 液态玻璃：底下是按贡献方品牌色生成的柔光，玻璃面做折射（位移）+ 磨砂（模糊）+ 高光描边 */
export const glass: Style = defineStyle({
  id: 'glass', name: '液态玻璃', en: 'Liquid Glass', note: '磨砂、折射、边缘高光；颜色来自各家品牌色', dark: false,
  c: { bg: '#E9ECF2', paper: '#FFFFFF', ink: '#0F1115', soft: '#4D5260', faint: '#8C92A0', line: 'rgba(15,17,21,0.08)', accent: '#0F1115', accent2: '#5B6170' },
  series: ['#0F1115', '#3A3F4B', '#666C79', '#9096A3', '#B8BDC8', '#D6D9E0'],
  f: { display: { family: SANS, weight: 600 }, body: { family: SANS, weight: 400 }, num: { family: SANS, weight: 600 }, label: { family: SANS, weight: 500 }, mono: { family: 'JetBrains Mono, Noto Sans SC' } },
  radius: 99, labelTrack: 0.14,
  dot: () => `<defs><radialGradient id="gd" cx=".3" cy=".3" r=".9"><stop offset="0" stop-color="#FFFFFF"/><stop offset=".55" stop-color="#C9D4FF"/><stop offset="1" stop-color="#FFD3C2"/></radialGradient></defs><circle cx="12" cy="12" r="11" fill="url(#gd)"/><circle cx="12" cy="12" r="10.4" fill="none" stroke="#FFFFFF" stroke-opacity=".9" stroke-width="1.2"/><ellipse cx="9" cy="7.5" rx="5" ry="2.4" fill="#FFFFFF" fill-opacity=".75"/>`,
  ground(s, b, m) {
    s.bg = glass.c.bg;
    const cols = m.rows.filter((x) => x.kind === 'agent').slice(0, 3).map((x) => BRAND[x.key] ?? '#8EA2FF');
    while (cols.length < 3) cols.push(['#8EA2FF', '#FFB38E', '#7FE0C8'][cols.length]);
    s.add(rect(b.x, b.y, b.w, b.h, { fill: linear(s, 'gl-bg', [[0, '#F3F4F8'], [1, '#E4E8F0']]) }));
    const orbs = g({ id: s.id + '-orbs' },
      circle(b.x + b.w * 0.18, b.y + b.h * 0.2, Math.max(b.w, b.h) * 0.36, { fill: cols[0], opacity: 0.55 }),
      circle(b.x + b.w * 0.88, b.y + b.h * 0.78, Math.max(b.w, b.h) * 0.4, { fill: cols[1], opacity: 0.5 }),
      circle(b.x + b.w * 0.78, b.y + b.h * 0.08, Math.max(b.w, b.h) * 0.22, { fill: cols[2], opacity: 0.42 }),
      circle(b.x + b.w * 0.3, b.y + b.h * 0.95, Math.max(b.w, b.h) * 0.2, { fill: mix(cols[0], cols[1], 0.5), opacity: 0.35 }));
    s.add(g({ filter: blur(s, Math.round(Math.max(b.w, b.h) / 9)) }, orbs));
    grain(s, b, { opacity: 0.05, seed: 3 });
  },
  plate(s, b) {
    // 折射：把柔光再画一遍，做位移 + 模糊，剪进玻璃面
    const clip = s.def(`gclip${Math.round(b.x)}-${Math.round(b.y)}`, (id) => el('clipPath', { id }, rect(b.x, b.y, b.w, b.h, { rx: 30 })));
    // 滤镜区域用用户坐标、限定在玻璃面内：resvg 在区域超出画布又被剪裁时会崩
    const fx = Math.max(0, Math.floor(b.x)), fy = Math.max(0, Math.floor(b.y));
    const refr = s.def(`refract${fx}-${fy}`, (id) => el('filter', { id, filterUnits: 'userSpaceOnUse', x: fx, y: fy, width: Math.floor(Math.min(b.w, s.w - fx)), height: Math.floor(Math.min(b.h, s.h - fy)) },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: 0.006, numOctaves: 2, seed: 9, result: 'n' }),
      el('feDisplacementMap', { in: 'SourceGraphic', in2: 'n', scale: 70, xChannelSelector: 'R', yChannelSelector: 'G', result: 'd' }),
      el('feGaussianBlur', { in: 'd', stdDeviation: 28 })));
    s.add(g({ clipPath: `url(#${clip})` },
      el('use', { href: `#${s.id}-orbs`, filter: `url(#${refr})` }),
      rect(b.x, b.y, b.w, b.h, { fill: '#FFFFFF', opacity: 0.5 }),
      rect(b.x, b.y, b.w, b.h * 0.42, { fill: linear(s, 'gl-sheen', [[0, '#FFFFFF', 0.55], [1, '#FFFFFF', 0]]) })));
    s.add(rect(b.x + 0.75, b.y + 0.75, b.w - 1.5, b.h - 1.5, { rx: 29.5, fill: 'none', stroke: linear(s, 'gl-rim', [[0, '#FFFFFF', 1], [0.45, '#FFFFFF', 0.35], [1, '#FFFFFF', 0.85]], [0, 0, 1, 1]), strokeWidth: 1.5 }));
    s.add(rect(b.x + 3, b.y + 3, b.w - 6, b.h - 6, { rx: 27, fill: 'none', stroke: '#0F1115', strokeOpacity: 0.05, strokeWidth: 1 }));
    return b;
  },
  bar(s, x, y, w, h, color) {
    const rr = Math.min(h, w) / 2;
    s.add(rect(x, y, w, h, { fill: color, rx: rr }));
    if (h < w) s.add(rect(x + 1, y + 0.6, Math.max(0, w - 2), h * 0.42, { fill: '#FFFFFF', opacity: 0.35, rx: h / 4 }));
  },
  track(s, x, y, w, h) { s.add(rect(x, y, w, h, { fill: '#0F1115', opacity: 0.06, rx: h / 2 })); },
  color(key, rank) {
    if (key === 'unattributed') return glass.c.faint;
    if (key === 'other') return '#B8BDC8';
    if (key === 'human') return '#5B6170';
    return BRAND[key] ? mix(BRAND[key], '#FFFFFF', 0.08) : glass.series[Math.min(rank, 5)];
  },
  mark(s, key, cx, cy, size) {
    if (key === 'unattributed') { s.add(circle(cx, cy, size * 0.3, { fill: 'none', stroke: glass.c.faint, strokeWidth: 1.4, strokeDasharray: '2.5 2.5' })); return; }
    const c = BRAND[key] ?? '#5B6170';
    s.add(circle(cx, cy, size * 0.62, { fill: '#FFFFFF', opacity: 0.7 }));
    s.add(circle(cx, cy, size * 0.62, { fill: alpha(c, 0.14), stroke: '#FFFFFF', strokeWidth: 1.2 }));
    s.add(logo(key, cx, cy, size * 0.62, c));
  },
});

export { el, g, rect, circle, line, r };
