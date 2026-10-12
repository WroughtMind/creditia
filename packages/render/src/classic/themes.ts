// 设计 token + 版式家族。风格之间的差异不只在颜色和字体：family 决定排版结构，
// 同一种形式在十四个家族里是不同的画法（见 forms.ts）。
export type Family =
  | 'minimal' | 'film' | 'terminal' | 'glass' | 'editorial' | 'mono'
  // 第二轮追加的体裁：各有装饰语汇，不是换色
  | 'cinema' | 'playbill' | 'gala' | 'broadsheet' | 'crt' | 'arcade' | 'neon' | 'poster';

export interface Theme {
  id: string;
  name: string;
  family: Family;
  dark: boolean;
  bg: string;
  /** Glass：背景上的柔光，按贡献方品牌色生成 */
  glow?: boolean;
  surface: string;
  edge: string;
  shadow: string;
  ink: string;
  soft: string;
  faint: string;
  line: string;
  track: string;
  stripeA: string;
  stripeB: string;
  /** 家族强调色：终端提示符、报刊红、胶片金 */
  accent: string;
  /** 正文、标签 */
  sans: string;
  /** 项目名、角色名等大字 */
  display: string;
  displayWeight: number;
  /** 数字 */
  num: string;
  /** 小标签：大写加字距 */
  labelFont: string;
  labelTrack: number;
  radius: number;
  barH: number;
  barRadius: number;
  /** 装饰字体：菱形、星、徽记、报头这类符号（缺省用 display） */
  deco?: string;
  /** 边框语汇：金框双线 / 报刊横线 / 花框 / 粗黑框 / 像素框 */
  frame?: 'none' | 'double' | 'rule' | 'ornate' | 'thick' | 'pixel';
  /** 底纹（CSS backgroundImage，satori 支持渐变） */
  pattern?: string;
}

const SANS = 'Inter, Noto Sans SC';
const SERIF = 'Instrument Serif, Noto Serif SC';
const NEWS = 'Newsreader, Noto Serif SC';
const MONO = 'JetBrains Mono, Noto Sans SC';
const CINZEL = 'Cinzel, Noto Serif SC';
const PLAYFAIR = 'Playfair Display, Noto Serif SC';
const BASKERVILLE = 'Libre Baskerville, Noto Serif SC';
const OSWALD = 'Oswald, Noto Sans SC';
const POIRET = 'Poiret One, Noto Sans SC';
const XIAOWEI = 'ZCOOL XiaoWei, Noto Serif SC';
const HUANGYOU = 'ZCOOL QingKe HuangYou, Noto Sans SC';
const PIXEL = 'Press Start 2P, ZCOOL QingKe HuangYou, Noto Sans SC';

