// 原版渲染器用到的字体：Node 端（index.ts）直接读包里的 ttf，App 端由 tools/fonts.mjs 按这份清单做子集。
export interface ClassicFont { family: string; weight: number; pkg: string; dir: string; cjk?: boolean }

const F = (family: string, pkg: string, faces: [number, string][], cjk = false): ClassicFont[] =>
  faces.map(([weight, dir]) => ({ family, weight, pkg, dir, cjk }));

export const CLASSIC_FONTS: ClassicFont[] = [
  ...F('Inter', 'inter', [[400, '400Regular'], [500, '500Medium'], [600, '600SemiBold'], [700, '700Bold']]),
  ...F('JetBrains Mono', 'jetbrains-mono', [[400, '400Regular'], [700, '700Bold']]),
  ...F('Instrument Serif', 'instrument-serif', [[400, '400Regular']]),
  ...F('Newsreader', 'newsreader', [[400, '400Regular'], [500, '500Medium']]),
  ...F('Noto Sans SC', 'noto-sans-sc', [[400, '400Regular'], [500, '500Medium'], [700, '700Bold']], true),
  ...F('Noto Serif SC', 'noto-serif-sc', [[400, '400Regular'], [500, '500Medium']], true),
  ...F('Cinzel', 'cinzel', [[500, '500Medium'], [700, '700Bold']]),
  ...F('Cinzel Decorative', 'cinzel-decorative', [[700, '700Bold']]),
  ...F('Playfair Display', 'playfair-display', [[500, '500Medium'], [700, '700Bold'], [900, '900Black']]),
  ...F('Libre Baskerville', 'libre-baskerville', [[400, '400Regular'], [700, '700Bold']]),
  ...F('Oswald', 'oswald', [[400, '400Regular'], [600, '600SemiBold']]),
  ...F('Poiret One', 'poiret-one', [[400, '400Regular']]),
  ...F('Limelight', 'limelight', [[400, '400Regular']]),
  ...F('UnifrakturMaguntia', 'unifrakturmaguntia', [[400, '400Regular']]),
  ...F('VT323', 'vt323', [[400, '400Regular']]),
  ...F('Press Start 2P', 'press-start-2p', [[400, '400Regular']]),
  ...F('Monoton', 'monoton', [[400, '400Regular']]),
  ...F('Archivo Black', 'archivo-black', [[400, '400Regular']]),
  ...F('Anton', 'anton', [[400, '400Regular']]),
  ...F('ZCOOL XiaoWei', 'zcool-xiaowei', [[400, '400Regular']], true),
  ...F('ZCOOL QingKe HuangYou', 'zcool-qingke-huangyou', [[400, '400Regular']], true),
  ...F('Ma Shan Zheng', 'ma-shan-zheng', [[400, '400Regular']], true),
];

/** 一个主题实际用到的字体：主题自己的字体 + 各形式里直接写死的（等宽、窄体、首字母）。
 *  satori 会把给它的每个字体都解析进内存，全给的话出一张图要六七百 MB */
export function fontsFor(t: { sans: string; display: string; num: string; labelFont: string; deco?: string }): ClassicFont[] {
  const fams = new Set(['Inter', 'JetBrains Mono', 'Oswald', 'Noto Sans SC']);
  for (const s of [t.sans, t.display, t.num, t.labelFont, t.deco ?? '']) for (const f of s.split(',')) if (f.trim()) fams.add(f.trim());
  return CLASSIC_FONTS.filter((f) => fams.has(f.family));
}
