// 两套渲染器摆在同一面展墙上：原版（satori 排版，十五个主题）和新风格（手写 SVG）。
// 圆点里先是原版主题，再是新风格；选到哪个，展墙就用哪一套的作品、版式和导出。
import * as C from '../../../packages/render/src/classic/data.ts';
import { themeById } from '../../../packages/render/src/classic/themes.ts';
import { ORDERED_THEMES, themeDot, themeName, themeNote, pieceName as cPiece, pieceQ as cQ, variantName as cVariant } from '../../../packages/render/src/classic/catalog.ts';
import { variantsOf, defaultVariant, animatedClassic } from '../../../packages/render/src/classic/art.ts';
import { prepare, render as classicRender, frames as classicFrames } from '../../../packages/render/src/classic/browser.ts';
import { isBridged, bridgeStyle } from '../../../packages/render/src/classic/bridge.ts';
import { STYLES } from '../../../packages/render/src/styles/index.ts';
import * as V from '../../../packages/render/src/data.ts';
import { PIECES, art as v2Art, frames as v2Frames, variantName as v2Variant } from '../../../packages/render/src/registry.ts';
import type { Style } from '../../../packages/render/src/style.ts';
import type { Art } from '../../../packages/render/src/export.ts';
import { fontFiles, registerFonts, ready } from './fonts.ts';

type Lang = 'zh' | 'en';
export interface Tile { id: string; variant?: string; span?: number }
export interface Kit {
  id: string;
  /** 导出带的字体：新风格按风格取；原版主题自己的作品文字已经是路径，借新风格画的作品按原版的字体取 */
  style: Style | null;
  dot: string;
  name(lang: Lang): string;
  note(lang: Lang): string;
  board: Tile[];
  /** raw 档案 → 这一套的模型（按语言缓存） */
  render(raw: any, lang: Lang, id: string, variant?: string): Promise<Art | null>;
  frames?(raw: any, lang: Lang, id: string, variant?: string): Art[] | Promise<Art[]>;
  variants(id: string): string[];
  current(id: string, variant?: string): string | undefined;
  animated(id: string): boolean;
  pieceName(id: string, lang: Lang): string;
  pieceQ(id: string, lang: Lang): string;
  variantName(v: string, lang: Lang): string;
}

export function bootKits() {
  prepare(fontFiles());
  registerFonts();
}

/** 同一份档案同一种语言只建一次模型 */
function memo<M>(build: (raw: any, lang: Lang) => M) {
  let k: [any, Lang] | null = null, v: M;
  return (raw: any, lang: Lang) => { if (!k || k[0] !== raw || k[1] !== lang) { v = build(raw, lang); k = [raw, lang]; } return v; };
}
const classicModel = memo((raw, lang) => C.buildModel(raw, { director: false, lang }));
const v2Model = memo((raw, lang) => V.buildModel(raw, { lang, named: false, sign: '' }));

const CLASSIC_BOARD: Tile[] = [
  { id: 'share', span: 2 }, { id: 'tier' }, { id: 'roll' }, { id: 'receipt' }, { id: 'donut' },
  { id: 'ribbon', span: 2 }, { id: 'waffle' }, { id: 'mosaic' }, { id: 'versions' }, { id: 'area' },
  { id: 'stream', span: 2 }, { id: 'survival' }, { id: 'evidence' }, { id: 'areas' },
  { id: 'lanes' }, { id: 'clock' }, { id: 'settle' }, { id: 'app' }, { id: 'fetch' },
  { id: 'codemap', span: 2 }, { id: 'calendar', span: 2 },
  { id: 'byline', span: 2 }, { id: 'badge' }, { id: 'badge', variant: 'emblem' }, { id: 'badge', variant: 'stack' },
];
const V2_BOARD: Tile[] = [
  { id: 'share', span: 2 }, { id: 'tier' }, { id: 'roll' }, { id: 'receipt' }, { id: 'donut' },
  { id: 'stream', span: 2 }, { id: 'waffle' }, { id: 'survival' }, { id: 'evidence' }, { id: 'versions' }, { id: 'areas' }, { id: 'lanes', span: 2 }, { id: 'clock', span: 2 },
  { id: 'mosaic' }, { id: 'codemap', span: 2 }, { id: 'calendar', span: 2 }, { id: 'badge', variant: 'emblem' }, { id: 'badge', variant: 'stack' }, { id: 'badge', variant: 'flat' }, { id: 'byline', span: 2 },
];

const classicKits: Kit[] = ORDERED_THEMES.map((t) => ({
  id: t.id, style: bridgeStyle(t), dot: themeDot(t),
  name: (lang) => themeName(t, lang), note: (lang) => themeNote(t, lang),
  board: CLASSIC_BOARD,
  // 原版没有的作品、追加的新风格版式按这个主题的颜色、字体、框线用新风格的画法画，先等字体
  render: async (raw, lang, id, variant) => {
    if (isBridged(id, variant)) await ready(bridgeStyle(t));
    return classicRender(id, classicModel(raw, lang), t, { lang, variant });
  },
  frames: async (raw, lang, id, variant) => {
    if (isBridged(id, variant)) await ready(bridgeStyle(t));
    return classicFrames(id, classicModel(raw, lang), t, { lang, variant }, 24);
  },
  variants: variantsOf,
  current: (id, variant) => variant ?? defaultVariant(id, t) ?? variantsOf(id)[0],
  animated: animatedClassic,
  pieceName: cPiece, pieceQ: cQ, variantName: cVariant,
}));

/** 新风格里和原版重名、或者原版已经有对应的，不再重复 */
const SKIP = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const v2Kits: Kit[] = STYLES.filter((s) => !SKIP.has(s.id)).map((s) => ({
  id: s.id, style: s, dot: s.dot(),
  name: (lang) => (lang === 'zh' ? s.name : s.en), note: (lang) => (lang === 'zh' ? s.note : s.noteEn),
  board: V2_BOARD,
  render: async (raw, lang, id, variant) => { await ready(s); return v2Art(id, v2Model(raw, lang), s, { variant }); },
  frames: (raw, lang, id, variant) => v2Frames(id, v2Model(raw, lang), s, { variant }, 24),
  variants: (id) => PIECES[id]?.variants ?? [],
  current: (id, variant) => variant ?? s.prefer[id] ?? PIECES[id]?.variants?.[0],
  animated: (id) => !!PIECES[id]?.animated,
  pieceName: (id, lang) => (lang === 'zh' ? PIECES[id]?.name : PIECES[id]?.en) ?? id,
  pieceQ: (id, lang) => (lang === 'zh' ? PIECES[id]?.q : PIECES[id]?.qEn) ?? '',
  variantName: v2Variant,
}));

export const KITS: Kit[] = [...classicKits, ...v2Kits];
/** 原版主题在前；分隔线画在这个位置 */
export const CLASSIC_COUNT = classicKits.length;
export const kitById = (id: string) => KITS.find((k) => k.id === id) ?? (themeById(id) ? classicKits.find((k) => k.id === themeById(id)!.id) : undefined);
