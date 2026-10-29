// 作品排版和 SVG 生成放在独立线程；主线程继续处理画布移动和指针。
import * as C from '../../../packages/render/src/classic/data.ts';
import * as V from '../../../packages/render/src/data.ts';
import { themeById } from '../../../packages/render/src/classic/themes.ts';
import { prepare, warm, render as classicRender, frames as classicFrames } from '../../../packages/render/src/classic/browser.ts';
import { STYLES } from '../../../packages/render/src/styles/index.ts';
import { art as v2Art, frames as v2Frames } from '../../../packages/render/src/registry.ts';
import { fontFiles, fontBytes } from './fonts.ts';
import { bridgeStyle } from '../../../packages/render/src/classic/bridge.ts';
import { Resvg, initWasm } from '@resvg/resvg-wasm';
import wasmUrl from '@resvg/resvg-wasm/index_bg.wasm?url';

prepare(fontFiles());
let rasterReady: Promise<void> | undefined;
let raw: any;
let modelKey: [any, string, string, boolean, boolean] | undefined;
let model: any;
/** 预览作为独立图片交给浏览器绘制。这里只把文字转路径，保留原图的滤镜和细节；
 *  对整张图运行 resvg.render() 会让纸纹等滤镜占住排版队列数秒。 */
async function preview(svg: string, style: string, classic: boolean) {
  if (!svg.includes('<text')) return new Blob([svg], {type:'image/svg+xml'});
  await (rasterReady ??= initWasm(fetch(wasmUrl)));
  const styleSpec = classic ? bridgeStyle(themeById(style)!) : STYLES.find((s) => s.id === style)!;
  const fonts = await fontBytes(styleSpec, svg);
  const image = new Resvg(svg, {font:{fontBuffers:fonts,loadSystemFonts:false,defaultFontFamily:'Noto Sans SC'}});
  try {
    return new Blob([image.toString()], {type:'image/svg+xml'});
  } finally { image.free(); }
}
self.onmessage = async ({ data }) => {
  const { seq, style, lang, group, named, id, variant, classic } = data;
  try {
    if (data.warm) { await warm(themeById(style)!); return; }
    if (data.svg) { self.postMessage({ seq, preview: await preview(data.svg, style, classic) }); return; }
    if ('raw' in data) raw = data.raw;
    const current = raw;
    if (!modelKey || modelKey[0] !== current || modelKey[1] !== lang || modelKey[2] !== group || modelKey[3] !== classic || modelKey[4] !== named) {
      model = classic ? C.buildModel(current, { director: false, lang, group, named }) : V.buildModel(current, { lang, named, sign: '', group });
      modelKey = [current, lang, group, classic, named];
    }
    if (data.frames) {
      const frames = classic ? await classicFrames(id, model, themeById(style)!, { lang, variant }, 24) : v2Frames(id, model, STYLES.find((s) => s.id === style)!, { variant }, 24);
      // 独立动画任务按顺序转路径，避免同时持有多份 WASM 树。
      const output = [];
      for (const frame of frames) output.push(data.previews ? await preview(frame.svg, style, classic) : frame);
      self.postMessage({ frames: output });
      return;
    }
    const a = classic ? await classicRender(id, model, themeById(style)!, { lang, variant }) : v2Art(id, model, STYLES.find((s) => s.id === style)!, { variant });
    self.postMessage({ seq, art: a });
  } catch (e) { self.postMessage({ seq, error: e instanceof Error ? e.message : String(e) }); }
};
