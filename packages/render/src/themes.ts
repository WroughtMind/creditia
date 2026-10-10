// 设计 token + 版式家族。风格之间的差异不只在颜色和字体：family 决定排版结构，
// 同一种形式在六个家族里是不同的画法（见 forms.ts）。
export type Family = 'minimal' | 'film' | 'terminal' | 'glass' | 'editorial' | 'mono';

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
}

const SANS = 'Inter, Noto Sans SC';
const SERIF = 'Instrument Serif, Noto Serif SC';
const NEWS = 'Newsreader, Noto Serif SC';
const MONO = 'JetBrains Mono, Noto Sans SC';

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

export const THEMES: Theme[] = [minimalLight, minimalDark, film, terminal, glass, editorial, mono];
export const themeById = (id: string) => THEMES.find((t) => t.id === id) ?? (id === 'minimal' ? minimalLight : undefined);
