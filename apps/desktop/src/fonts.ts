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
import cinzel500 from './fonts/Cinzel-500.ttf?url';
import cinzel700 from './fonts/Cinzel-700.ttf?url';
import cinzelDec700 from './fonts/CinzelDecorative-700.ttf?url';
import playfair500 from './fonts/PlayfairDisplay-500.ttf?url';
import playfair700 from './fonts/PlayfairDisplay-700.ttf?url';
import playfair900 from './fonts/PlayfairDisplay-900.ttf?url';
import baskerville400 from './fonts/LibreBaskerville-400.ttf?url';
import baskerville700 from './fonts/LibreBaskerville-700.ttf?url';
import oswald400 from './fonts/Oswald-400.ttf?url';
import oswald600 from './fonts/Oswald-600.ttf?url';
import poiret400 from './fonts/PoiretOne-400.ttf?url';
import limelight400 from './fonts/Limelight-400.ttf?url';
import unifraktur400 from './fonts/UnifrakturMaguntia-400.ttf?url';
import vt323400 from './fonts/VT323-400.ttf?url';
import pressStart400 from './fonts/PressStart2P-400.ttf?url';
import monoton400 from './fonts/Monoton-400.ttf?url';
import archivoBlack400 from './fonts/ArchivoBlack-400.ttf?url';
import anton400 from './fonts/Anton-400.ttf?url';
import zcoolXiaoWei400 from './fonts/ZCOOLXiaoWei-400.ttf?url';
import zcoolQingKe400 from './fonts/ZCOOLQingKeHuangYou-400.ttf?url';
import maShanZheng400 from './fonts/MaShanZheng-400.ttf?url';

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
  { name: 'Cinzel', weight: 500, url: cinzel500 },
  { name: 'Cinzel', weight: 700, url: cinzel700 },
  { name: 'Cinzel Decorative', weight: 700, url: cinzelDec700 },
  { name: 'Playfair Display', weight: 500, url: playfair500 },
  { name: 'Playfair Display', weight: 700, url: playfair700 },
  { name: 'Playfair Display', weight: 900, url: playfair900 },
  { name: 'Libre Baskerville', weight: 400, url: baskerville400 },
  { name: 'Libre Baskerville', weight: 700, url: baskerville700 },
  { name: 'Oswald', weight: 400, url: oswald400 },
  { name: 'Oswald', weight: 600, url: oswald600 },
  { name: 'Poiret One', weight: 400, url: poiret400 },
  { name: 'Limelight', weight: 400, url: limelight400 },
  { name: 'UnifrakturMaguntia', weight: 400, url: unifraktur400 },
  { name: 'VT323', weight: 400, url: vt323400 },
  { name: 'Press Start 2P', weight: 400, url: pressStart400 },
  { name: 'Monoton', weight: 400, url: monoton400 },
  { name: 'Archivo Black', weight: 400, url: archivoBlack400 },
  { name: 'Anton', weight: 400, url: anton400 },
  { name: 'ZCOOL XiaoWei', weight: 400, url: zcoolXiaoWei400 },
  { name: 'ZCOOL QingKe HuangYou', weight: 400, url: zcoolQingKe400 },
  { name: 'Ma Shan Zheng', weight: 400, url: maShanZheng400 },
];
