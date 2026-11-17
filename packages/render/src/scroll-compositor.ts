import { scrollPaperPose, scrollPaperTexture } from './scroll-paper.ts';
import { scrollY, scrollLayerTimings, type ScrollMotion, type ScrollLayer } from './scroll-motion.ts';

export interface ScrollPixels { w: number; h: number; data: Uint8Array | Uint8ClampedArray }
export interface ScrollAtlas { svg: string; w: number; h: number; originalW: number; originalH: number; layers: ScrollLayer[] }

/** 三个并排平面共享相同像素相位：原卷、无章节字组的原纸、透明章节前景。只栅格化这个 atlas 一次。 */
export function scrollAtlas(svg: string, w: number, h: number, motion: ScrollMotion): ScrollAtlas {
  const material = motion.material;
  if (!material) throw new Error('Scroll material layers are required');
  const inner = svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '');
  const defs = /<defs>[\s\S]*?<\/defs>/.exec(inner)?.[0] ?? '';
  const body = inner.replace(defs, '');
  let background = body;
  for (const layer of material.layers) {
    if (!layer.paint || !background.includes(layer.paint)) throw new Error('Declared scroll layer is missing from the original scene');
    background = background.replace(layer.paint, '');
  }
  const layers = scrollLayerTimings(motion);
  const plane = (paint: string, index: number) => {
    const ids = new Map([...defs.matchAll(/\bid="([^"]+)"/g)].map(match => [match[1], `scroll-plane-${index}-${match[1]}`]));
    const content = (defs + paint).replace(/\bid="([^"]+)"/g, (_, id) => `id="${ids.get(id) ?? id}"`).replace(/url\(#([^)]*)\)/g, (_, id) => `url(#${ids.get(id) ?? id})`).replace(/((?:xlink:)?href=")#([^"]+)(")/g, (_, a, id, b) => a + '#' + (ids.get(id) ?? id) + b);
    // 原卷必须隔离在原画幅：装饰线可越界，不能污染另两张透明/纸面图层。
    // 保持一次栅格化；显式裁剪可能有少量边缘AA差异，不能为字节相等放开隔离。
    if (index === 0) return `<defs><clipPath id="scroll-original-viewport"><rect width="${w}" height="${h}"/></clipPath></defs><g clip-path="url(#scroll-original-viewport)">${defs + paint}</g>`;
    return `<svg x="${index * w}" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${content}</svg>`;
  };
  return { w: w * 3, h, originalW: w, originalH: h, layers,
    svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${w * 3}" height="${h}" viewBox="0 0 ${w * 3} ${h}">${plane(body, 0)}${plane(background, 1)}${plane(material.layers.map(layer => layer.paint).join(''), 2)}</svg>` };
}