const minimalLight: Theme = {
  id: 'minimal-light', name: 'Minimal', family: 'minimal', dark: false,
  bg: '#F4F3F0', surface: '#FFFFFF', edge: 'rgba(22,21,20,0.06)', shadow: '0 1px 2px rgba(22,21,20,0.04), 0 12px 40px rgba(22,21,20,0.06)',
  ink: '#161514', soft: '#76726B', faint: '#AEAAA3', line: 'rgba(22,21,20,0.08)', track: 'rgba(22,21,20,0.06)',
  stripeA: '#CFCBC3', stripeB: '#ECEAE5', accent: '#161514',
  sans: SANS, display: SANS, displayWeight: 600, num: SANS, labelFont: SANS, labelTrack: 0.14,
  radius: 18, barH: 4, barRadius: 99,
};
const minimalDark: Theme = {
  ...minimalLight, id: 'minimal-dark', name: 'Minimal 暗', dark: true,
  bg: '#0D0D0C', surface: '#171716', edge: 'rgba(240,238,233,0.06)', shadow: '0 12px 40px rgba(0,0,0,0.4)',
  ink: '#F0EEE9', soft: '#8F8B84', faint: '#5C5953', line: 'rgba(240,238,233,0.08)', track: 'rgba(240,238,233,0.07)',
  stripeA: '#4A4844', stripeB: '#2A2927', accent: '#F0EEE9',
};
const film: Theme = {
  id: 'film', name: 'Film Credits', family: 'film', dark: true,
  bg: '#0A0908', surface: '#12110F', edge: 'rgba(237,230,216,0.08)', shadow: 'none',
  ink: '#EDE6D8', soft: '#8C8577', faint: '#5A554C', line: 'rgba(237,230,216,0.1)', track: 'rgba(237,230,216,0.08)',
  stripeA: '#8C8577', stripeB: '#26231F', accent: '#BFA46A',
  sans: SANS, display: SERIF, displayWeight: 400, num: SERIF, labelFont: SANS, labelTrack: 0.32,
  radius: 2, barH: 2, barRadius: 0,
};
const terminal: Theme = {
  id: 'terminal', name: 'Terminal', family: 'terminal', dark: true,
  bg: '#0B0D0F', surface: '#111418', edge: 'rgba(215,222,229,0.08)', shadow: 'none',
  ink: '#D7DEE5', soft: '#7C8792', faint: '#4B545D', line: 'rgba(215,222,229,0.09)', track: 'rgba(215,222,229,0.07)',
  stripeA: '#3A424A', stripeB: '#1A1F24', accent: '#7FD08C',
  sans: MONO, display: MONO, displayWeight: 700, num: MONO, labelFont: MONO, labelTrack: 0.08,
  radius: 6, barH: 8, barRadius: 0,
};
const glass: Theme = {
  id: 'glass', name: 'Glass', family: 'glass', dark: false, glow: true,
  bg: '#EEF0F5', surface: 'rgba(255,255,255,0.58)', edge: 'rgba(255,255,255,0.9)', shadow: '0 20px 60px rgba(40,48,80,0.12)',
  ink: '#14161C', soft: '#5E6472', faint: '#9AA0AC', line: 'rgba(20,22,28,0.08)', track: 'rgba(20,22,28,0.07)',
  stripeA: '#BFC4CF', stripeB: '#E4E7EE', accent: '#5B7CFA',
  sans: SANS, display: SANS, displayWeight: 600, num: SANS, labelFont: SANS, labelTrack: 0.14,
  radius: 28, barH: 6, barRadius: 99,
};
const editorial: Theme = {
  id: 'editorial', name: 'Editorial', family: 'editorial', dark: false,
  bg: '#F2EDE3', surface: '#F2EDE3', edge: '#121110', shadow: 'none',
  ink: '#121110', soft: '#6A6359', faint: '#A39B8E', line: '#121110', track: 'rgba(18,17,16,0.08)',
  stripeA: '#121110', stripeB: '#E2DBCF', accent: '#C8442E',
  sans: SANS, display: NEWS, displayWeight: 500, num: NEWS, labelFont: SANS, labelTrack: 0.18,
  radius: 0, barH: 10, barRadius: 0,
};
const mono: Theme = {
  id: 'mono', name: 'Mono', family: 'mono', dark: false,
  bg: '#FFFFFF', surface: '#FFFFFF', edge: '#000000', shadow: 'none',
  ink: '#000000', soft: '#6B6B6B', faint: '#B0B0B0', line: '#000000', track: '#EDEDED',
  stripeA: '#000000', stripeB: '#FFFFFF', accent: '#000000',
  sans: SANS, display: SANS, displayWeight: 700, num: SANS, labelFont: SANS, labelTrack: 0.12,
  radius: 0, barH: 6, barRadius: 0,
};

// ───────────────────────── 第二轮追加的体裁 ─────────────────────────
// 每个体裁有自己的字体、装饰与结构。风格不追求克制：片尾字幕、戏院海报、
// 颁奖礼、老报纸本来就各有各的繁复。

