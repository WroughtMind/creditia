// 浏览器端：satori standalone 出 SVG（文字已转成路径）。字体按主题取：App 给出字体文件的地址，用到哪个取哪个。
import satori, { init as initSatori } from 'satori/standalone';
import layoutUrl from 'satori/layout.wasm?url';
import { classicArt } from './art.ts';
import { fontsFor } from './fonts.ts';
import type { Ctx } from './registry.ts';
import type { Model } from './data.ts';
import type { Theme } from './themes.ts';

let files: { family: string; weight: number; url: string }[] = [];
let boot: Promise<void> | null = null;
const loaded = new Map<string, Promise<{ name: string; weight: number; data: ArrayBuffer }>>();

/** files：{ family, weight, url } 列表 */
export function prepare(list: { family: string; weight: number; url: string }[]) {
  files = list;
  boot ??= fetch(layoutUrl).then((r) => initSatori(r));
  return boot;
}

function fontsOf(t: Theme) {
  return Promise.all(fontsFor(t).map((f) => {
    const k = `${f.family}@${f.weight}`;
    const file = files.find((x) => x.family === f.family && x.weight === f.weight);
    if (!file) return null;
    if (!loaded.has(k)) loaded.set(k, fetch(file.url).then((r) => r.arrayBuffer()).then((data) => ({ name: f.family, weight: f.weight, data })));
    return loaded.get(k)!;
  })).then((xs) => xs.filter(Boolean));
}

export async function render(id: string, m: Model, t: Theme, ctx: Ctx) {
  await boot;
  return classicArt(id, m, t, ctx, satori as any, await fontsOf(t));
}
