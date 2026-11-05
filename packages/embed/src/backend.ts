// The browser uses the same request dispatcher, layout code and pinned fonts as
// the desktop and CLI. Only font loading and Satori initialization differ.
import satori, { init } from 'satori/standalone';
import { dispatchRender, type RenderBackend } from '../../render/src/protocol.ts';
import { classicArt, classicFrames } from '../../render/src/classic/art.ts';
import { fontsFor } from '../../render/src/classic/fonts.ts';
import { nativeFonts } from '../../render/src/fonts.ts';
import { fontKey, portableTextFonts, portableFontBindings } from '../../render/src/portable-text.ts';

interface Face { family: string; weight: number; italic: boolean; file: string }
const assets = new URL('./', import.meta.url);
let manifest: Promise<Face[]> | undefined;
let layout: Promise<unknown> | undefined;
const buffers = new Map<string, Promise<ArrayBuffer>>();
const browserFonts = new Map<string, Promise<void>>();
function faces() {
  return manifest ??= fetch(new URL('fonts/manifest.json', assets)).then(async r => {
    if (!r.ok) throw new Error(`Font manifest: HTTP ${r.status}`);
    return await r.json() as Face[];
  }).catch(error => { manifest = undefined; throw error; });
}
function bytes(face: Face) {
  let pending = buffers.get(face.file);
  if (!pending) {
    pending = fetch(new URL(`fonts/${face.file}`, assets)).then(r => {
      if (!r.ok) throw new Error(`Font ${face.family}: HTTP ${r.status}`);
      return r.arrayBuffer();
    }).catch(error => { buffers.delete(face.file); throw error; });
    buffers.set(face.file, pending);
  }
  return pending;
}
async function prepare(theme: Parameters<typeof fontsFor>[0]) {
  layout ??= fetch(new URL('layout.wasm', assets)).then(async r => {
    if (!r.ok) throw new Error(`Layout engine: HTTP ${r.status}`);
    return init(await r.arrayBuffer());
  }).catch(error => { layout = undefined; throw error; });
  const all = await faces();
  const fonts = await Promise.all(fontsFor(theme).map(async spec => {
    const face = all.find(f => f.family === spec.family && f.weight === spec.weight && !f.italic);
    if (!face) throw new Error(`Missing packaged font: ${spec.family} ${spec.weight}`);
    return { name: face.family, weight: face.weight, data: await bytes(face) };
  }));
  await layout;
  return fonts;
}
const noExport = (): never => { throw new Error('Use a Creditia export for offline graphic files'); };
const backend: RenderBackend = {
  raster: { png: noExport, rgba: noExport, flat: noExport },
  deflate: noExport,
  classicArt: async (id, model, theme, ctx) => classicArt(id, model, theme, ctx, satori as any, await prepare(theme)),
  classicFrames: async (id, model, theme, ctx, n, times) => classicFrames(id, model, theme, ctx, satori as any, await prepare(theme), n, times),
};

/** Install the exact font bytes referenced by SVG text before publishing it. */
async function prepareText(svg: string) {
  if (!svg.includes('<text')) return svg;
  const all = await faces();
  const loaded = new Map<string, ArrayBuffer>();
  await Promise.all(nativeFonts(svg).map(async spec => {
    const face = all.find(f => f.family === spec.family && f.weight === spec.weight && f.italic === !!spec.italic);
    if (!face) throw new Error(`Missing packaged font: ${spec.family} ${spec.weight}`);
    loaded.set(fontKey(spec), await bytes(face));
  }));
  const display = portableTextFonts(svg,loaded);
  await Promise.all(portableFontBindings(display).map(binding => {
    const face = all.find(f => f.family === binding.face.family && f.weight === binding.face.weight && f.italic === !!binding.face.italic);
    if (!face) throw new Error(`Missing display font: ${binding.family}`);
    const key = `${binding.family}|${face.file}`;
    let pending = browserFonts.get(key);
    if (!pending) {
      pending = bytes(face).then(async data => {
        const font = new FontFace(binding.family, data, { weight: String(face.weight), style: face.italic ? 'italic' : 'normal' });
        await font.load(); document.fonts.add(font);
      }).catch(error => { browserFonts.delete(key); throw error; });
      browserFonts.set(key, pending);
    }
    return pending;
  }));
  return display;
}

export async function renderWeb(request: Record<string, unknown>) {
  const response = await dispatchRender({ ...request, version: 1, id: 'web', ...(request.op === 'render' ? { flatten: false } : {}) }, backend);
  if (!response.ok) throw new Error(response.error.message);
  const result = response.result as any;
  // Preserve canonical data for hosts and parity checks; display uses the same
  // resolved fallback faces as native usvg instead of browser font synthesis.
  if (result.art) result.displayArt = {...result.art, svg:await prepareText(result.art.svg)};
  if (result.frames?.length) result.displayFrames = await Promise.all(result.frames.map(async (art: any) => ({...art,svg:await prepareText(art.svg)})));
  return result;
}
