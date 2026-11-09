import type { Art } from '../../../packages/render/src/export.ts';
import { scrollTransforms } from '../../../packages/render/src/scroll-motion.ts';
import { scrollAtlas } from '../../../packages/render/src/scroll-compositor.ts';
import { prepareScrollCanvas } from '../../../packages/render/src/scroll-player.ts';

/** 一次播放只拥有自己的图层、URL 和线程；静态图始终留在下面。 */
export function playFocusMotion(
  root: HTMLElement, art: Art, preview: Blob,
  frames: (signal: AbortSignal) => Promise<Blob[]>, duration: number,
  place: (size: Pick<Art, 'w' | 'h'>) => void, onError: (error: unknown) => void,
  scrollPreview?: (svg: string) => Promise<Blob>,
): () => void {
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  if (reduced.matches) return () => {};
  const controller = new AbortController(), urls: string[] = [];
  let frame = 0, active: HTMLElement | null = null, animation: Animation | null = null, stopped = false;
  let material: Awaited<ReturnType<typeof prepareScrollCanvas>> | null = null;
  const stop = () => {
    if (stopped) return;
    stopped = true;
    controller.abort(); cancelAnimationFrame(frame); animation?.cancel(); animation = null;
    material?.destroy(); material = null;
    active?.remove(); active = null;
    root.classList.remove('motion-playing', 'motion-scroll');
    reduced.removeEventListener('change', preferenceChanged);
    for (const url of urls) URL.revokeObjectURL(url);
    urls.length = 0;
    place(art);
  };
  const preferenceChanged = () => { if (reduced.matches) stop(); };
  reduced.addEventListener('change', preferenceChanged);
  const imageOf = async (blob: Blob) => {
    if (stopped) return null;
    const image = new Image(), url = URL.createObjectURL(blob);
    urls.push(url); image.src = url;
    image.alt = ''; image.draggable = false; image.className = 'focus-motion-frame';
    await image.decode();
    return stopped ? null : image;
  };
  void (async () => {
    if (art.motion?.kind === 'scroll') {
      if (art.motion.material && scrollPreview) {
        const motion = art.motion, atlas = scrollAtlas(art.svg, art.w, art.h, motion);
        const source = await scrollPreview(atlas.svg);
        if (stopped) return;
        const player = await prepareScrollCanvas(source, atlas, motion, Math.ceil((root.clientWidth || motion.viewW) * (globalThis.devicePixelRatio || 1)), controller.signal);
        if (stopped) { player.destroy(); return; }
        material = player;
        const screen = document.createElement('div'); screen.className = 'focus-motion-screen';
        screen.style.aspectRatio = `${motion.viewW} / ${motion.viewH}`; screen.style.background = art.bg;
        screen.appendChild(player.canvas); active = screen;
        root.classList.add('motion-playing', 'motion-scroll'); root.appendChild(screen);
        let start: number | undefined;
        const paint = (now: number) => {
          if (stopped) return;
          start ??= now;
          const p = Math.min(1, (now - start) / (motion.secs * 1000)); player.paint(p);
          if (p < 1) frame = requestAnimationFrame(paint);
          else { stop(); if (root.parentElement) root.parentElement.scrollTop = root.parentElement.scrollHeight; }
        };
        frame = requestAnimationFrame(paint); return;
      }
      const image = await imageOf(preview);
      if (!image || stopped) return;
      const motion = art.motion, screen = document.createElement('div');
      screen.className = 'focus-motion-screen';
      screen.style.aspectRatio = `${motion.viewW} / ${motion.viewH}`;
      screen.style.background = art.bg;
      screen.appendChild(image); active = screen;
      root.classList.add('motion-playing', 'motion-scroll'); root.appendChild(screen);
      animation = image.animate(scrollTransforms(motion.keys, art.h), { duration: motion.secs * 1000, easing: 'linear', fill: 'forwards' });
      animation.onfinish = () => {
        if (stopped) return;
        stop();
        // 完整长图仍可手动浏览；停在刚看到的末尾，取景框始终不变。
        if (root.parentElement) root.parentElement.scrollTop = root.parentElement.scrollHeight;
      };
      return;
    }
    const blobs = await frames(controller.signal);
    if (stopped) return;
    if (blobs.length < 2) throw new Error('Animation did not return enough frames');
    const images = await Promise.all(blobs.map(imageOf));
    if (stopped) return;
    let start: number | undefined, shown = -1;
    const paint = (now: number) => {
      if (stopped) return;
      start ??= now;
      const elapsed = now - start;
      if (elapsed >= duration) { stop(); return; }
      const at = Math.min(images.length - 1, Math.floor(elapsed / duration * images.length));
      if (at !== shown) {
        active?.remove(); active = images[at]!; shown = at;
        root.classList.add('motion-playing'); root.appendChild(active);
      }
      frame = requestAnimationFrame(paint);
    };
    frame = requestAnimationFrame(paint);
  })().catch((error) => {
    if (stopped) return;
    stop(); onError(error);
  });
  return stop;
}
