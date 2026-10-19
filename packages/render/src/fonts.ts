// 字体清单：family → 各字重所在的 @expo-google-fonts 包和目录。
// Node 端（CLI、样张）按这份清单把文件喂给 resvg；App 端按同一份清单生成 @font-face；度量表也由它生成。
// 文件名不靠拼（各包大小写不一致，在大小写敏感的文件系统上会找不到），统一在目录里找 .ttf。

export interface FontFace { family: string; weight: number; italic?: boolean; pkg: string; dir: string; cjk?: boolean }

const F = (family: string, pkg: string, faces: [number, string, boolean?][], cjk = false): FontFace[] =>
  faces.map(([weight, dir, italic]) => ({ family, weight, italic, pkg, dir, cjk }));

export const FONTS: FontFace[] = [
  // ── 基础 ──
  ...F('Inter', 'inter', [[400, '400Regular'], [500, '500Medium'], [600, '600SemiBold'], [700, '700Bold']]),
  ...F('JetBrains Mono', 'jetbrains-mono', [[400, '400Regular'], [700, '700Bold']]),
  ...F('Noto Sans SC', 'noto-sans-sc', [[400, '400Regular'], [500, '500Medium'], [700, '700Bold'], [900, '900Black']], true),
  ...F('Noto Serif SC', 'noto-serif-sc', [[400, '400Regular'], [500, '500Medium'], [700, '700Bold'], [900, '900Black']], true),
  // ── 电影、古典 ──
  ...F('Cinzel', 'cinzel', [[500, '500Medium'], [700, '700Bold']]),
  ...F('Cormorant Garamond', 'cormorant-garamond', [[500, '500Medium'], [600, '600SemiBold'], [700, '700Bold'], [500, '500Medium_Italic', true]]),
  ...F('Pinyon Script', 'pinyon-script', [[400, '400Regular']]),
  ...F('Pirata One', 'pirata-one', [[400, '400Regular']]),
  ...F('UnifrakturMaguntia', 'unifrakturmaguntia', [[400, '400Regular']]),
  ...F('IM FELL English', 'im-fell-english', [[400, '400Regular'], [400, '400Regular_Italic', true]]),
  // ── 文学、打字机 ──
  ...F('Courier Prime', 'courier-prime', [[400, '400Regular'], [700, '700Bold'], [400, '400Regular_Italic', true]]),
  // ── 几何、海报 ──
  ...F('Jost', 'jost', [[400, '400Regular'], [500, '500Medium'], [700, '700Bold'], [900, '900Black']]),
  ...F('Anton', 'anton', [[400, '400Regular']]),
  ...F('Oswald', 'oswald', [[400, '400Regular'], [600, '600SemiBold']]),
  // ── 赛博、终端 ──
  ...F('Rajdhani', 'rajdhani', [[500, '500Medium'], [700, '700Bold']]),
  ...F('Chakra Petch', 'chakra-petch', [[400, '400Regular'], [600, '600SemiBold'], [700, '700Bold']]),
  ...F('Share Tech Mono', 'share-tech-mono', [[400, '400Regular']]),
  ...F('Bangers', 'bangers', [[400, '400Regular']]),
  // ── 原版主题（画新作品时用原版自己的字体） ──
  ...F('Instrument Serif', 'instrument-serif', [[400, '400Regular']]),
  ...F('Newsreader', 'newsreader', [[400, '400Regular'], [500, '500Medium']]),
  ...F('Cinzel Decorative', 'cinzel-decorative', [[700, '700Bold']]),
  ...F('Playfair Display', 'playfair-display', [[500, '500Medium'], [700, '700Bold'], [900, '900Black']]),
  ...F('Libre Baskerville', 'libre-baskerville', [[400, '400Regular'], [700, '700Bold']]),
  ...F('Poiret One', 'poiret-one', [[400, '400Regular']]),
  ...F('Limelight', 'limelight', [[400, '400Regular']]),
  ...F('VT323', 'vt323', [[400, '400Regular']]),
  ...F('Press Start 2P', 'press-start-2p', [[400, '400Regular']]),
  ...F('Monoton', 'monoton', [[400, '400Regular']]),
  ...F('Archivo Black', 'archivo-black', [[400, '400Regular']]),
  // ── 中日文字体 ──
  ...F('Ma Shan Zheng', 'ma-shan-zheng', [[400, '400Regular']], true),
  ...F('Zhi Mang Xing', 'zhi-mang-xing', [[400, '400Regular']], true),
  ...F('Long Cang', 'long-cang', [[400, '400Regular']], true),
  ...F('ZCOOL XiaoWei', 'zcool-xiaowei', [[400, '400Regular']], true),
  ...F('Klee One', 'klee-one', [[400, '400Regular'], [600, '600SemiBold']], true),
  ...F('Shippori Mincho B1', 'shippori-mincho-b1', [[500, '500Medium'], [800, '800ExtraBold']], true),
  ...F('Dela Gothic One', 'dela-gothic-one', [[400, '400Regular']], true),
  ...F('Zen Maru Gothic', 'zen-maru-gothic', [[500, '500Medium'], [700, '700Bold']], true),
  ...F('Zen Antique', 'zen-antique', [[400, '400Regular']], true),
  ...F('DotGothic16', 'dotgothic16', [[400, '400Regular']], true),
  ...F('ZCOOL QingKe HuangYou', 'zcool-qingke-huangyou', [[400, '400Regular']], true),
];

/** 某个 family 的全部字重 */
export const facesOf = (family: string) => FONTS.filter((f) => f.family === family);

/** CSS font-family 串 → 引用到的 family 名（去引号） */
export const familiesIn = (stack: string) => stack.split(',').map((s) => s.trim().replace(/^['"]|['"]$/g, '')).filter(Boolean);