/** Cinema：黄金时代片尾。黑底金字、Art Deco 双线框、菱形分隔 */
const cinema: Theme = {
  id: 'cinema', name: 'Cinema 片尾', family: 'cinema', dark: true,
  bg: '#08070A', surface: '#0F0E12', edge: 'rgba(214,178,106,0.34)', shadow: 'none',
  ink: '#F2E8D2', soft: '#A2946F', faint: '#6C6250', line: 'rgba(214,178,106,0.26)', track: 'rgba(214,178,106,0.16)',
  stripeA: '#D6B26A', stripeB: '#1C1A20', accent: '#D6B26A',
  sans: BASKERVILLE, display: CINZEL, displayWeight: 500, num: CINZEL, labelFont: CINZEL, labelTrack: 0.34,
  radius: 0, barH: 3, barRadius: 0,
  deco: 'Cinzel Decorative, Cinzel, Noto Serif SC', frame: 'double',
};
/** Playbill：剧院节目单。奶油纸、红黑、票根虚线 */
const playbill: Theme = {
  id: 'playbill', name: 'Playbill 节目单', family: 'playbill', dark: false,
  bg: '#F1E6CC', surface: '#F8F0DE', edge: '#8C1C13', shadow: 'none',
  ink: '#241A12', soft: '#6E5D46', faint: '#A79A80', line: 'rgba(36,26,18,0.28)', track: 'rgba(140,28,19,0.16)',
  stripeA: '#8C1C13', stripeB: '#E6D6B4', accent: '#8C1C13',
  sans: BASKERVILLE, display: PLAYFAIR, displayWeight: 900, num: PLAYFAIR, labelFont: PLAYFAIR, labelTrack: 0.22,
  radius: 0, barH: 9, barRadius: 0,
  deco: PLAYFAIR, frame: 'ornate',
};
/** Gala：颁奖礼。深蓝金、Art Deco 细线、放射光 */
const gala: Theme = {
  id: 'gala', name: 'Gala 颁奖礼', family: 'gala', dark: true,
  bg: '#0A0918', surface: '#121128', edge: 'rgba(212,175,55,0.32)', shadow: 'none',
  ink: '#F6EFDD', soft: '#9B93B9', faint: '#625C81', line: 'rgba(212,175,55,0.28)', track: 'rgba(212,175,55,0.16)',
  stripeA: '#D4AF37', stripeB: '#1A1834', accent: '#D4AF37',
  sans: OSWALD, display: 'Limelight, ZCOOL XiaoWei, Noto Serif SC', displayWeight: 400, num: OSWALD, labelFont: POIRET, labelTrack: 0.36,
  radius: 0, barH: 4, barRadius: 0,
  deco: 'Poiret One, ZCOOL XiaoWei, Noto Serif SC', frame: 'rule',
};
/** Broadsheet：老报纸。米纸、黑、报头黑字、多栏 */
const broadsheet: Theme = {
  id: 'broadsheet', name: 'Broadsheet 报纸', family: 'broadsheet', dark: false,
  bg: '#EDE6D4', surface: '#F5EFE0', edge: '#1A1713', shadow: 'none',
  ink: '#1A1713', soft: '#5F574A', faint: '#9C9282', line: 'rgba(26,23,19,0.34)', track: 'rgba(26,23,19,0.1)',
  stripeA: '#1A1713', stripeB: '#DBD1BB', accent: '#8C1C13',
  sans: BASKERVILLE, display: PLAYFAIR, displayWeight: 900, num: PLAYFAIR, labelFont: PLAYFAIR, labelTrack: 0.2,
  radius: 0, barH: 10, barRadius: 0,
  deco: 'UnifrakturMaguntia, Playfair Display, Noto Serif SC', frame: 'rule',
};
/** CRT：老终端。绿磷光、扫描线 */
const crt: Theme = {
  id: 'crt', name: 'CRT 磷光', family: 'crt', dark: true,
  bg: '#04120A', surface: '#061A0F', edge: 'rgba(80,255,140,0.3)', shadow: 'none',
  ink: '#8CFFB0', soft: '#4FBF78', faint: '#2E7A4C', line: 'rgba(80,255,140,0.24)', track: 'rgba(80,255,140,0.13)',
  stripeA: '#50FF8C', stripeB: '#0A2416', accent: '#C6FF5C',
  sans: 'VT323, Noto Sans SC', display: 'VT323, Noto Sans SC', displayWeight: 400, num: 'VT323, Noto Sans SC', labelFont: 'VT323, Noto Sans SC', labelTrack: 0.1,
  radius: 0, barH: 10, barRadius: 0,
  pattern: 'repeating-linear-gradient(0deg, rgba(0,0,0,0.3) 0px, rgba(0,0,0,0.3) 1px, rgba(0,0,0,0) 1px, rgba(0,0,0,0) 4px)',
  frame: 'pixel',
};
/** Arcade：街机高分榜。深紫、像素框、霓虹色 */
const arcade: Theme = {
  id: 'arcade', name: 'Arcade 街机', family: 'arcade', dark: true,
  bg: '#120726', surface: '#1B0C36', edge: '#FF4FA3', shadow: 'none',
  ink: '#F7ECFF', soft: '#B79BD6', faint: '#7B5FA6', line: 'rgba(255,79,163,0.4)', track: 'rgba(255,255,255,0.12)',
  stripeA: '#FF4FA3', stripeB: '#2A1250', accent: '#4FE3FF',
  sans: HUANGYOU, display: PIXEL, displayWeight: 400, num: PIXEL, labelFont: PIXEL, labelTrack: 0.04,
  radius: 0, barH: 12, barRadius: 0,
  deco: HUANGYOU, frame: 'pixel',
};
/** Neon：霓虹招牌。黑、Monoton、粉青 */
const neon: Theme = {
  id: 'neon', name: 'Neon 霓虹', family: 'neon', dark: true,
  bg: '#0A0710', surface: '#130C1E', edge: 'rgba(255,93,143,0.42)', shadow: 'none',
  ink: '#FFE9F2', soft: '#C08FB4', faint: '#7C5B78', line: 'rgba(255,93,143,0.32)', track: 'rgba(255,255,255,0.1)',
  stripeA: '#FF5D8F', stripeB: '#241230', accent: '#4FE3FF',
  sans: POIRET, display: 'Monoton, ZCOOL XiaoWei, Noto Serif SC', displayWeight: 400, num: POIRET, labelFont: POIRET, labelTrack: 0.3,
  radius: 10, barH: 5, barRadius: 99,
  deco: XIAOWEI, frame: 'ornate',
};
/** Poster：海报。米白红黑大色块、粗横线 */
const poster: Theme = {
  id: 'poster', name: 'Poster 海报', family: 'poster', dark: false,
  bg: '#F0EDE6', surface: '#FFFFFF', edge: '#141414', shadow: 'none',
  ink: '#141414', soft: '#5A5A5A', faint: '#A3A3A3', line: '#141414', track: 'rgba(20,20,20,0.1)',
  stripeA: '#E23B2E', stripeB: '#141414', accent: '#E23B2E',
  sans: 'Inter, Noto Sans SC', display: 'Archivo Black, Noto Sans SC', displayWeight: 400, num: 'Anton, Noto Sans SC', labelFont: OSWALD, labelTrack: 0.18,
  radius: 0, barH: 14, barRadius: 0,
  frame: 'thick',
};

export const THEMES: Theme[] = [
  minimalLight, minimalDark, film, terminal, glass, editorial, mono,
  cinema, playbill, gala, broadsheet, crt, arcade, neon, poster,
];
export const themeById = (id: string) => THEMES.find((t) => t.id === id) ?? (id === 'minimal' ? minimalLight : undefined);
