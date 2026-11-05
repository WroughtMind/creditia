// One platform-neutral request contract for native, browser and CLI rendering.
import { buildModel, type GroupOpt, type Lang } from './data.ts';
import { STYLES } from './styles/index.ts';
import { PIECES, art, frame, variantName } from './registry.ts';
import { FORMS, type Ctx } from './classic/registry.ts';
import { buildModel as classicModel, type Model as ClassicModel } from './classic/data.ts';
import { ORDERED_THEMES, themeName, themeDot, pieceName, pieceQ, variantName as classicVariantName } from './classic/catalog.ts';
import { variantsOf, defaultVariant, animatedClassic } from './classic/art.ts';
import { BRIDGED, isBridged, bridgeStyle, v2ModelOf } from './classic/bridge.ts';
import type { Theme } from './classic/themes.ts';
import type { Style } from './style.ts';
import { FORMATS, SIZES, type Art, type Raster, type Deflate } from './export.ts';
import { exportArtwork, exportSize, encoded, type GraphicFormat } from './export-job.ts';
import { timelineFor, sampleTimes, type Animation } from './timeline.ts';
import { CLASSIC_BOARD, V2_BOARD } from './boards.ts';
import { creditsBlock, fetchSegs, embedSnippet } from './text.ts';
import { mergeCredits } from './merge.ts';

export const PROTOCOL_VERSION = 1;
export const MAX_REQUEST_BYTES = 8 * 1024 * 1024;
export const MAX_RESPONSE_BYTES = 64 * 1024 * 1024;
const SKIP_STYLES = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const GRAPHIC_FORMATS = new Set(['png', 'svg', 'pdf', 'gif', 'html']);
const TEXT_FORMATS = new Set(['md', 'txt', 'json', 'embed']);
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
class ProtocolError extends Error { readonly code: string; constructor(code: string, message: string) { super(message); this.code = code; } }
const fail = (code: string, message: string): never => { throw new ProtocolError(code, message); };
const object = (x: unknown): x is Record<string, any> => x !== null && typeof x === 'object' && !Array.isArray(x);
const string = (x: unknown, name: string, limit = 128): string =>
  typeof x === 'string' && x.length > 0 && x.length <= limit ? x : fail('invalid_request', `${name} must be a nonempty string of at most ${limit} characters`);

/** Bounded, restartable session. Eviction is explicit: clients re-register archives. */
export class RenderSession {
  private archives = new Map<string, { raw: any; bytes: number }>();
  private archiveBytes = 0;
  private models = new Map<string, any>();
  private pictures = new Map<string, { value: Art | null; bytes: number }>();
  private pictureBytes = 0;
  private rawIds = new WeakMap<object, number>();
  private seq = 0;
  readonly backend: RenderBackend;
  constructor(backend: RenderBackend) { this.backend = backend; }
  dispatch(input: unknown) { return dispatchRender(input, this.backend, this); }
  register(id: string, raw: any) {
    const bytes = encoder.encode(JSON.stringify(raw)).byteLength;
    this.release(id);
    while (this.archives.size >= 8 || this.archiveBytes + bytes > 24 * 1024 * 1024) this.release(this.archives.keys().next().value!);
    this.archives.set(id, { raw: structuredClone(raw), bytes }); this.archiveBytes += bytes;
  }
  release(id: string) {
    const item = this.archives.get(id); if (!item) return;
    this.archiveBytes -= item.bytes; this.archives.delete(id);
    // Models and artwork can reference the released raw. Clear them immediately.
    this.models.clear(); this.pictures.clear(); this.pictureBytes = 0;
  }
  archive(id: string) {
    const item = this.archives.get(id); if (!item) fail('unknown_archive', `Archive was not registered or was evicted: ${id}`);
    this.archives.delete(id); this.archives.set(id, item); return item.raw;
  }
  identity(raw: object) { let id = this.rawIds.get(raw); if (!id) { id = ++this.seq; this.rawIds.set(raw, id); } return id; }
  model(key: string, make: () => any) {
    if (this.models.has(key)) { const value = this.models.get(key); this.models.delete(key); this.models.set(key, value); return value; }
    const value = make(); while (this.models.size >= 16) this.models.delete(this.models.keys().next().value!);
    this.models.set(key, value); return value;
  }
  async picture(key: string, make: () => Promise<Art | null>): Promise<Art | null> {
    const hit = this.pictures.get(key);
    if (hit) { this.pictures.delete(key); this.pictures.set(key, hit); return structuredClone(hit.value); }
    const value = await make(), bytes = encoder.encode(JSON.stringify(value)).byteLength;
    while (this.pictures.size && (this.pictures.size >= 64 || this.pictureBytes + bytes > 32 * 1024 * 1024)) {
      const first = this.pictures.keys().next().value!; this.pictureBytes -= this.pictures.get(first)!.bytes; this.pictures.delete(first);
    }
    if (bytes <= 32 * 1024 * 1024) { this.pictures.set(key, { value: structuredClone(value), bytes }); this.pictureBytes += bytes; }
    return value;
  }
  stats() { return { archives: this.archives.size, archiveBytes: this.archiveBytes, models: this.models.size, artworks: this.pictures.size, artworkBytes: this.pictureBytes }; }
}
export const createRenderSession = (backend: RenderBackend) => new RenderSession(backend);

