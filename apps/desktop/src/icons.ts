// App 外壳的图标：同一套规矩画——16 的网格、1.5 的描边、圆头圆角，有效区域 12×12（四边各留 2）。
// 颜色跟着 currentColor；作品里的图标不在这里。

/** 齿轮：六个齿，按 16 网格算出来，描边粗细和别的图标一致（照搬 24 网格的图标缩小会显得细一档） */
const gear = (() => {
  const c = 8, ro = 6.1, ri = 4.7, teeth = 6, pts: string[] = [];
  for (let i = 0; i < teeth; i++) {
    const a = (i / teeth) * 2 * Math.PI - Math.PI / 2, w = Math.PI / teeth;
    for (const [r, t] of [[ri, a - w * 0.62], [ro, a - w * 0.3], [ro, a + w * 0.3], [ri, a + w * 0.62]] as const) {
      pts.push(`${(c + r * Math.cos(t)).toFixed(2)} ${(c + r * Math.sin(t)).toFixed(2)}`);
    }
  }
  return `<path d="M${pts.join('L')}Z"/><circle cx="8" cy="8" r="1.9"/>`;
})();

const PATHS: Record<string, string> = {
  /** 墙：错落的四块 */
  wall: '<rect x="2.75" y="2.75" width="4.5" height="6" rx="1"/><rect x="8.75" y="2.75" width="4.5" height="3.5" rx="1"/><rect x="2.75" y="10.25" width="4.5" height="3" rx="1"/><rect x="8.75" y="7.75" width="4.5" height="5.5" rx="1"/>',
  /** 漫游：中间一块，四面的都伸出框外——画布没有边 */
  roam: '<rect x="6" y="6" width="4" height="4" rx="1"/><path d="M1.75 6H4v4H1.75M14.25 6H12v4h2.25M6 1.75V4h4V1.75M6 14.25V12h4v2.25"/>',
  gear,
  close: '<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>',
  prev: '<path d="M9.75 3.75 5.5 8l4.25 4.25"/>',
  next: '<path d="M6.25 3.75 10.5 8l-4.25 4.25"/>',
  updown: '<path d="M5.5 6.25 8 3.75l2.5 2.5M5.5 9.75 8 12.25l2.5-2.5"/>',
  more: '<circle cx="3.75" cy="8" r=".6"/><circle cx="8" cy="8" r=".6"/><circle cx="12.25" cy="8" r=".6"/>',
  search: '<circle cx="7.25" cy="7.25" r="4.25"/><path d="M10.5 10.5l3 3"/>',
  addFolder: '<path d="M2.25 4.75a1 1 0 0 1 1-1h2.7l1.4 1.5h5.4a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3.25a1 1 0 0 1-1-1z"/><path d="M8 7.5v3.5M6.25 9.25h3.5"/>',
  folder: '<path d="M2.25 4.75a1 1 0 0 1 1-1h2.7l1.4 1.5h5.4a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3.25a1 1 0 0 1-1-1z"/>',
  refresh: '<path d="M13.25 8a5.25 5.25 0 1 1-1.54-3.71"/><path d="M13.25 2.5v3.25H10"/>',
  export: '<path d="M8 2.5V10M5 5.25l3-3 3 3"/><path d="M3 9.75v2.75a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V9.75"/>',
  copy: '<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 3.5v-.25a1 1 0 0 0-1-1H3.5a1 1 0 0 0-1 1V9.5a1 1 0 0 0 1 1h.25"/>',
  link: '<path d="M6.75 9.25 9.25 6.75"/><path d="M8.5 4.25l.9-.9a2.6 2.6 0 0 1 3.7 3.7l-.9.9M7.5 11.75l-.9.9a2.6 2.6 0 0 1-3.7-3.7l.9-.9"/>',
  remove: '<path d="M2.75 4.5h10.5M6.25 4.5V3a.75.75 0 0 1 .75-.75h2a.75.75 0 0 1 .75.75v1.5M4.25 4.5l.6 8.3a1 1 0 0 0 1 .95h4.3a1 1 0 0 0 1-.95l.6-8.3"/>',
  info: '<circle cx="8" cy="8" r="5.75"/><path d="M8 7.25v3.75"/><circle cx="8" cy="5" r=".4"/>',
  home: '<circle cx="8" cy="8" r="2.25"/><path d="M8 1.75v2.5M8 11.75v2.5M1.75 8h2.5M11.75 8h2.5"/>',
  check: '<path d="M3.5 8.5l3 3 6-6.5"/>',
};

/** 取一个图标；size 只改显示大小，线宽按 16 网格算 */
export function icon(name: keyof typeof PATHS | string, size = 16, cls = ''): string {
  return `<svg class="ic${cls ? ' ' + cls : ''}" width="${size}" height="${size}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${PATHS[name] ?? ''}</svg>`;
}
