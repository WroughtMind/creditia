// Platform-neutral export policy. Hosts provide rasterization and font bytes.
import { makeExporter, sized, SIZES, type Art, type Raster, type Deflate } from './export.ts';
import { frameDelays, timelineFor, type Animation } from './timeline.ts';

export type GraphicFormat = 'png' | 'svg' | 'pdf' | 'gif' | 'html';
export type FontMode = 'outline' | 'embed';
export interface ExportOptions {
  format: GraphicFormat; size?: string; lang?: 'zh' | 'en'; fontMode?: FontMode;
  scale?: number; width?: number; style?: string; piece?: string; reducedMotion?: boolean;
}
export interface Exported { bytes: Uint8Array; mime: string; ext: string }
export const encoded = (bytes: Uint8Array) => {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
};
export const scriptJson = (value: unknown) => JSON.stringify(value).replace(/</g, '\\u003c').replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029');

export function exportSize(id = 'native'): { w: number; h: number; id: string } {
  const preset = SIZES.find(s => s.id === id);
  if (preset) return preset;
  const custom = /^(\d{1,5})[x×](\d{1,5})$/.exec(id);
  const w = Number(custom?.[1]), h = Number(custom?.[2]);
  if (!(w > 0 && h > 0 && w <= 32768 && h <= 32768 && w * h <= 32_000_000)) throw new Error('Unknown or excessive export size');
  return { id, w, h };
}

export async function exportArtwork(a: Art, o: ExportOptions, raster: Raster, deflate?: Deflate, animation?: Animation): Promise<Exported> {
  const size = exportSize(o.size), fit = (a: Art) => size.w ? sized(a, size.w, size.h) : a;
  const ex = makeExporter(raster, deflate), enc = new TextEncoder(), fontMode = o.fontMode ?? 'outline';
  const scale = o.scale ?? (size.w ? 1 : 2);
  if (!Number.isFinite(scale) || scale <= 0 || scale > 8) throw new Error('scale must be greater than 0 and at most 8');
  let bytes: Uint8Array, mime: string;
  switch (o.format) {
    case 'svg': bytes = enc.encode(ex.svg(fit(a), fontMode)); mime = 'image/svg+xml'; break;
    case 'png': bytes = ex.png(fit(a), scale); mime = 'image/png'; break;
    case 'pdf': bytes = await ex.pdf(fit(a), size.id === 'a4' ? 1 : Math.min(3, 4000 / Math.max(a.w, a.h))); mime = 'application/pdf'; break;
    case 'gif': {
      if (!animation?.frames.length) throw new Error('This piece has no animation');
      const frames = animation.frames.map(fit);
      const width = o.width ?? Math.min(frames[0].w, 800);
      if (!Number.isSafeInteger(width) || width <= 0 || width > 8192) throw new Error('GIF width must be from 1 to 8192');
      bytes = ex.gif(frames, { width, delays: frameDelays(animation.times, animation.timeline), repeat: animation.timeline.loop ? 0 : -1 });
      mime = 'image/gif'; break;
    }
    case 'html': {
      // Animation owns its viewport, including sized long rolls; no second CSS clock.
      const frames = animation?.frames.map(fit);
      const final = frames?.at(-1) ?? fit(a);
      let html = ex.html([{ ...final, motion: undefined }], { title: a.title, lang: o.lang, fontMode });
      if (frames?.length && animation && !o.reducedMotion) {
        const svgs = frames.map(frame => ex.svg(frame, fontMode));
        html = html.replace('</body>', `<script>\n${htmlPlayer.toString()}\nhtmlPlayer(${scriptJson({ svgs, times: animation.times, timeline: animation.timeline })});\n</script>\n</body>`);
      }
      bytes = enc.encode(html); mime = 'text/html'; break;
    }
    default: throw new Error('Unsupported graphic format');
  }
  return { bytes, mime, ext: o.format };
}

/** Standalone player: same normalized samples and clock as the native client. */
function htmlPlayer(animation: { svgs: string[]; times: number[]; timeline: { durationMs: number; holdMs: number; loop: boolean } }) {
  const root = document.getElementById('a0');
  if (!root) return;
  const still = root.querySelector('svg');
  if (!still) return;
  const images = animation.svgs.map(svg => {
    const img = new Image(); img.src = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })); return img;
  });
  const image = document.createElement('img'); image.style.cssText = 'display:block;width:100%;height:auto'; image.alt = document.title;
  still.replaceWith(image);
  const controls = document.createElement('div'); controls.style.cssText = 'display:flex;gap:12px;justify-content:center;padding:12px';
  const toggle = document.createElement('button'), replay = document.createElement('button');
  const zh = document.documentElement.lang === 'zh';
  toggle.textContent = zh ? '暂停' : 'Pause'; replay.textContent = zh ? '重播' : 'Replay';
  toggle.type = replay.type = 'button'; controls.append(toggle, replay); root.parentElement?.append(controls);
  let elapsed = 0, last = performance.now(), playing = true, previous = -1, raf = 0;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const setFrame = (index: number) => { if (index === previous) return; previous = index; image.src = images[index].src; };
  const draw = (now: number) => {
    const active = playing && !document.hidden && !reduced.matches;
    if (active) elapsed += Math.max(0, now - last);
    last = now;
    const cycle = animation.timeline.durationMs + animation.timeline.holdMs;
    const at = animation.timeline.loop && cycle > 0 ? elapsed % cycle : elapsed;
    const t = reduced.matches || animation.timeline.durationMs <= 0 ? 1 : Math.min(1, at / animation.timeline.durationMs);
    let index = 0; while (index + 1 < animation.times.length && animation.times[index + 1] <= t) index++;
    setFrame(index);
    const hits = root.querySelector<HTMLElement>('.ct-hits'); if (hits) hits.style.visibility = t >= 1 ? 'visible' : 'hidden';
    raf = active ? requestAnimationFrame(draw) : 0;
  };
  const wake = () => { last = performance.now(); if (!raf) raf = requestAnimationFrame(draw); };
  toggle.onclick = () => { playing = !playing; toggle.textContent = playing ? (zh ? '暂停' : 'Pause') : (zh ? '播放' : 'Play'); wake(); };
  replay.onclick = () => { elapsed = 0; playing = true; toggle.textContent = zh ? '暂停' : 'Pause'; wake(); };
  document.addEventListener('visibilitychange', wake); reduced.addEventListener('change', wake);
  Promise.all(images.map(img => img.decode().catch(() => {}))).then(wake);
  setFrame(animation.svgs.length - 1);
  window.addEventListener('pagehide', () => { cancelAnimationFrame(raf); images.forEach(img => URL.revokeObjectURL(img.src)); }, { once: true });
}
