// 浏览器端渲染：satori 出 SVG，resvg-wasm 出 PNG。和 CLI 共用 registry 里的同一套 forms。
// 浏览器里要用 standalone 构建，自己把 layout 引擎喂进去；Node 构建会去碰 Buffer/fs，跑不了。
import satori, { init as initSatori } from 'satori/standalone';
import layoutUrl from 'satori/layout.wasm?url';
import { initWasm, Resvg } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';
import { FORMS, type Ctx } from '../../../packages/render/src/registry.ts';
import { useLogoSource, BRANDS } from '../../../packages/render/src/brand.ts';
import type { Model } from '../../../packages/render/src/data.ts';
import type { Theme } from '../../../packages/render/src/themes.ts';

/** 同一页面会内联多张 satori 出的 SVG，它们的 clipPath/mask id 都是 satori_ 开头，会互相顶掉；每张换成独有前缀 */
let seq = 0;
const uniq = (svg: string) => { const prefix = `s${(seq++).toString(36)}_`; return svg.replace(/satori_/g, prefix); };

let fonts: { name: string; weight: 400 | 500 | 600 | 700 | 900; data: ArrayBuffer }[] | null = null;
let boot: Promise<void> | null = null;

export function ready() { boot ??= init(); return boot; }
async function init() {
  await initSatori(await fetch(layoutUrl));
  const { FONT_FILES } = await import('./fonts.ts');
  const loaded = await Promise.all(FONT_FILES.map(async (f) => ({ name: f.name, weight: f.weight, data: await (await fetch(f.url)).arrayBuffer() })));
  fonts = loaded;
  // 给同页的 DOM 也注册一份，内联 SVG 的文字就能用同一批字体渲染
  for (const f of loaded) { try { (document as any).fonts.add(new FontFace(f.name, f.data, { weight: String(f.weight) })); } catch { /* 有的环境不给加 */ } }
  const names = [...new Set(Object.values(BRANDS).map((b) => b.logo).filter(Boolean))] as string[];
  const cache = new Map<string, string | null>();
  await Promise.all(names.map(async (n) => { try { cache.set(n, await (await fetch(`/logos/${n}.svg`)).text()); } catch { cache.set(n, null); } }));
  useLogoSource({ svg: (n) => cache.get(n) ?? null });

}
const fontList = () => fonts!.map((f) => ({ name: f.name, weight: f.weight, data: f.data }));

export async function renderSvg(form: string, m: Model, t: Theme, ctx: Ctx): Promise<string | null> {
  await ready();
  const out = FORMS[form].render(m, t, ctx);
  if (!out) return null;
  return uniq(await satori(out.tree, { width: out.w, height: out.h, fonts: fontList() }));
}

/** 社交尺寸：把形式居中放进指定画布 */
export async function renderSized(form: string, m: Model, t: Theme, w: number, h: number, ctx: Ctx): Promise<string | null> {
  await ready();
  const out = FORMS[form].render(m, t, ctx);
  if (!out) return null;
  const scale = Math.min((w * 0.92) / out.w, (h * 0.92) / out.h);
  const bg = t.bg;
  const tree: any = { type: 'div', props: { style: { display: 'flex', width: w, height: h, alignItems: 'center', justifyContent: 'center', backgroundColor: bg }, children: { type: 'div', props: { style: { display: 'flex', transform: `scale(${scale})` }, children: out.tree } } } };
  return uniq(await satori(tree, { width: w, height: h, fonts: fontList() }));
}

let wasm: Promise<void> | null = null;
export async function toRaster(svg: string, width?: number) {
  await ready();
  wasm ??= initWasm(fetch(wasmUrl)).then(() => undefined);
  await wasm;
  const r = new Resvg(svg, { fitTo: width ? { mode: 'width', value: width } : undefined,
    font: { fontBuffers: fonts!.map(f => new Uint8Array(f.data)), loadSystemFonts: false } });
  const rendered = r.render();
  const result = { png: rendered.asPng().slice(), rgba: rendered.pixels.slice(), width: rendered.width, height: rendered.height };
  rendered.free(); r.free();
  return result;
}
export async function toPng(svg: string, width?: number): Promise<Uint8Array> { return (await toRaster(svg, width)).png; }
