// 作品排版和 SVG 生成放在独立线程；主线程继续处理画布移动和指针。
import * as C from '../../../packages/render/src/classic/data.ts';
import * as V from '../../../packages/render/src/data.ts';
import { themeById } from '../../../packages/render/src/classic/themes.ts';
import { prepare, warm, render as classicRender } from '../../../packages/render/src/classic/browser.ts';
import { STYLES } from '../../../packages/render/src/styles/index.ts';
import { art as v2Art } from '../../../packages/render/src/registry.ts';
import { fontFiles, fontBytes } from './fonts.ts';
import { bridgeStyle } from '../../../packages/render/src/classic/bridge.ts';
import { Resvg, initWasm } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';

prepare(fontFiles());
let rasterReady: Promise<void> | undefined;
let raw: any;
let modelKey: [any, string, string, boolean] | undefined;
let model: any;
/** 漫游要的缩略图：展墙直接摆 SVG，用不着，所以单独来要 */
async function rasterize(svg: string, style: string, classic: boolean, width: number) {
  await (rasterReady ??= initWasm(fetch(wasmUrl)));
  const styleSpec = classic ? bridgeStyle(themeById(style)!) : STYLES.find((s) => s.id === style)!;
  const fonts = svg.includes('<text') ? await fontBytes(styleSpec, svg) : [];
  const image = new Resvg(svg, {font:{fontBuffers:fonts,loadSystemFonts:false,defaultFontFamily:'Noto Sans SC'},fitTo:{mode:'width',value:width}});
  try {
    const rendered = image.render();
    try { return new Blob([rendered.asPng()], {type:'image/png'}); } finally { rendered.free(); }
  } finally { image.free(); }
}
self.onmessage = async ({ data }) => {
  const { seq, style, lang, group, id, variant, classic } = data;
  try {
    if (data.warm) { await warm(themeById(style)!); return; }
    if (data.svg) { self.postMessage({ seq, preview: await rasterize(data.svg, style, classic, data.width) }); return; }
    if ('raw' in data) raw = data.raw;
    const current = raw;
    if (!modelKey || modelKey[0] !== current || modelKey[1] !== lang || modelKey[2] !== group || modelKey[3] !== classic) {
      model = classic ? C.buildModel(current, { director: false, lang, group }) : V.buildModel(current, { lang, named: false, sign: '', group });
      modelKey = [current, lang, group, classic];
    }
    const a = classic ? await classicRender(id, model, themeById(style)!, { lang, variant }) : v2Art(id, model, STYLES.find((s) => s.id === style)!, { variant });
    self.postMessage({ seq, art: a });
  } catch (e) { self.postMessage({ seq, error: e instanceof Error ? e.message : String(e) }); }
};
