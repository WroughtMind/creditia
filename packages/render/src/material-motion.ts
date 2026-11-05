// Material motion changes the foreground, leaving the paper/plate steady.
// Fixed noise seeds keep ink edges coherent between samples. At t=1 no wrapper
// or filter is emitted, so the resting frame is exactly the ordinary artwork.
import { Scene, el, rect } from './svg.ts';

export function materialSvg(scene: Scene, style: string, time?: number): string {
  if (time === undefined || time >= 1 || !['ink', 'ukiyoe', 'woodblock', 'manuscript'].includes(style)) return scene.toString();
  const t = Math.max(0, time), wet = style === 'ink', print = style === 'woodblock';
  const u = 1 - Math.pow(1 - t, wet ? 1.6 : 2.2);
  const fuzz = wet ? 28 : print ? 2 : 8;
  const filter = scene.def('material-edge', id => el('filter', { id, x: -0.1, y: -0.1, width: 1.2, height: 1.2 },
    el('feTurbulence', { type: 'fractalNoise', baseFrequency: wet ? '0.012 0.065' : '0.04 0.14', numOctaves: wet ? 3 : 2, seed: wet ? 7 : 19, result: 'grain' }),
    el('feDisplacementMap', { in: 'SourceGraphic', in2: 'grain', scale: fuzz * (1 - t), xChannelSelector: 'R', yChannelSelector: 'G' })));
  const mask = scene.def('material-unfold', id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: 0, y: 0, width: scene.w, height: scene.h },
    rect(-fuzz, -fuzz, Math.max(0, scene.w * u + fuzz * 2), scene.h + fuzz * 2, { fill: '#fff', filter: `url(#${filter})`, opacity: t === 0 ? 0 : 1 })));
  return scene.toString(content => el('g', { mask: `url(#${mask})` }, content));
}
