// Environment-independent native render protocol. Browser workers may use the
// same dispatcher with their own Satori, raster and compression implementations.
import { buildModel, type GroupOpt, type Lang } from './data.ts';
import { STYLES } from './styles/index.ts';
import { PIECES, art, frame, frames, variantName } from './registry.ts';
import { FORMS, type Ctx } from './classic/registry.ts';
import { buildModel as classicModel, type Model as ClassicModel } from './classic/data.ts';
import { ORDERED_THEMES, themeName, pieceName, variantName as classicVariantName } from './classic/catalog.ts';
import { variantsOf, defaultVariant, animatedClassic } from './classic/art.ts';
import { BRIDGED, isBridged, bridgeStyle, v2ModelOf } from './classic/bridge.ts';
import type { Theme } from './classic/themes.ts';
import { FORMATS, SIZES, makeExporter, sized, type Art, type Raster, type Deflate } from './export.ts';

export const PROTOCOL_VERSION = 1;
export const MAX_REQUEST_BYTES = 8 * 1024 * 1024;
export const MAX_RESPONSE_BYTES = 64 * 1024 * 1024;
const SKIP_STYLES = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const GRAPHIC_FORMATS = new Set(['png', 'svg', 'pdf', 'gif', 'html']);
const encoder = new TextEncoder();

export interface RenderBackend {
  raster: Raster;
  deflate: Deflate;
  classicArt(id: string, model: ClassicModel, theme: Theme, ctx: Ctx): Promise<Art | null>;
  classicFrames(id: string, model: ClassicModel, theme: Theme, ctx: Ctx, n: number, times?: readonly number[]): Promise<Art[]>;
}

export type RequestId = string | number | null;
export type RenderResponse =
  | { version: 1; id: RequestId; ok: true; result: unknown }
  | { version: 1; id: RequestId; ok: false; error: { code: string; message: string } };

class ProtocolError extends Error {
  code: string;
  constructor(code: string, message: string) { super(message); this.code = code; }
}
const fail = (code: string, message: string): never => { throw new ProtocolError(code, message); };
const object = (x: unknown): x is Record<string, any> => x !== null && typeof x === 'object' && !Array.isArray(x);
const string = (x: unknown, name: string, limit = 128): string =>
  typeof x === 'string' && x.length > 0 && x.length <= limit ? x : fail('invalid_request', `${name} must be a nonempty string of at most ${limit} characters`);

/** Stable catalog IDs are the IDs already used by the desktop and web renderers. */
export function renderCatalog() {
  const classicIds = [...new Set([...Object.keys(FORMS), 'badge', ...BRIDGED])];
  const styles = [
    ...ORDERED_THEMES.map(t => ({
      id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'), renderer: 'classic',
      pieces: classicIds.map(id => ({
        id, name: pieceName(id, 'zh'), en: pieceName(id, 'en'),
        variants: variantsOf(id).map(v => ({ id: v, name: classicVariantName(v, 'zh'), en: classicVariantName(v, 'en') })),
        defaultVariant: defaultVariant(id, t) ?? variantsOf(id)[0] ?? null,
        animated: animatedClassic(id),
        requires: id === 'profile' ? ['projects'] : id === 'blame' ? ['blame'] : [],
      })),
    })),
    ...STYLES.filter(s => !SKIP_STYLES.has(s.id)).map(s => ({
      id: s.id, name: s.name, en: s.en, renderer: 'svg',
      pieces: Object.entries(PIECES).map(([id, p]) => ({
        id, name: p.name, en: p.en,
        variants: (p.variants ?? []).map(v => ({ id: v, name: variantName(v, 'zh'), en: variantName(v, 'en') })),
        defaultVariant: s.prefer[id] ?? p.variants?.[0] ?? null,
        animated: !!p.animated, requires: [],
      })),
    })),
  ];
  return {
    styles, formats: FORMATS.filter(f => GRAPHIC_FORMATS.has(f.id)), sizes: SIZES,
    limits: { requestBytes: MAX_REQUEST_BYTES, responseBytes: MAX_RESPONSE_BYTES, frames: 120 },
    // Text/archive output continues to use the existing CLI/schema modules.
    unsupported: ['transparent-background-override', 'font-embedding-mode'],
  };
}

export function errorResponse(id: RequestId, code: string, message: string): RenderResponse {
  return { version: PROTOCOL_VERSION, id, ok: false, error: { code, message } };
}

