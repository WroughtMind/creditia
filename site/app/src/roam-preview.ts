import type { Art } from '../../../packages/render/src/export.ts';

/** 视口附近的卡片复用同一 SVG、同一尺寸的画布。 */
type Preview = { users: number; ready: Promise<HTMLCanvasElement | null> };
const previews = new Map<Blob, Map<string, Preview>>();
const mounted = new WeakMap<HTMLElement, () => void>();

function acquire(source: Blob, width: number, height: number) {
  let sizes = previews.get(source);
  if (!sizes) { sizes = new Map(); previews.set(source, sizes); }
  const key = `${width}x${height}`;
  let entry = sizes.get(key);
  if (!entry) {
    const created: Preview = { users: 0, ready: Promise.resolve(null) };
    created.ready = Promise.resolve().then(async () => {
      if (!created.users) return null;
      const url = URL.createObjectURL(source), image = new Image();
      try {
        image.src = url;
        await image.decode();
        if (!created.users) return null;
        const canvas = document.createElement('canvas');
        canvas.width = width; canvas.height = height;
        canvas.getContext('2d')!.drawImage(image, 0, 0, width, height);
        return canvas;
      } finally { URL.revokeObjectURL(url); image.src = ''; }
    });
    sizes.set(key, created); entry = created;
  }
  entry.users++;
  const active = entry;
  return { ready: active.ready, release() {
    if (--active.users) return;
    sizes!.delete(key);
    if (!sizes!.size) previews.delete(source);
    void active.ready.then((canvas) => { if (canvas) canvas.width = canvas.height = 1; }, () => {});
  }};
}

export function releaseRoamPreview(root: HTMLElement) {
  mounted.get(root)?.(); mounted.delete(root);
}

export function mountRoamPreview(root: HTMLElement, art: Art, source: Blob, pixelWidth: number) {
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
  const preview = acquire(source, canvas.width, canvas.height);
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