let catalogCache: any;
export function renderCatalog() {
  if (catalogCache) return structuredClone(catalogCache);
  const classicIds = [...new Set([...Object.keys(FORMS), 'badge', ...BRIDGED])];
  const sharedStyles = STYLES.map(s => ({
    id: s.id, name: s.name, en: s.en, renderer: 'svg', visible: !SKIP_STYLES.has(s.id),
    dot: s.dot(), colors: [s.c.bg, s.c.ink, s.c.accent], board: V2_BOARD,
    pieces: Object.entries(PIECES).map(([id, p]) => ({ id, name: p.name, en: p.en, q: p.q, qEn: p.qEn,
      variants: (p.variants ?? []).map(v => ({ id: v, name: variantName(v, 'zh'), en: variantName(v, 'en') })),
      defaultVariant: s.prefer[id] ?? p.variants?.[0] ?? null, animated: !!p.animated, requires: [],
    })),
  }));
  const styles = [
    ...ORDERED_THEMES.map(t => ({ id: t.id, name: themeName(t, 'zh'), en: themeName(t, 'en'), renderer: 'classic', visible: true,
      dot: themeDot(t), colors: [t.bg, t.ink, t.accent], board: CLASSIC_BOARD,
      pieces: classicIds.map(id => ({ id, name: pieceName(id, 'zh'), en: pieceName(id, 'en'), q: pieceQ(id, 'zh'), qEn: pieceQ(id, 'en'),
        variants: variantsOf(id).map(v => ({ id: v, name: classicVariantName(v, 'zh'), en: classicVariantName(v, 'en') })),
        defaultVariant: defaultVariant(id, t) ?? variantsOf(id)[0] ?? null, animated: animatedClassic(id),
        requires: id === 'profile' ? ['projects'] : id === 'blame' ? ['blame'] : [],
      })),
    })), ...sharedStyles.filter(s => s.visible),
  ];
  catalogCache = { styles, sharedStyles,
    formats: [...FORMATS, { id: 'txt', name: 'Text', note: 'Plain text credits' }, { id: 'json', name: 'JSON', note: 'Complete credits archive' }], sizes: SIZES,
    fontModes: ['outline', 'embed'], transparency: true,
    limits: { requestBytes: MAX_REQUEST_BYTES, responseBytes: MAX_RESPONSE_BYTES, frames: 120, archives: 8, archiveBytes: 24 * 1024 * 1024, artworkBytes: 32 * 1024 * 1024 }, unsupported: [],
  };
  return structuredClone(catalogCache);
}
export const errorResponse = (id: RequestId, code: string, message: string): RenderResponse => ({ version: 1, id, ok: false, error: { code, message } });
const success = (id: RequestId, result: unknown): RenderResponse => bounded({ version: 1, id, ok: true, result });

