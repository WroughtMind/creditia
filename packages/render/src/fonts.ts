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
export const facesOf = (family: string) => FONTS.filter((f) => f.family.toLowerCase() === family.toLowerCase());

/** CSS font-family 串或 SVG 属性 → 引用到的 family 名（解实体、去引号） */
export const familiesIn = (stack: string) => stack.replace(/&quot;/g, '"').replace(/&apos;|&#39;/g, "'").split(',').map((s) => s.trim().replace(/^['"]|['"]$/g, '')).filter(Boolean);

/** Legacy badges named system fonts. Pin their existing native fallback so a
 * browser, offline SVG and the native renderer never pick different OS fonts. */
export function portableFontFamilies(svg: string): string {
  // The legacy terminal prompt's chevron is absent from Noto Sans SC; native
  // fallback shapes that whole prompt in JetBrains Mono, including its spaces.
  svg = svg.replace(/<text\b([^>]*)>([\s\S]*?)<\/text>/g, (element, attributes: string, content: string) =>
    content.includes('❯') && /font-family="(?:Menlo|Consolas)[^"]*"/.test(attributes)
      ? `<text${attributes.replace(/font-family="[^"]*"/, 'font-family="JetBrains Mono"')}>${content}</text>` : element);
  return svg.replace(/\bfont-family="([^"]+)"/g, (attribute, value: string) =>
    familiesIn(value.replaceAll('&quot;', '"').replaceAll('&apos;', "'")).some(name => facesOf(name).length)
      ? attribute : 'font-family="Noto Sans SC"');
}

// CSS weight matching is directional (600 prefers 700 before 500; 400 prefers
// 500 before 300). A numeric nearest-weight sort subtly changes native output
// when moving from all installed faces to just the faces needed by a picture.
export function matchingFace(family: string, weight: number, italic: boolean): FontFace | undefined {
  const rank = (w: number) => weight >= 400 && weight <= 500
    ? w >= weight && w <= 500 ? w - weight : w < weight ? 1000 + weight - w : 2000 + w - weight
    : weight < 400 ? w <= weight ? weight - w : 1000 + w - weight
    : w >= weight ? w - weight : 1000 + weight - w;
  return facesOf(family).sort((a, b) => Number(!!a.italic !== italic) - Number(!!b.italic !== italic) || rank(a.weight) - rank(b.weight))[0];
}

/** Font faces actually referenced by text, including inherited tspan attributes. */
export function referencedFonts(svg: string): FontFace[] {
  const refs = new Map<string, FontFace>();
  const stack: { family?: string; weight: number; italic: boolean }[] = [{ weight: 400, italic: false }];
  for (const match of svg.matchAll(/<\/?([\w:-]+)\b[^>]*>/g)) {
    const tag = match[0];
    if (tag.startsWith('</')) { if (stack.length > 1) stack.pop(); continue; }
    const parent = stack.at(-1)!;
    const family = /\bfont-family="([^"]+)"/.exec(tag)?.[1] ?? parent.family;
    const weight = Number(/\bfont-weight="(\d+)"/.exec(tag)?.[1] ?? parent.weight);
    const mode = /\bfont-style="([^"]+)"/.exec(tag)?.[1];
    const italic = mode ? mode === 'italic' : parent.italic;
    if (match[1] === 'text' || match[1] === 'tspan') {
      let found = false;
      for (const name of familiesIn((family ?? '').replaceAll('&quot;', '"').replaceAll('&apos;', "'"))) {
        const face = matchingFace(name, weight, italic);
        if (face) { refs.set(`${face.pkg}/${face.dir}`, face); found = true; }
      }
      if (!found) {
        const fallback = matchingFace('Noto Sans SC', weight, italic)!;
        refs.set(`${fallback.pkg}/${fallback.dir}`, fallback);
      }
    }
    if (!tag.endsWith('/>')) stack.push({ family, weight, italic });
  }
  return [...refs.values()];
}

/** Face order used by native usvg glyph fallback and portable text shaping. */
export function nativeFonts(svg: string): FontFace[] {
  const referenced = referencedFonts(svg);
  if (!referenced.length) return [];
  const wanted = new Set(referenced.map(face => `${face.pkg}/${face.dir}`));
  return FONTS.filter(face => wanted.has(`${face.pkg}/${face.dir}`)
    || face.family === 'Inter' || face.family === 'JetBrains Mono'
    || face.family === 'Noto Sans SC' && face.weight === 400);
}
