import { deflateSync } from 'node:zlib';
import { art, frames as classicFrames, buildModel, FORMS, themeById, raster, type Options, type Theme } from '../../render/src/classic/index.ts';
import { ORDERED_THEMES } from '../../render/src/classic/catalog.ts';
import { makeExporter, type Art } from '../../render/src/export.ts';
import * as V2 from '../../render/src/index.ts';
import { BRIDGED } from '../../render/src/classic/bridge.ts';
import { animatedClassic } from '../../render/src/classic/art.ts';

export { themeDot, themeName, themeNote, pieceName, pieceQ, variantName } from '../../render/src/classic/catalog.ts';
export { animatedClassic, variantsOf } from '../../render/src/classic/art.ts';
export { SIZES, FORMATS } from '../../render/src/export.ts';
export { PIECES } from '../../render/src/registry.ts';
export { renderRequest } from '../../render/src/native-protocol.ts';

const deflate = (d: Uint8Array) => new Uint8Array(deflateSync(d));
// 原版作品的文字已经是路径，栅格化不需要字体；新风格的作品要带字体。
// 原版主题里借新风格画的作品（河流、留存……）还是 <text>，按内容挑
const pick = (svg: string) => (svg.includes('<text') ? V2.raster : raster);
const ex = makeExporter({ png: (s, w) => pick(s).png(s, w), rgba: (s, w) => pick(s).rgba(s, w), flat: (s) => pick(s).flat(s) }, deflate);
const exV2 = makeExporter(V2.raster, deflate);

/** 能出图的作品：需要额外数据的（个人履历、逐行溯源）不在这里 */
export const PIECE_IDS = [...Object.keys(FORMS).filter((id) => id !== 'profile' && id !== 'blame'), 'badge', ...BRIDGED];
/** 新风格里和原版重名、或者原版已经有对应的，用原版 */
const V2_SKIP = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const V2_STYLES = V2.STYLES.filter((s) => !V2_SKIP.has(s.id));
/** 风格：原版主题（Theme）或新风格（Style） */
export type Look = { kind: 'classic'; t: Theme } | { kind: 'v2'; s: (typeof V2.STYLES)[number] };
export function findLook(id?: string): Look | undefined {
  const t = themeById(id ?? 'minimal-light');
  if (t) return { kind: 'classic', t };
  const s = V2_STYLES.find((x) => x.id === id);
  if (s) return { kind: 'v2', s };
}
export const exOf = (l: Look) => (l.kind === 'classic' ? ex : exV2);
/** 新风格的 hand 只写人这一侧的数字，作品里不再写人 */
const V2_PIECES = Object.keys(V2.PIECES).filter((id) => id !== 'hand');
export const piecesOf = (l: Look) => (l.kind === 'classic' ? PIECE_IDS : V2_PIECES);
export const LOOKS: Look[] = [...ORDERED_THEMES.map((t) => ({ kind: 'classic' as const, t })), ...V2_STYLES.map((s) => ({ kind: 'v2' as const, s }))];

type ArtOptions = Pick<Options, 'lang' | 'named' | 'group'> & { lang: 'zh' | 'en' };
async function artOf(id: string, raw: any, l: Look, options: ArtOptions): Promise<Art | null> {
  if (l.kind === 'classic') return art(id, buildModel(raw, { director: false, ...options }), l.t, { lang: options.lang });
  return V2.art(id, V2.buildModel(raw, { sign: '', ...options }), l.s, {});
}
/** 动画帧；不会动的作品返回 null */
async function framesOf(id: string, raw: any, l: Look, options: ArtOptions, variant: string | undefined, n: number): Promise<Art[] | null> {
  if (l.kind === 'v2') return V2.PIECES[id]?.animated ? V2.frames(id, V2.buildModel(raw, { sign: '', ...options }), l.s, { variant }, n) : null;
  return animatedClassic(id) ? classicFrames(id, buildModel(raw, { director: false, ...options }), l.t, { lang: options.lang, variant }, n) : null;
}
export async function artsOf(ids: string[], raw: any, l: Look, options: ArtOptions): Promise<Art[]> {
  const out: Art[] = [];
  for (const id of ids) if (piecesOf(l).includes(id)) { const a = await artOf(id, raw, l, options); if (a) out.push(a); }
  return out;
}
