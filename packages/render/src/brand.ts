// 各家 logo 与熟悉的品牌色；版式家族给贡献方一套家族化的填充。
import type { Theme } from './themes.ts';

interface Brand { logo?: string; color: string; onDark?: string }
export const BRANDS: Record<string, Brand> = {
  codex: { logo: 'openai', color: '#10A37F' },
  cursor: { logo: 'cursor', color: '#0A84FF' },
  grok: { logo: 'x', color: '#26262A', onDark: '#E6E6E3' },
  'claude-code': { logo: 'claude', color: '#D97757' },
  deepseek: { logo: 'deepseek', color: '#4D6BFE' },
  kimi: { logo: 'kimi', color: '#1F2329', onDark: '#E6E6E3' },
  gemini: { logo: 'gemini', color: '#8E75B2' },
  'github-actions': { logo: 'github-actions', color: '#2088FF' },
};

/** logo 源。Node 端从磁盘读（index.ts 注册），App / 网页端从内存注册，forms 不碰文件系统。 */
export interface LogoSource { svg(name: string): string | null }
let source: LogoSource | null = null;
export function useLogoSource(s: LogoSource) { source = s; }

const svgCache = new Map<string, string | null>();
export function logoSvg(key: string): string | null {
  const b = BRANDS[key];
  if (!b?.logo) return null;
  if (!svgCache.has(b.logo)) svgCache.set(b.logo, source?.svg(b.logo) ?? null);
  return svgCache.get(b.logo)!;
}
const uriCache = new Map<string, string>();
export function logoUri(key: string, fill: string): string | null {
  const svg = logoSvg(key);
  if (!svg) return null;
  const k = key + fill;
  if (!uriCache.has(k)) uriCache.set(k, 'data:image/svg+xml;base64,' + b64(svg.replace('<svg ', `<svg fill="${fill}" `)));
  return uriCache.get(k)!;
}
function b64(s: string) {
  if (typeof Buffer !== 'undefined') return Buffer.from(s).toString('base64');
  const bytes = new TextEncoder().encode(s);
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}

/** 品牌色。photographs、embed 里也用这份。 */
export function colorOf(key: string, t: Theme, rank = 0): string {
  if (key === 'unattributed' || key === 'other') return t.faint;
  const b = BRANDS[key];
  if (!b) return t.soft;
  return t.dark && b.onDark ? b.onDark : b.color;
}

// 家族色阶：片尾字幕是奶油色、杂志是黑、单色是灰阶；其余家族用各家品牌色
const CREAM = ['#EDE6D8', '#D3CAB7', '#B8AE9B', '#978E7C', '#787162', '#5B554A'];
const INKSTEPS = ['#121110', '#48423A', '#7A7266', '#A59D90', '#CBC4B8', '#E2DBCF'];
const GRAYS = ['#000000', '#3D3D3D', '#6E6E6E', '#9A9A9A', '#BEBEBE', '#D9D9D9'];
// 第二轮体裁的家族色阶
const GOLD = ['#D6B26A', '#B99549', '#8F7233', '#6B5526', '#4A3A1A', '#332812'];
const GALA_RAMP = ['#D4AF37', '#A99BE8', '#7E6FD0', '#5B4F9E', '#3E3670', '#2A2450'];
const REDS = ['#8C1C13', '#A8352A', '#C0503F', '#D2796A', '#E0A79B', '#EBD0C9'];
const PHOSPHOR = ['#8CFFB0', '#5CD98A', '#3FAF6B', '#2C8450', '#1F5C38', '#164026'];
const ARCADE_RAMP = ['#FF4FA3', '#4FE3FF', '#FFE066', '#8CFF6B', '#B06BFF', '#FF9F45'];
const NEON_RAMP = ['#FF5D8F', '#4FE3FF', '#B06BFF', '#FFD166', '#7CFFB2', '#FF8A5B'];
const POSTER_RAMP = ['#E23B2E', '#141414', '#5A5A5A', '#8C8C8C', '#C4C4C4', '#E0E0E0'];
export function famColor(key: string, t: Theme, rank = 0): string {
  if (key === 'unattributed' || key === 'other') return t.faint;
  if (t.family === 'film') return CREAM[Math.min(rank, 5)];
  if (t.family === 'editorial') return INKSTEPS[Math.min(rank, 5)];
  if (t.family === 'mono') return GRAYS[Math.min(rank, 5)];
  if (t.family === 'cinema') return GOLD[Math.min(rank, 5)];
  if (t.family === 'gala') return GALA_RAMP[Math.min(rank, 5)];
  if (t.family === 'playbill') return REDS[Math.min(rank, 5)];
  if (t.family === 'broadsheet') return INKSTEPS[Math.min(rank, 5)];
  if (t.family === 'crt') return PHOSPHOR[Math.min(rank, 5)];
  if (t.family === 'arcade') return ARCADE_RAMP[Math.min(rank, 5)];
  if (t.family === 'neon') return NEON_RAMP[Math.min(rank, 5)];
  if (t.family === 'poster') return POSTER_RAMP[Math.min(rank, 5)];
  return colorOf(key, t, rank);
}
