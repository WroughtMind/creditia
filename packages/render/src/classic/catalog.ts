// 给 App、官网、命令行看的目录：主题、作品、版式的中英文名字，主题圆点。不碰文件系统，也不碰 satori。
import { THEMES, type Theme } from './themes.ts';

export const THEME_INFO: Record<string, { zh: string; en: string; note: string; noteEn: string }> = {
  'minimal-light': { zh: '极简', en: 'Minimal', note: '白底黑字，什么场合都不出错', noteEn: 'Plain and safe anywhere' },
  'minimal-dark': { zh: '极简 · 暗', en: 'Minimal Dark', note: '同一套排版的深色版', noteEn: 'The same layout, dark' },
  glass: { zh: '玻璃', en: 'Glass', note: '磨砂卡片，背后透出各家的颜色', noteEn: 'Frosted cards over each tool’s colour' },
  film: { zh: '胶片', en: 'Film', note: '奶油色字，老电影的片尾', noteEn: 'Cream type, old film credits' },
  cinema: { zh: '影院', en: 'Cinema', note: '黑底金字，正式的演职员表', noteEn: 'Gold on black, a formal cast list' },
  editorial: { zh: '杂志', en: 'Editorial', note: '衬线大字和细线，像一页杂志', noteEn: 'Serif headlines and hairlines' },
  broadsheet: { zh: '报纸', en: 'Broadsheet', note: '报头、栏线、旧纸', noteEn: 'Mastheads, column rules, old paper' },
  playbill: { zh: '节目单', en: 'Playbill', note: '剧场节目单的红与米色', noteEn: 'Theatre programme red and cream' },
  gala: { zh: '颁奖礼', en: 'Gala', note: '深紫和金，颁奖典礼', noteEn: 'Deep violet and gold' },
  mono: { zh: '单色', en: 'Mono', note: '只有黑白灰', noteEn: 'Black, white and grey only' },
  poster: { zh: '海报', en: 'Poster', note: '粗黑框和红，瑞士海报', noteEn: 'Heavy frames and red, Swiss poster' },
  terminal: { zh: '终端', en: 'Terminal', note: '等宽字和提示符', noteEn: 'Monospace and prompts' },
  crt: { zh: '磷光屏', en: 'CRT', note: '绿色磷光和扫描线', noteEn: 'Green phosphor and scanlines' },
  neon: { zh: '霓虹', en: 'Neon', note: '暗处发光的招牌', noteEn: 'Signs glowing in the dark' },
  arcade: { zh: '街机', en: 'Arcade', note: '像素字和高分榜', noteEn: 'Pixel type and high scores' },
};

/** 圆点按主题自己的底色、线色、强调色画，一眼能认出是哪种 */
export function themeDot(t: Theme): string {
  const ring = t.dark ? 'rgba(255,255,255,.22)' : 'rgba(0,0,0,.16)';
  const glow = t.glow ? '<circle cx="9" cy="10" r="5" fill="#D97757" opacity=".55"/><circle cx="15" cy="14" r="5" fill="#10A37F" opacity=".45"/>' : '';
  const mark = t.family === 'terminal' || t.family === 'crt'
    ? `<path d="M7.5 9l3 3-3 3M12.5 15.5h4" stroke="${t.accent}" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>`
    : t.family === 'arcade' ? `<rect x="9" y="9" width="6" height="6" fill="${t.accent}"/>`
      : t.family === 'poster' ? `<rect x="6.5" y="6.5" width="11" height="11" fill="none" stroke="${t.ink}" stroke-width="2"/><circle cx="12" cy="12" r="2.6" fill="${t.accent}"/>`
        : t.family === 'film' || t.family === 'cinema' || t.family === 'gala' ? `<path d="M12 7.5l1.3 3.2 3.2 1.3-3.2 1.3L12 16.5l-1.3-3.2L7.5 12l3.2-1.3z" fill="${t.accent}"/>`
          : t.family === 'editorial' || t.family === 'broadsheet' || t.family === 'playbill' ? `<path d="M7 9h10M7 12h10M7 15h6" stroke="${t.accent}" stroke-width="1.4"/>`
            : t.family === 'neon' ? `<circle cx="12" cy="12" r="4" fill="none" stroke="${t.accent}" stroke-width="1.6"/>`
              : `<circle cx="12" cy="12" r="3.2" fill="${t.accent}"/>`;
  return `<circle cx="12" cy="12" r="11" fill="${t.bg}" stroke="${ring}"/>${glow}${mark}`;
}

