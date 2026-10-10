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
export function famColor(key: string, t: Theme, rank = 0): string {
  if (key === 'unattributed' || key === 'other') return t.faint;
  if (t.family === 'film') return CREAM[Math.min(rank, 5)];
  if (t.family === 'editorial') return INKSTEPS[Math.min(rank, 5)];
  if (t.family === 'mono') return GRAYS[Math.min(rank, 5)];
  return colorOf(key, t, rank);
}
