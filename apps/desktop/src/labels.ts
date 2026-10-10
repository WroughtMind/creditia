// App 里给人看的名字。渲染器内部 id 不变，只在界面上换成大白话。

export const THEME_ZH: Record<string, string> = {
  'minimal-light': '简洁·浅', 'minimal-dark': '简洁·深', film: '电影胶片', terminal: '终端', glass: '毛玻璃',
  editorial: '杂志', mono: '黑白', cinema: '老电影片尾', playbill: '剧院节目单', gala: '颁奖典礼',
  broadsheet: '老报纸', crt: '绿屏终端', arcade: '街机', neon: '霓虹灯', poster: '包豪斯海报',
};

export const VARIANT_ZH: Record<string, Record<string, string>> = {
  roll: {
    classic: '经典居中', columns: '左右两栏', billing: '海报署名块', title: '标题卡', crawl: '星战滚字',
    cast: '演员表', scroll: '双栏滚动', cards: '一张张卡片', end: '剧终', block: '紧凑署名块', program: '节目单',
  },
  receipt: {
    slip: '收银条', thermal: '热敏小票', folio: '对折账单', stub: '票根', ticket: '电影票', ledger: '账本', scroll: '长卷纸',
  },
};

/** [值, 显示名] */
export const SIZES: [string, string][] = [
  ['native', '原始大小'],
  ['1200x630', '链接预览 1200×630'],
  ['1080x1350', '竖版帖子 1080×1350'],
  ['1080x1920', '手机全屏 1080×1920'],
  ['1080x1080', '正方形 1080×1080'],
];

/** 「在访达中显示」按平台叫法不同 */
export function revealLabel(ua = navigator.userAgent) {
  if (/Mac/i.test(ua)) return '在访达中显示';
  if (/Win/i.test(ua)) return '在资源管理器中显示';
  return '打开所在文件夹';
}
