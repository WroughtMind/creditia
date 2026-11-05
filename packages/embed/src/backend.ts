// The browser uses the same request dispatcher, layout code and pinned fonts as
// the desktop and CLI. Only font loading and Satori initialization differ.
import satori, { init } from 'satori/standalone';
import { dispatchRender, type RenderBackend } from '../../render/src/protocol.ts';
import { classicArt, classicFrames } from '../../render/src/classic/art.ts';
import { fontsFor } from '../../render/src/classic/fonts.ts';
import { familiesIn } from '../../render/src/fonts.ts';

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
  layout ??= fetch(new URL('layout.wasm', assets)).then(r => {
    if (!r.ok) throw new Error(`Layout engine: HTTP ${r.status}`);
    return init(r);
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
  if (!svg.includes('<text')) return;
  const all = await faces();
  const wanted = new Set<Face>();
  for (const tag of svg.matchAll(/<text\b[^>]*>/g)) {
    const family = /font-family="([^"]+)"/.exec(tag[0])?.[1] ?? '';
    const weight = Number(/font-weight="(\d+)"/.exec(tag[0])?.[1] ?? 400);
    const italic = /font-style="italic"/.test(tag[0]);
    for (const name of familiesIn(family)) {
      const options = all.filter(f => f.family === name);
      const face = options.sort((a, b) => (Number(a.italic !== italic) * 10000 + Math.abs(a.weight - weight)) - (Number(b.italic !== italic) * 10000 + Math.abs(b.weight - weight)))[0];
      if (face) wanted.add(face);
    }
  }
  await Promise.all([...wanted].map(face => {
    let pending = browserFonts.get(face.file);
    if (!pending) {
      pending = bytes(face).then(async data => {
        const font = new FontFace(face.family, data, { weight: String(face.weight), style: face.italic ? 'italic' : 'normal' });
        await font.load(); document.fonts.add(font);
      }).catch(error => { browserFonts.delete(face.file); throw error; });
      browserFonts.set(face.file, pending);
    }
    return pending;
  }));
}

export async function renderWeb(request: Record<string, unknown>) {
  const response = await dispatchRender({ ...request, version: 1, id: 'web', ...(request.op === 'render' ? { flatten: false } : {}) }, backend);
  if (!response.ok) throw new Error(response.error.message);
  const result = response.result as any;
  if (result.art) await prepareText(result.art.svg);
  if (result.frames?.length) await prepareText(result.frames[result.frames.length - 1].svg);
  return result;
}
