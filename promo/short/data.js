// 片中的数据：样例项目 catnap（packages/render/fixtures/catnap.json，官网用的同一份）
export const AGENTS = [
  { key: 'claude-code', name: 'Claude Code', model: 'Claude Opus 5.5', color: '#D97757', pct: 50.9 },
  { key: 'codex', name: 'Codex', model: 'GPT-5.6 Sol', color: '#10A37F', pct: 30.8 },
  { key: 'cursor', name: 'Cursor', model: 'Composer 2', color: '#4C8DF6', pct: 9.1 },
  { key: 'kimi', name: 'Kimi', model: 'Kimi K3', color: '#8C7CFF', pct: 4.7 },
  { key: 'grok', name: 'Grok', model: 'Grok 5 Code', color: '#E8B730', pct: 2.1 },
];
export const UNATTRIBUTED = { pct: 2.5, color: '#8A8478' };
export const UN = AGENTS.length; // 「未确认」的下标：这样的行没有脚注
export const LINES = 50190;

// src/spots/pick.ts：官网「出处」那一节的同一个文件。[代码, 写它的工具]
export const FILE = [
  ['export function pickSpot(cat: Cat, room: Room) {', 0],
  ['  let best = { spot: room.door, score: -Infinity }', 0],
  ['  for (const spot of room.spots) {', 0],
  ['    const warmth = spot.sunlight * 2 + spot.softness', 1],
  ['    const score = warmth - (cat.grudges[spot.id] ?? 0)', 1],
  ['    if (spot.isKeyboard) return spot', 2],
  ['    if (score > best.score) best = { spot, score }', 0],
  ['  }', 0],
  ['  return best.spot', 0],
  ['}', 0],
  ['', UN],
  ['// 3am zoomies are a feature, not a bug', 2],
  ['export function zoomies(cat: Cat, now: Date) {', 2],
  ['  const odds = now.getHours() === 3 ? 0.9 : 0.05', 1],
  ['  return !cat.asleep && Math.random() < odds', UN],
  ['}', UN],
];
export const KEYWORDS = /\b(export|async|function|const|let|await|for|of|if|return)\b/g;

// 26 种风格的同一张分享图（promo/short/prints.sh 导出），扇面里从左到右的顺序：深浅、冷暖错开
export const PRINTS = [
  'cinema', 'ukiyoe', 'minimal-light', 'neon', 'manuscript', 'terminal', 'pastel', 'baroque', 'glass', 'poster',
  'woodblock', 'crt', 'editorial', 'grimoire', 'meadow', 'film', 'cutout', 'arcade', 'broadsheet', 'manga',
  'minimal-dark', 'playbill', 'ink', 'gala', 'mono', 'cyber',
];