/** Never accepts executable code, input SVG, local paths or output filenames. */
export async function dispatchRender(input: unknown, backend: RenderBackend, session?: RenderSession): Promise<RenderResponse> {
  const id = object(input) && (typeof input.id === 'string' && input.id.length <= 128 || typeof input.id === 'number' && Number.isSafeInteger(input.id)) ? input.id : null;
  try {
    if (!object(input)) fail('invalid_request', 'Request must be an object');
    if (input.version !== 1) fail('unsupported_version', 'Expected protocol version 1');
    if (id === null) fail('invalid_request', 'id must be a string or safe integer');
    if (encoder.encode(JSON.stringify(input)).byteLength > MAX_REQUEST_BYTES) fail('request_too_large', 'Request exceeds 8 MiB');
    const op = input.op;
    const simple: Record<string, string[]> = { catalog: [], status: [], archive: ['archiveId', 'raw'], release: ['archiveId'], merge: ['projects', 'lang'] };
    const common = ['style', 'renderer', 'piece', 'variant', 'lang', 'group', 'named', 'sign', 'raw', 'archiveId', 'projects', 'person', 'blame', 'transparent', 'fontMode', 'reducedMotion'];
    const extra = op === 'render' ? ['t', 'flatten'] : op === 'frames' ? ['frameCount', 'flatten'] : op === 'export' ? ['format', 'size', 'frameCount', 'scale', 'width', 'by'] : simple[op];
    if (!extra) fail('invalid_request', 'Unknown operation');
    const allowed = new Set(['version', 'id', 'op', ...(simple[op] ? extra : [...common, ...extra])]);
    for (const key of Object.keys(input)) if (!allowed.has(key)) fail('invalid_request', `Unknown option: ${key}`);
    if (op === 'catalog') return success(id, renderCatalog());
    if (op === 'status') return success(id, session?.stats() ?? { archives: 0, models: 0, artworks: 0 });
    if (op === 'archive' || op === 'release') {
      if (!session) fail('session_required', 'Archive registration requires a renderer session');
      const archiveId = string(input.archiveId, 'archiveId', 256);
      if (op === 'archive') { if (!object(input.raw)) fail('invalid_request', 'raw must be an archive object'); session.register(archiveId, input.raw); }
      else session.release(archiveId);
      return success(id, { archiveId });
    }
    const lang: Lang = input.lang ?? 'zh';
    if (lang !== 'zh' && lang !== 'en') fail('invalid_request', 'lang must be zh or en');
    if (input.projects !== undefined && (!Array.isArray(input.projects) || input.projects.length > 50 || input.projects.some(p => !object(p)))) fail('invalid_request', 'projects must be at most 50 credits objects');
    if (op === 'merge') {
      if (!input.projects?.length) fail('missing_data', 'merge requires at least one project');
      return success(id, { raw: mergeCredits(input.projects.map((raw, i) => ({ name: raw.project?.name ?? `Project ${i + 1}`, raw })), lang) });
    }
    if (input.raw !== undefined && input.archiveId !== undefined) fail('invalid_request', 'Use raw or archiveId, not both');
    const raw = input.archiveId !== undefined ? session?.archive(string(input.archiveId, 'archiveId', 256)) : input.raw;
    if (!object(raw)) fail(input.archiveId !== undefined ? 'unknown_archive' : 'invalid_request', 'A registered archive or raw object is required');
    const group: GroupOpt = input.group ?? 'auto';
    if (!['auto', 'tool', 'model'].includes(group)) fail('invalid_request', 'group must be auto, tool or model');
    const named = input.named ?? true, transparent = input.transparent ?? false, reducedMotion = input.reducedMotion ?? false, flatten = input.flatten ?? false;
    for (const [name, value] of Object.entries({ named, transparent, reducedMotion, flatten })) if (typeof value !== 'boolean') fail('invalid_request', `${name} must be a boolean`);
    const fontMode = input.fontMode ?? 'outline';
    if (!['outline', 'embed'].includes(fontMode)) fail('invalid_request', 'fontMode must be outline or embed');
    const sign = input.sign ?? ''; if (typeof sign !== 'string' || sign.length > 256) fail('invalid_request', 'sign must be at most 256 characters');
    const modelKey = `${session?.identity(raw)}|${lang}|${group}|${named}|${sign}`;
    const getModel = (suffix: string, make: () => any) => session ? session.model(modelKey + suffix, make) : make();
    const textModel = () => getModel('|svg', () => buildModel(raw, { lang, group, named, sign }));
    const format = input.format;
    if (op === 'export' && TEXT_FORMATS.has(format) && format !== 'embed') {
      if (input.by !== undefined && !['tool', 'contributor'].includes(input.by)) fail('invalid_request', 'by must be tool or contributor');
      const view = input.by ?? 'tool';
      const value = format === 'json' ? JSON.stringify(raw, null, 2) + '\n'
        : format === 'md' ? creditsBlock(textModel(), 10, view) + '\n'
        : fetchSegs(textModel(), view).map(line => line.map(segment => segment.text).join('')).join('\n') + '\n';
      return output(id, encoder.encode(value), format === 'json' ? 'application/json' : format === 'md' ? 'text/markdown' : 'text/plain', format);
    }
    const styleId = string(input.style, 'style'), pieceId = string(input.piece, 'piece');
    if (input.renderer !== undefined && !['classic', 'svg'].includes(input.renderer)) fail('invalid_request', 'renderer must be classic or svg');
    const all = catalogCache ?? renderCatalog();
    const styleEntry = (input.renderer === 'svg' ? all.sharedStyles : all.styles).find(s => s.id === styleId && (!input.renderer || s.renderer === input.renderer));
    if (!styleEntry) fail('unknown_style', `Unknown style: ${styleId}`);
    const piece = styleEntry.pieces.find(p => p.id === pieceId);
    if (!piece) fail('unknown_piece', `Unknown piece for ${styleId}: ${pieceId}`);
    const variant = input.variant === undefined ? undefined : string(input.variant, 'variant');
    if (variant !== undefined && !piece.variants.some(v => v.id === variant)) fail('unknown_variant', `Unknown variant for ${pieceId}: ${variant}`);
    if (op === 'export' && format === 'embed') return output(id, encoder.encode(embedSnippet(pieceId, styleId, '/.creditia/credits.json', variant,
      { renderer: styleEntry.renderer, lang, group, named, ...(sign ? { sign } : {}), ...(transparent ? { transparent } : {}) })), 'text/plain', 'html');
    const t = input.t;
    if (t !== undefined && (typeof t !== 'number' || !Number.isFinite(t) || t < 0 || t > 1)) fail('invalid_request', 't must be between 0 and 1');
    if (input.person !== undefined) string(input.person, 'person', 256);
    if (input.blame !== undefined && (!object(input.blame) || typeof input.blame.file !== 'string' || !Number.isSafeInteger(input.blame.line))) fail('invalid_request', 'blame must include file and line');
    for (const key of piece.requires) if (!input[key] || Array.isArray(input[key]) && !input[key].length) fail('missing_data', `${pieceId} requires ${key}`);
    const n = input.frameCount ?? 24;
    if (op === 'frames' || op === 'export') {
      if (!Number.isSafeInteger(n) || n < 1 || n > 120) fail('invalid_request', 'frameCount must be from 1 to 120');
    }
    if (op === 'export') {
      if (!GRAPHIC_FORMATS.has(format)) fail('invalid_request', 'Unsupported export format');
      try { exportSize(input.size ?? 'native'); } catch { fail('invalid_request', 'Unknown or excessive export size'); }
      if (format === 'gif' && !piece.animated) fail('not_animated', 'This piece does not support GIF animation');
    }
    const theme = styleEntry.renderer === 'classic' ? ORDERED_THEMES.find(t => t.id === styleId) : undefined;
    const baseStyle = theme ? bridgeStyle(theme) : STYLES.find(s => s.id === styleId)!;
    const style: Style = transparent ? { ...baseStyle, ground(s) { s.bg = 'transparent'; } } : baseStyle;
    const model = theme ? getModel('|classic', () => classicModel(raw, { director: false, lang, group, named })) : textModel();
    const ctx: Ctx = { lang, variant, transparent, fontMode, person: input.person, blame: input.blame, projects: input.projects?.map(raw => classicModel(raw, { director: false, lang, group, named })) };
    const svgModel = theme ? getModel('|bridge', () => buildModel(raw, { lang, group: (model as ClassicModel).group, named, sign })) : model;
    const bridged = !!theme && isBridged(pieceId, variant);
    const contextKey = session ? JSON.stringify([input.person, input.projects?.map(project => session.identity(project)), input.blame && session.identity(input.blame)]) : '';
    const key = `${modelKey}|${styleEntry.renderer}|${styleId}|${pieceId}|${variant ?? ''}|${transparent}|${fontMode}|${contextKey}`;
    const full = async () => { const result = !theme || bridged ? art(pieceId, svgModel, style, { variant }) : await backend.classicArt(pieceId, model, theme, ctx); return result && transparent ? { ...result, bg: 'transparent' } : result; };
    const original = session ? await session.picture(key, full) : await full();
    const at = async (time?: number): Promise<Art | null> => {
      if (time === undefined || time >= 1 && !original?.motion || !piece.animated) return structuredClone(original);
      if (!theme || bridged) return frame(pieceId, svgModel, style, { variant }, time, original);
      return (await backend.classicFrames(pieceId, model, theme, ctx, 24, [time]))[0] ?? null;
    };
    const flattenArt = (a: Art | null) => {
      if (!a) return null;
      if (a.svg.includes('<text')) a.svg = backend.raster.flat(a.svg);
      if (a.crop?.svg.includes('<text')) a.crop = { ...a.crop, svg: backend.raster.flat(a.crop.svg) };
      return a;
    };
    if (op === 'render') {
      const result = await at(reducedMotion ? 1 : t);
      return success(id, { art: flatten ? flattenArt(result) : result,
        timeline: original ? timelineFor(styleId, pieceId, original, reducedMotion || !piece.animated) : null });
    }
    if (!original) fail('missing_data', 'This archive does not contain enough data for the selected piece');
    const animate = async (): Promise<Animation> => {
      const still = reducedMotion || !piece.animated;
      const times = sampleTimes(n, still), timeline = timelineFor(styleId, pieceId, original, still);
      const generated: Art[] = theme && !bridged && !still
        ? await backend.classicFrames(pieceId, model, theme, ctx, n, times)
        : (await Promise.all(times.map(time => at(time)))).filter((a): a is Art => !!a);
      if (generated.length !== times.length) fail('missing_data', 'Animation did not return every requested frame');
      if (!original.motion && times.at(-1) === 1) generated[generated.length - 1] = structuredClone(original);
      return { frames: generated, times, timeline };
    };
    if (op === 'frames') {
      const animation = await animate(); if (flatten) animation.frames = animation.frames.map(a => flattenArt(a)!);
      return success(id, animation);
    }
    const animation = (format === 'gif' || format === 'html' && piece.animated) ? await animate() : undefined;
    const result = await exportArtwork(original, { format: format as GraphicFormat, size: input.size, lang, fontMode, scale: input.scale, width: input.width, style: styleId, piece: pieceId, reducedMotion }, boundedRaster(backend.raster), backend.deflate, animation);
    return output(id, result.bytes, result.mime, result.ext);
  } catch (error) {
    return errorResponse(id, error instanceof ProtocolError ? error.code : 'render_failed', error instanceof Error ? error.message : String(error));
  }
}
function output(id: RequestId, bytes: Uint8Array, mime: string, ext: string): RenderResponse {
  if (bytes.byteLength > MAX_RESPONSE_BYTES * 0.74) fail('response_too_large', 'Export exceeds transport limit');
  return success(id, { bytes: encoded(bytes), encoding: 'base64', mime, ext });
}
function bounded(response: RenderResponse): RenderResponse {
  if (encoder.encode(JSON.stringify(response)).byteLength > MAX_RESPONSE_BYTES) fail('response_too_large', 'Response exceeds 64 MiB');
  return response;
}
export function boundedRaster(raster: Raster): Raster {
  const check = (svg: string, width?: number) => {
    const root = /^<svg\b[^>]*>/.exec(svg)?.[0] ?? '';
    const w = Number(/\bwidth="([\d.]+)"/.exec(root)?.[1]), h = Number(/\bheight="([\d.]+)"/.exec(root)?.[1]);
    const outW = width ?? w, outH = h * outW / w;
    if (![w, h, outW, outH].every(x => Number.isFinite(x) && x > 0) || outW > 32768 || outH > 32768 || outW * outH > 32_000_000) fail('raster_too_large', 'Bitmap exceeds 32 million pixels; choose a smaller size or vector output');
  };
  return { png(svg, width) { check(svg, width); return raster.png(svg, width); }, rgba(svg, width) { check(svg, width); return raster.rgba(svg, width); }, flat: svg => raster.flat(svg), ...(raster.embed ? { embed: (svg: string) => raster.embed!(svg) } : {}) };
}