export const themeName = (t: Theme, lang: 'zh' | 'en') => THEME_INFO[t.id]?.[lang] ?? t.name;
export const themeNote = (t: Theme, lang: 'zh' | 'en') => (lang === 'zh' ? THEME_INFO[t.id]?.note : THEME_INFO[t.id]?.noteEn) ?? '';
/** 展示顺序：常用的放前面 */
export const THEME_ORDER = ['minimal-light', 'minimal-dark', 'glass', 'film', 'cinema', 'editorial', 'broadsheet', 'playbill', 'gala', 'mono', 'poster', 'terminal', 'crt', 'neon', 'arcade'];
export const ORDERED_THEMES: Theme[] = THEME_ORDER.map((id) => THEMES.find((t) => t.id === id)!).filter(Boolean);

export const PIECE_INFO: Record<string, { zh: string; en: string; q: string; qEn: string }> = {
  tier: { zh: '署名榜', en: 'Credits', q: '谁写的、各占多少', qEn: 'Who wrote it, and how much' },
  share: { zh: '分享图', en: 'Share card', q: '链接预览、社交媒体', qEn: 'Link previews and social posts' },
  byline: { zh: '署名行', en: 'Byline', q: '页脚、关于页', qEn: 'Footers and about pages' },
  badge: { zh: '徽章', en: 'Badge', q: 'README 顶部', qEn: 'Top of a README' },
  roll: { zh: '片尾字幕', en: 'End credits', q: '谁担任了什么角色', qEn: 'Who played which part' },
  receipt: { zh: '小票', en: 'Receipt', q: '花了多少精力', qEn: 'What it took' },
  ribbon: { zh: '时间织带', en: 'Ribbon', q: '怎么一步步做出来的', qEn: 'How it was built, step by step' },
  mosaic: { zh: '代码马赛克', en: 'Mosaic', q: '现在的代码由谁构成', qEn: 'Who the code is made of now' },
  fetch: { zh: '终端名片', en: 'creditia fetch', q: '终端里的一张名片', qEn: 'A card for the terminal' },
  app: { zh: 'App 致谢页', en: 'In-app credits', q: '原生 App 的致谢页', qEn: 'A native app credits screen' },
  waffle: { zh: '华夫饼图', en: 'Waffle', q: '一格 1%', qEn: 'One square, one percent' },
  donut: { zh: '环形图', en: 'Donut', q: '占比', qEn: 'Shares' },
  area: { zh: '堆叠面积图', en: 'Weekly area', q: '每周新增的代码', qEn: 'Lines added each week' },
  lanes: { zh: '泳道图', en: 'Lanes', q: '各工具的会话', qEn: 'Sessions per tool' },
  clock: { zh: '作息图', en: 'Clock', q: '星期 × 小时', qEn: 'Weekday × hour' },
  versions: { zh: '版本柱状图', en: 'Versions', q: '每个版本谁写的', qEn: 'Who wrote each release' },
  settle: { zh: '沉淀', en: 'Survival', q: '写下的代码留下多少', qEn: 'How much of it stayed' },
};
export const pieceName = (id: string, lang: 'zh' | 'en') => PIECE_INFO[id]?.[lang] ?? id;
export const pieceQ = (id: string, lang: 'zh' | 'en') => (lang === 'zh' ? PIECE_INFO[id]?.q : PIECE_INFO[id]?.qEn) ?? '';

export const VARIANT_INFO: Record<string, [string, string]> = {
  // 片尾字幕
  classic: ['居中', 'Centered'], columns: ['双栏', 'Two columns'], billing: ['海报署名', 'Billing'], title: ['标题卡', 'Title card'],
  crawl: ['滚字', 'Crawl'], cast: ['演员表', 'Cast'], scroll: ['长卷', 'Scroll'], cards: ['字卡', 'Cards'], end: ['终', 'The End'],
  block: ['署名块', 'Billing block'], program: ['节目单', 'Programme'],
  opening: ['片头', 'Opening titles'], production: ['出品', 'Production card'], slate: ['场记板', 'Clapperboard'], crew: ['部门', 'Departments'],
  // 小票
  slip: ['收银条', 'Slip'], thermal: ['热敏纸', 'Thermal'], folio: ['账页', 'Folio'], stub: ['票根', 'Stub'], ticket: ['电影票', 'Ticket'],
  ledger: ['账本', 'Ledger'],
};
export const variantName = (v: string, lang: 'zh' | 'en') => VARIANT_INFO[v]?.[lang === 'zh' ? 0 : 1] ?? v;