/** Never accepts executable code, input SVG, paths or arbitrary output filenames. */
export async function dispatchRender(input: unknown, backend: RenderBackend): Promise<RenderResponse> {
  const id = object(input) && (typeof input.id === 'string' && input.id.length <= 128 || typeof input.id === 'number' && Number.isSafeInteger(input.id)) ? input.id : null;
  try {
    if (!object(input)) fail('invalid_request', 'Request must be an object');
    if (input.version !== PROTOCOL_VERSION) fail('unsupported_version', 'Expected protocol version 1');
    if (id === null) fail('invalid_request', 'id must be a string or safe integer');
    const serialized = JSON.stringify(input);
    if (encoder.encode(serialized).byteLength > MAX_REQUEST_BYTES) fail('request_too_large', 'Request exceeds 8 MiB');
    const op = input.op;
    if (!['catalog', 'render', 'export'].includes(op)) fail('invalid_request', 'Unknown operation');
    const allowed = new Set(op === 'catalog' ? ['version', 'id', 'op'] : ['version', 'id', 'op', 'style', 'piece', 'variant', 'lang', 'group', 'named', 'raw', 'projects', 'person', 'blame', ...(op === 'render' ? ['t', 'flatten'] : ['format', 'size', 'frameCount'])]);
    for (const key of Object.keys(input)) if (!allowed.has(key)) fail('invalid_request', `Unknown option: ${key}`);
    if (op === 'catalog') return { version: 1, id, ok: true, result: renderCatalog() };

    const styleId = string(input.style, 'style'), pieceId = string(input.piece, 'piece');
    const catalog = renderCatalog().styles.find(s => s.id === styleId);
    if (!catalog) fail('unknown_style', `Unknown style: ${styleId}`);
    const piece = catalog.pieces.find(p => p.id === pieceId);
    if (!piece) fail('unknown_piece', `Unknown piece for ${styleId}: ${pieceId}`);
    const variant = input.variant === undefined ? undefined : string(input.variant, 'variant');
    if (variant !== undefined && !piece.variants.some(v => v.id === variant)) fail('unknown_variant', `Unknown variant for ${pieceId}: ${variant}`);
    if (!object(input.raw)) fail('invalid_request', 'raw must be a credits archive object');
    const lang: Lang = input.lang ?? 'zh';
    if (lang !== 'zh' && lang !== 'en') fail('invalid_request', 'lang must be zh or en');
    const group: GroupOpt = input.group ?? 'auto';
    if (!['auto', 'tool', 'model'].includes(group)) fail('invalid_request', 'group must be auto, tool or model');
    const named = input.named ?? true;
    if (typeof named !== 'boolean') fail('invalid_request', 'named must be a boolean');
    const flatten = input.flatten ?? false;
    if (typeof flatten !== 'boolean') fail('invalid_request', 'flatten must be a boolean');
    const t = input.t;
    if (t !== undefined && (typeof t !== 'number' || !Number.isFinite(t) || t < 0 || t > 1)) fail('invalid_request', 't must be between 0 and 1');
    if (input.projects !== undefined && (!Array.isArray(input.projects) || input.projects.length > 50 || input.projects.some(p => !object(p)))) fail('invalid_request', 'projects must be at most 50 credits objects');
    if (input.person !== undefined) string(input.person, 'person', 256);
    if (input.blame !== undefined && (!object(input.blame) || typeof input.blame.file !== 'string' || !Number.isSafeInteger(input.blame.line))) fail('invalid_request', 'blame must include file and line');
    for (const key of piece.requires) if (!input[key] || Array.isArray(input[key]) && input[key].length === 0) fail('missing_data', `${pieceId} requires ${key}`);
    const format = input.format;
    const size = SIZES.find(s => s.id === (input.size ?? 'native'));
    const n = input.frameCount ?? 24;
    if (op === 'export') {
      if (!GRAPHIC_FORMATS.has(format)) fail('invalid_request', 'format must be png, svg, pdf, gif or html');
      if (!size) fail('invalid_request', `Unknown size: ${input.size}`);
      if (!Number.isSafeInteger(n) || n < 1 || n > 120) fail('invalid_request', 'frameCount must be an integer from 1 to 120');
      if (format === 'gif' && !piece.animated) fail('not_animated', 'This piece does not support GIF animation');
    }
    const theme = ORDERED_THEMES.find(x => x.id === styleId);
    const style = theme ? bridgeStyle(theme) : STYLES.find(x => x.id === styleId)!;
    const model = theme ? classicModel(input.raw, { director: false, lang, group, named }) : buildModel(input.raw, { lang, group, named, sign: '' });
    const ctx: Ctx = { lang, variant, person: input.person, blame: input.blame, projects: input.projects?.map(raw => classicModel(raw, { director: false, lang, group, named })) };
    const svgModel = theme ? v2ModelOf(input.raw, lang, (model as ClassicModel).group, named) : model as ReturnType<typeof buildModel>;
    const bridged = !!theme && isBridged(pieceId, variant);
    const render = async (time?: number): Promise<Art | null> => {
      if (!theme || bridged) return time === undefined ? art(pieceId, svgModel, style, { variant }) : frame(pieceId, svgModel, style, { variant }, time);
      if (time === undefined || !piece.animated) return backend.classicArt(pieceId, model as ClassicModel, theme, ctx);
      return (await backend.classicFrames(pieceId, model as ClassicModel, theme, ctx, 24, [time]))[0] ?? null;
    };
    const a = await render(t);
    if (op === 'render') {
      // Match desktop preview semantics: preserve already outlined Satori SVGs.
      if (a && flatten && a.svg.includes('<text')) a.svg = backend.raster.flat(a.svg);
      return bounded({ version: 1, id, ok: true, result: { art: a } });
    }
    if (!a) fail('missing_data', 'This archive does not contain enough data for the selected piece');
    if (format === 'html' && size!.w && a.motion) fail('unsupported_size', 'Scrolling HTML currently requires native size');
    const fit = (x: Art) => size!.w ? sized(x, size!.w, size!.h) : x;
    const ex = makeExporter(boundedRaster(backend.raster), backend.deflate);
    let bytes: Uint8Array, mime: string;
    switch (format) {
      case 'png': bytes = ex.png(fit(a), size!.w ? 1 : 2); mime = 'image/png'; break;
      case 'svg': bytes = encoder.encode(ex.svg(fit(a))); mime = 'image/svg+xml'; break;
      case 'pdf': bytes = await ex.pdf(fit(a), size!.id === 'a4' ? 1 : Math.min(3, 4000 / Math.max(a.w, a.h))); mime = 'application/pdf'; break;
      case 'html': bytes = encoder.encode(ex.html([fit(a)], { title: a.title, lang })); mime = 'text/html'; break;
      case 'gif': {
        if (!piece.animated) fail('not_animated', 'This piece does not support GIF animation');
        const generated = !theme || bridged ? frames(pieceId, svgModel, style, { variant }, n) : await backend.classicFrames(pieceId, model as ClassicModel, theme, ctx, n);
        if (!generated.length) fail('missing_data', 'This archive does not contain animation frames');
        const fitted = generated.map(fit);
        bytes = ex.gif(fitted, { width: Math.min(fitted[0].w, 800) }); mime = 'image/gif'; break;
      }
      default: return fail('invalid_request', 'Unsupported export format');
    }
    // Base64 keeps the transport newline-safe. It is also independent of Node Buffer.
    if (bytes.byteLength > MAX_RESPONSE_BYTES * 0.74) fail('response_too_large', 'Export exceeds transport limit');
    let binary = '';
    for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return bounded({ version: 1, id, ok: true, result: { bytes: btoa(binary), encoding: 'base64', mime, ext: format } });
  } catch (error) {
    return errorResponse(id, error instanceof ProtocolError ? error.code : 'render_failed', error instanceof Error ? error.message : String(error));
  }
}

