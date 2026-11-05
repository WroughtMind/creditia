// Material entrances complement each piece's data animation. Ground and plate
// stay steady for Scene artwork. At rest there is no wrapper/filter: the exact
// ordinary artwork is returned. All masks are deterministic, bounded geometry.
import { Scene, el, rect, circle, path, r } from './svg.ts';
import { motionProfile, materialProgress, unit } from './motion-profiles.ts';

export function materialSvg(scene: Scene, style: string, time?: number): string {
  if (time === undefined || time >= 1) return scene.toString();
  const t = unit(time), profile = motionProfile(style), kind = profile.material;
  const W = scene.w, H = scene.h, cx = W / 2, cy = H / 2, diagonal = Math.hypot(W, H);
  const progress = (i = 0, n = 1) => materialProgress(profile, t, i, n);
  const p = progress();
  let shapes = '', transform: string | undefined;
  // Wet ink, print grain and pen edges retain the same noise seed every frame.
  const brushed = ['ink', 'bands', 'press', 'writing'].includes(kind);
  let brush: string | undefined;
  if (brushed) {
    const wet = kind === 'ink', fuzz = wet ? 28 : kind === 'press' ? 2 : 7;
    brush = scene.def('material-edge', id => el('filter', { id, x: '-10%', y: '-10%', width: '120%', height: '120%' },
      el('feTurbulence', { type: 'fractalNoise', baseFrequency: wet ? '0.012 0.065' : '0.04 0.14', numOctaves: wet ? 3 : 2, seed: wet ? 7 : 19, result: 'grain' }),
      el('feDisplacementMap', { in: 'SourceGraphic', in2: 'grain', scale: fuzz * (1 - t), xChannelSelector: 'R', yChannelSelector: 'G' })));
  }
  const white = { fill: '#fff' };
  switch (kind) {
    case 'ink': case 'press':
      shapes = rect(-32, -32, (W + 64) * p, H + 64, white); break;
    case 'line':
      shapes = rect(0, 0, W * p, H, white); break;
    case 'split':
      shapes = rect(0, 0, W * p, cy + 1, white) + rect(W * (1 - p), cy, W * p, cy + 1, white); break;
    case 'curtain':
      shapes = rect(cx * (1 - p), 0, W * p, H, white); break;
    case 'shutter':
      shapes = rect(0, cy * (1 - p), W, H * p, white); break;
    case 'columns': case 'glass': {
      const n = profile.parts, width = W / n;
      for (let i = 0; i < n; i++) {
        const q = progress(i, n);
        shapes += rect(i * width, kind === 'glass' ? H * (1 - q) : 0, width + 1, H * q, white);
      }
      if (kind === 'glass') transform = `translate(0 ${r((1 - p) * 14)})`;
      break;
    }
    case 'page': {
      const top = W * p, bottom = W * unit(p - .22 * (1 - p));
      shapes = profile.parts === 2
        ? path(`M${r(cx - top / 2)} 0H${r(cx + top / 2)}L${r(cx + bottom / 2)} ${H}H${r(cx - bottom / 2)}Z`, white)
        : path(`M0 0H${r(top)}L${r(bottom)} ${H}H0Z`, white); break;
    }
    case 'iris': {
      const n = profile.parts, radius = diagonal * .6;
      for (let i = 0; i < n; i++) {
        const q = progress(i, n), a = i / n * Math.PI * 2 - Math.PI / 2, b = (i + 1) / n * Math.PI * 2 - Math.PI / 2;
        shapes += path(`M${cx} ${cy}L${r(cx + Math.cos(a) * radius * q)} ${r(cy + Math.sin(a) * radius * q)}L${r(cx + Math.cos(b) * radius * q)} ${r(cy + Math.sin(b) * radius * q)}Z`, white);
      }
      break;
    }
    case 'blocks': case 'panels': {
      const n = profile.parts, height = H / n;
      for (let i = 0; i < n; i++) {
        const q = progress(i, n);
        shapes += rect(i % 2 ? W * (1 - q) : 0, i * height, W * q, height + 1, white);
      }
      break;
    }
    case 'scan': case 'writing': {
      const n = profile.parts, height = H / n;
      if (kind === 'scan' && (profile.steps ?? 0) >= 24) {
        shapes = rect(0, 0, W, height * Math.floor(t * (n + 1)), white); break;
      }
      for (let i = 0; i < n; i++) {
        // A scan/pen crosses each line before advancing to the next one.
        const q = unit(t * (n + 2) - i);
        const width = kind === 'scan' ? Math.floor(q * 24) / 24 : q;
        shapes += rect(0, i * height - 1, W * width, height + 2, white);
      }
      break;
    }
    case 'neon':
      shapes = circle(cx, cy, diagonal * .51 * p, white); break;
    case 'pixels': {
      const nx = profile.parts, ny = Math.max(4, Math.round(nx * H / W)), dx = W / nx, dy = H / ny;
      for (let y = 0; y < ny; y++) for (let x = 0; x < nx; x++) {
        const order = (x + y) / (nx + ny - 2), q = unit((t - order * .55) / .45);
        const side = Math.floor(q * 4) / 4;
        shapes += rect(x * dx + dx * (1 - side) / 2, y * dy + dy * (1 - side) / 2, dx * side + (side ? 1 : 0), dy * side + (side ? 1 : 0), white);
      }
      break;
    }
    case 'bands': case 'slices': {
      const n = profile.parts, height = H / n;
      for (let i = 0; i < n; i++) {
        const q = progress(i, n), reverse = kind === 'slices' && i % 2;
        shapes += rect(reverse ? W * (1 - q) : 0, i * height - 1, W * q, height + 2, white);
      }
      break;
    }
    case 'bubbles': case 'leaves': {
      const n = profile.parts;
      for (let i = 0; i < n; i++) {
        const angle = i * Math.PI * 2 / n, q = progress(i, n);
        const x = cx + Math.cos(angle) * W * .22, y = cy + Math.sin(angle) * H * .22;
        shapes += kind === 'bubbles' ? circle(x, y, diagonal * .42 * q, white)
          : el('ellipse', { cx: x, cy: y, rx: W * .7 * q, ry: H * .27 * q,
            transform: `rotate(${r(angle * 180 / Math.PI + Math.sin(q * Math.PI) * 8)} ${r(x)} ${r(y)})`, ...white });
      }
      break;
    }
    case 'folds': {
      const n = profile.parts, width = W / n;
      for (let i = 0; i < n; i++) {
        const q = progress(i, n), left = i * width;
        shapes += path(`M${r(left)} 0L${r(left + width * q)} ${r(H * .06 * (1 - q))}V${r(H * (1 - .06 * (1 - q)))}L${r(left)} ${H}Z`, white);
      }
      break;
    }
  }
  const mask = scene.def('material-unfold', id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: 0, y: 0, width: W, height: H },
    el('g', { filter: brush ? `url(#${brush})` : undefined, opacity: t === 0 ? 0 : 1 }, shapes)));
  return scene.toString(content => el('g', { mask: `url(#${mask})`, transform }, content));
}

/** Classic Satori trees own their plate; a stable canvas sits behind the reveal. */
export function materialFrame(svg: string, w: number, h: number, style: string, t: number, bg: string): string {
  if (t >= 1) return svg;
  const scene = new Scene(w, h);
  if (bg !== 'transparent') scene.add(rect(0, 0, w, h, { fill: bg }));
  scene.markContent();
  scene.add(svg.replace(/^<svg\b[^>]*>/, '').replace(/<\/svg>\s*$/, ''));
  return materialSvg(scene, style, t);
}
