// App 预览用的字体子集（tools/fonts.mjs 生成），和 CLI 用的是同一批字体家族。
import inter400 from './fonts/Inter-400.ttf?url';
import inter500 from './fonts/Inter-500.ttf?url';
import inter600 from './fonts/Inter-600.ttf?url';
import inter700 from './fonts/Inter-700.ttf?url';
import mono400 from './fonts/JetBrainsMono-400.ttf?url';
import mono700 from './fonts/JetBrainsMono-700.ttf?url';
import iserif from './fonts/InstrumentSerif-400.ttf?url';
import news400 from './fonts/Newsreader-400.ttf?url';
import news500 from './fonts/Newsreader-500.ttf?url';
import nssc400 from './fonts/NotoSansSC-400.ttf?url';
import nssc700 from './fonts/NotoSansSC-700.ttf?url';
import nserif400 from './fonts/NotoSerifSC-400.ttf?url';

export const FONT_FILES: { name: string; weight: number; url: string }[] = [
  { name: 'Inter', weight: 400, url: inter400 },
  { name: 'Inter', weight: 500, url: inter500 },
  { name: 'Inter', weight: 600, url: inter600 },
  { name: 'Inter', weight: 700, url: inter700 },
  { name: 'JetBrains Mono', weight: 400, url: mono400 },
  { name: 'JetBrains Mono', weight: 700, url: mono700 },
  { name: 'Instrument Serif', weight: 400, url: iserif },
  { name: 'Newsreader', weight: 400, url: news400 },
  { name: 'Newsreader', weight: 500, url: news500 },
  { name: 'Noto Sans SC', weight: 400, url: nssc400 },
  { name: 'Noto Sans SC', weight: 700, url: nssc700 },
  { name: 'Noto Serif SC', weight: 400, url: nserif400 },
];