/** App、GIF、离线 HTML 都调用此同一合成器。每帧只处理可见窗口及未落定章节，不再排版或栅格化长卷。 */
export function createScrollCompositor(source: ScrollPixels, atlas: Pick<ScrollAtlas, 'originalW' | 'originalH' | 'layers'>, motion: ScrollMotion, positionAt = scrollY, paperPose = scrollPaperPose, paperTexture = scrollPaperTexture) {
  const w = source.w / 3, scale = w / atlas.originalW, h = Math.round(motion.viewH * scale);
  if (!Number.isInteger(w)) throw new Error('Scroll atlas width must contain three equal pixel planes');
  const pixels = source.data, out = new Uint8ClampedArray(w * h * 4);
  const style = motion.material!.style;
  const layers = atlas.layers.map(l => ({ ...l, x: l.x * scale, y: l.y * scale, w: l.w * scale, h: l.h * scale }));
  const blend = (to: number, from: number, opacity: number, glint = 0) => {
    const a = pixels[from + 3] / 255 * opacity, rest = 1 - a;
    if (a <= 0) return;
    const red = pixels[from] * (1 - glint) + 255 * glint, green = pixels[from + 1] * (1 - glint) + 239 * glint, blue = pixels[from + 2] * (1 - glint) + 180 * glint;
    if (out[to + 3] === 255) {
      out[to] = Math.round(red * a + out[to] * rest); out[to + 1] = Math.round(green * a + out[to + 1] * rest); out[to + 2] = Math.round(blue * a + out[to + 2] * rest);
    } else {
      // ImageData存straight RGBA；透明纸面不能把预乘RGB当作最终颜色写回。
      const behind = out[to + 3] / 255 * rest, alpha = a + behind;
      out[to] = Math.round((red * a + out[to] * behind) / alpha); out[to + 1] = Math.round((green * a + out[to + 1] * behind) / alpha); out[to + 2] = Math.round((blue * a + out[to + 2] * behind) / alpha);
      out[to + 3] = Math.round(255 * alpha);
    }
  };
  const edge = (x: number, y: number, top: number, from: number, opacity: number) => {
    if (x >= 0 && x < w && y >= top && y < top + h) blend(((y - top) * w + x) * 4, from, opacity);
  };
  const render = (progress: number, cropY?: number): ScrollPixels => {
    const t = Math.max(0, Math.min(1, progress)), seconds = t * motion.secs;
    const top = Math.max(0, Math.min(Math.round((cropY ?? positionAt(motion.keys, t)) * scale), source.h - h));
    for (let y = 0; y < h; y++) out.set(pixels.subarray(((top + y) * source.w) * 4, ((top + y) * source.w + w) * 4), y * w * 4);
    if (t >= 1) return { w, h, data: out };
    const active = layers.map(l => ({ l, p: Math.max(0, Math.min(1, (seconds - l.start!) / l.duration!)) }))
      .filter(({ l, p }) => p < 1 && l.y + l.h > top && l.y < top + h);
    // 先恢复这些字组下的真实纸面，再合成完整字组；不会擦掉已经落定的相邻章。
    for (const { l } of active) {
      const x0 = Math.max(0, Math.floor(l.x)), x1 = Math.min(w, Math.ceil(l.x + l.w));
      const y0 = Math.max(top, Math.floor(l.y)), y1 = Math.min(top + h, Math.ceil(l.y + l.h));
      for (let y = y0; y < y1; y++) out.set(pixels.subarray((y * source.w + w + x0) * 4, (y * source.w + w + x1) * 4), ((y - top) * w + x0) * 4);
    }
    for (const { l, p } of active) {
      if (p <= 0) continue;
      const rest = 1 - p;
      const paper = paperPose(style, l.role, p);
      let dx = paper.dx * scale, dy = paper.dy * scale, visible = true, angle = 0;
      let bleed = paper.bleed ?? 0;
      if (style === 'cutout') { dx = -9 * rest * rest * scale; dy = 12 * rest * rest * scale; }
      else if (style === 'cyber') { visible = p >= .32; dy = Math.ceil(rest * 2) * 2 * scale; }
      else if (style === 'manga') { dy = -3 * rest * rest * scale; }
      else if (style === 'meadow') { angle = Math.PI / 30 * rest * rest; bleed = .8 * rest; }
      else if (style === 'pastel') { dx = (l.role === 'role' ? -28 : 28) * rest * rest * scale; dy = -8 * Math.sin(p * Math.PI) * rest * scale; bleed = 1.2 * rest; }
      const pivotX = l.x + 12 * scale, pivotY = l.y + l.h / 2;
      const cosine = Math.cos(angle), sine = Math.sin(angle);
      const corners = [[l.x, l.y], [l.x + l.w, l.y], [l.x, l.y + l.h], [l.x + l.w, l.y + l.h]].map(([x, y]) => ({ x: pivotX + (x - pivotX) * cosine - (y - pivotY) * sine + dx, y: pivotY + (x - pivotX) * sine + (y - pivotY) * cosine + dy }));
      const left = Math.floor(Math.min(...corners.map(c => c.x))), right = Math.ceil(Math.max(...corners.map(c => c.x)));
      const upper = Math.floor(Math.min(...corners.map(c => c.y))), lower = Math.ceil(Math.max(...corners.map(c => c.y)));
      if (style === 'cutout') {
        const shift = Math.max(1, Math.round(2 * rest * scale));
        for (let y = Math.max(top, upper + shift); y < Math.min(top + h, lower + shift); y++) for (let x = Math.max(0, left + shift); x < Math.min(w, right + shift); x++) {
          if (x < right && y < lower) continue;
          const pos = ((y - top) * w + x) * 4, shade = .16 * rest;
          out[pos] = Math.round(out[pos] * (1 - shade)); out[pos + 1] = Math.round(out[pos + 1] * (1 - shade)); out[pos + 2] = Math.round(out[pos + 2] * (1 - shade));
        }
      }
      if (visible && style === 'cutout') {
        // 原纸片大面通常完全不透明，按行复制；透明像素才需要 Alpha 混合。
        const x0 = Math.max(0, Math.ceil(l.x + dx)), x1 = Math.min(w, Math.ceil(l.x + l.w + dx));
        for (let y = Math.max(top, Math.ceil(l.y + dy)); y < Math.min(top + h, Math.ceil(l.y + l.h + dy)); y++) {
          const sy = Math.max(0, Math.min(source.h - 1, Math.round(y - dy))), sx = Math.max(0, Math.round(x0 - dx));
          const from = (sy * source.w + sx) * 4, to = ((y - top) * w + x0) * 4, bytes = Math.max(0, x1 - x0) * 4;
          let opaque = true;
          for (let at = from + 3; at < from + bytes; at += 4) if (pixels[at] !== 255) { opaque = false; break; }
          if (opaque) out.set(pixels.subarray(from, from + bytes), to);
          else for (let at = 0; at < bytes; at += 4) blend(to + at, from + at, 1);
        }
      }
      if (visible && style !== 'cutout') {
        for (let y = Math.max(top, upper); y < Math.min(top + h, lower); y++) for (let x = Math.max(0, left); x < Math.min(w, right); x++) {
          const sampleX = pivotX + (x - dx - pivotX) * cosine + (y - dy - pivotY) * sine;
          const sampleY = pivotY - (x - dx - pivotX) * sine + (y - dy - pivotY) * cosine;
          if (sampleX < l.x || sampleX >= l.x + l.w || sampleY < l.y || sampleY >= l.y + l.h) continue;
          if (angle) {
            // 对实际完整字行作刚性逆变换；双线性 Alpha 采样避免小字转开时锯齿跳动。
            const xx = Math.floor(sampleX), yy = Math.floor(sampleY), fx = sampleX - xx, fy = sampleY - yy;
            let alpha = 0, red = 0, green = 0, blue = 0;
            for (let j = 0; j < 2; j++) for (let i = 0; i < 2; i++) {
              if (xx + i < 0 || xx + i >= w || yy + j < 0 || yy + j >= source.h) continue;
              const from = ((yy + j) * source.w + 2 * w + xx + i) * 4;
              const weight = (i ? fx : 1 - fx) * (j ? fy : 1 - fy) * pixels[from + 3] / 255;
              alpha += weight; red += pixels[from] * weight; green += pixels[from + 1] * weight; blue += pixels[from + 2] * weight;
            }
            if (alpha > 0) {
              const to = ((y - top) * w + x) * 4, rest = 1 - alpha;
              if (out[to + 3] === 255) { out[to] = Math.round(red + out[to] * rest); out[to + 1] = Math.round(green + out[to + 1] * rest); out[to + 2] = Math.round(blue + out[to + 2] * rest); }
              else {
                const behind = out[to + 3] / 255 * rest, combined = alpha + behind;
                out[to] = Math.round((red + out[to] * behind) / combined); out[to + 1] = Math.round((green + out[to + 1] * behind) / combined); out[to + 2] = Math.round((blue + out[to + 2] * behind) / combined); out[to + 3] = Math.round(255 * combined);
              }
            }
            continue;
          }
          const sx = Math.max(0, Math.min(w - 1, Math.round(sampleX))), sy = Math.max(0, Math.min(source.h - 1, Math.round(sampleY)));
          const plane = style === 'cutout' ? 0 : 2, from = (sy * source.w + plane * w + sx) * 4;
          if (!pixels[from + 3]) continue;
          let texture = paper.grain ? paperTexture(style, p, sx / scale, sy / scale) : 1;
          if (style === 'manga') {
            // 网点仅填在原字形内部；完整浅墨字身保持可读，没有额外冒号或标点。
            const xx = ((sx / scale) % 6 + 6) % 6 - 3, yy = ((sy / scale) % 6 + 6) % 6 - 3;
            texture = xx * xx + yy * yy < (1 + p * 3) ** 2 ? 1 : .58 + .42 * p;
          }
          const alpha = paper.opacity * texture;
          // 墨晕/压印只沿真实字形的外缘，字身一直完整；不是整页柔焦或揭幕窗口。
          if (bleed > .1 && plane === 2) {
            const radius = bleed * scale, r0 = Math.floor(radius), fraction = radius - r0;
            const a = (paper.bleedOpacity ?? .1) * rest;
            if (r0 > 0) {
              const alpha = a * (1 - fraction);
              edge(x - r0, y, top, from, alpha); edge(x + r0, y, top, from, alpha);
              edge(x, y - r0, top, from, alpha); edge(x, y + r0, top, from, alpha);
            }
            if (fraction > 0) {
              const r1 = r0 + 1, alpha = a * fraction;
              edge(x - r1, y, top, from, alpha); edge(x + r1, y, top, from, alpha);
              edge(x, y - r1, top, from, alpha); edge(x, y + r1, top, from, alpha);
            }
            if (style === 'ink') {
              const radius = bleed * scale / Math.SQRT2, r0 = Math.floor(radius), fraction = radius - r0;
              if (r0 > 0) { const alpha = a * .65 * (1 - fraction); edge(x - r0, y - r0, top, from, alpha); edge(x + r0, y - r0, top, from, alpha); edge(x - r0, y + r0, top, from, alpha); edge(x + r0, y + r0, top, from, alpha); }
              if (fraction > 0) { const r1 = r0 + 1, alpha = a * .65 * fraction; edge(x - r1, y - r1, top, from, alpha); edge(x + r1, y - r1, top, from, alpha); edge(x - r1, y + r1, top, from, alpha); edge(x + r1, y + r1, top, from, alpha); }
            }
          }
          const shine = paper.glint ? Math.max(0, 1 - Math.abs((sx - l.x) / Math.max(1, l.w) - p) / .2) * paper.glint : 0;
          blend(((y - top) * w + x) * 4, from, alpha, shine);
        }
      }
      if (style === 'cyber' && p < .8) {
        const x = Math.round(l.x + 3 * scale), y0 = Math.max(top, Math.round(l.y + 10 * scale)), y1 = Math.min(top + h, Math.round(l.y + (10 + 20 * Math.min(1, p / .48)) * scale));
        for (let y = y0; y < y1; y++) for (let xx = Math.max(0, x); xx < Math.min(w, x + Math.max(1, Math.round(2 * scale))); xx++) {
          const pos = ((y - top) * w + xx) * 4; out[pos] = 63; out[pos + 1] = 230; out[pos + 2] = 210; out[pos + 3] = 255;
        }
      }
    }
    return { w, h, data: out };
  };
  return { render, width: w, height: h, sourceBytes: pixels.byteLength, frameBytes: out.byteLength };
}
