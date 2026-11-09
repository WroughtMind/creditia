// 只缓存已完成的放大动画；进行中的任务仍归当前视图所有，可立即取消。
const completed = new WeakMap<object, Map<string, Blob[]>>();
const MAX_BYTES = 32 * 1024 * 1024;

export async function cachedMotion(raw: object, key: string, signal: AbortSignal, generate: () => Promise<Blob[]>): Promise<Blob[]> {
  if (signal.aborted) throw new DOMException('Animation cancelled', 'AbortError');
  const cache = completed.get(raw) ?? new Map<string, Blob[]>();
  completed.set(raw, cache);
  const hit = cache.get(key);
  if (hit) { cache.delete(key); cache.set(key, hit); return hit; }
  const frames = await generate();
  if (signal.aborted) throw new DOMException('Animation cancelled', 'AbortError');
  if (frames.length < 2) return frames;
  const bytes = (items: Blob[]) => items.reduce((sum, item) => sum + item.size, 0);
  if (bytes(frames) > MAX_BYTES) return frames;
  cache.set(key, frames);
  while (Array.from(cache.values()).reduce((sum, items) => sum + bytes(items), 0) > MAX_BYTES) cache.delete(cache.keys().next().value!);
  return frames;
}
