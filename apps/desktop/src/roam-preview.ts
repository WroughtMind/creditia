import type { Art } from '../../../packages/render/src/export.ts';

/** 显示画布只保留视口附近的；压缩后的成图跟随作品缓存，回到看过的卡片不重新绘图。 */
type Preview = { priorities: Map<symbol, () => number>; ready: Promise<HTMLCanvasElement | null> };
const previews = new Map<Blob, Map<string, Preview>>();
const rendered = new WeakMap<Blob, { width: number; png: Blob }>();
const mounted = new WeakMap<HTMLElement, () => void>();
type RenderJob = { priority: () => number; run: () => Promise<Blob | null>; resolve: (blob: Blob | null) => void; reject: (error: unknown) => void };
const queue: RenderJob[] = [];
let rendering = false;

function enqueue(run: RenderJob['run'], priority: () => number) {
  const result = new Promise<Blob | null>((resolve, reject) => queue.push({run, priority, resolve, reject}));
  if (!rendering) {
    rendering = true;
    void Promise.resolve().then(async () => {
      while (queue.length) {
        // 每次重新按当前视口排序，滑到新区域时不必等屏幕外的预加载。
        queue.sort((a, b) => a.priority() - b.priority());
        const job = queue.shift()!;
        try { job.resolve(await job.run()); } catch (error) { job.reject(error); }
      }
      rendering = false;
    });
  }
  return result;
}

function acquire(source: Blob, width: number, height: number, priority: () => number) {
  let sizes = previews.get(source);
  if (!sizes) { sizes = new Map(); previews.set(source, sizes); }
  const key = `${width}x${height}`;
  let entry = sizes.get(key);
  if (!entry) {
    const created: Preview = { priorities: new Map(), ready: Promise.resolve(null) };
    created.ready = Promise.resolve().then(async () => {
      const active = () => created.priorities.size > 0;
      if (!active()) return null;
      const native = (globalThis as any).__TAURI__?.core?.invoke;
      let imageSource: Blob | null = source;
      if (native) {
        const cached = rendered.get(source);
        imageSource = cached && cached.width >= width ? cached.png : await enqueue(async () => {
          if (!active()) return null;
          const ready = rendered.get(source);
          if (ready && ready.width >= width) return ready.png;
          const svg = await source.text();
          if (!active()) return null;
          const bytes: ArrayBuffer | number[] = await native('render_preview', {svg, width});
          const png = new Blob([new Uint8Array(bytes)], {type:'image/png'});
          // 即使绘制期间滑走，也保留已完成的成图；源作品释放后它可一同回收。
          rendered.set(source, {width, png});
          return png;
        }, () => Math.min(...[...created.priorities.values()].map((p) => p())));
      }
      if (!active() || !imageSource) return null;
      const url = URL.createObjectURL(imageSource), image = new Image();
      try {
        image.src = url;
        await image.decode();
        if (!active()) return null;
        const canvas = document.createElement('canvas');
        canvas.width = width; canvas.height = height;
        canvas.getContext('2d')!.drawImage(image, 0, 0, width, height);
        return canvas;
      } finally { URL.revokeObjectURL(url); image.src = ''; }
    });
    sizes.set(key, created); entry = created;
  }
  const interest = Symbol();
  entry.priorities.set(interest, priority);
  const active = entry;
  return { ready: active.ready, release() {
    active.priorities.delete(interest);
    if (active.priorities.size) return;
    sizes!.delete(key);
    if (!sizes!.size) previews.delete(source);
    void active.ready.then((canvas) => { if (canvas) canvas.width = canvas.height = 1; }, () => {});
  }};
}

export function releaseRoamPreview(root: HTMLElement) {
  mounted.get(root)?.(); mounted.delete(root);
}

export function mountRoamPreview(root: HTMLElement, art: Art, source: Blob, pixelWidth: number, priority: () => number = () => 0) {
  const canvas = document.createElement('canvas');
  canvas.width = pixelWidth; canvas.height = Math.ceil(pixelWidth * art.h / art.w);
  canvas.setAttribute('role', 'img'); canvas.setAttribute('aria-label', art.title);
  // 调整窗口时先缩放已画好的图，新的清晰度准备好后原位替换。
  const previous = root.querySelector('canvas');
  if (previous?.dataset.ready) {
    canvas.getContext('2d')!.drawImage(previous, 0, 0, canvas.width, canvas.height);
    canvas.dataset.ready = 'true';
  }
  releaseRoamPreview(root);
  root.appendChild(canvas);
  const preview = acquire(source, canvas.width, canvas.height, priority);
  let active = true;
  mounted.set(root, () => { active = false; preview.release(); canvas.width = canvas.height = 1; canvas.remove(); });
  void preview.ready.then((image) => {
    if (active && image) {
      const context = canvas.getContext('2d')!;
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.drawImage(image, 0, 0); canvas.dataset.ready = 'true';
    }
  }).catch((error) => { if (active) console.error('roam preview', error); });
}