function bounded(response: RenderResponse): RenderResponse {
  if (encoder.encode(JSON.stringify(response)).byteLength > MAX_RESPONSE_BYTES) fail('response_too_large', 'Response exceeds 64 MiB');
  return response;
}

/** A large archive may produce a very tall artwork; refuse a bitmap that cannot
 * fit the renderer budget before asking native resvg to allocate it. */
function boundedRaster(raster: Raster): Raster {
  const check = (svg: string, width?: number) => {
    const root = /^<svg\b[^>]*>/.exec(svg)?.[0] ?? '';
    const w = Number(/\bwidth="([\d.]+)"/.exec(root)?.[1]);
    const h = Number(/\bheight="([\d.]+)"/.exec(root)?.[1]);
    const outW = width ?? w, outH = h * outW / w;
    if (![w, h, outW, outH].every(x => Number.isFinite(x) && x > 0) || outW > 32768 || outH > 32768 || outW * outH > 32_000_000) fail('raster_too_large', 'Bitmap exceeds 32 million pixels; choose a smaller size or vector output');
  };
  return {
    png(svg, width) { check(svg, width); return raster.png(svg, width); },
    rgba(svg, width) { check(svg, width); return raster.rgba(svg, width); },
    flat: svg => raster.flat(svg),
  };
}
