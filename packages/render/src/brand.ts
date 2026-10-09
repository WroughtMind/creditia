// 各家 logo 与熟悉的品牌色
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { Theme } from './themes.ts';

const DIR = path.join(import.meta.dirname, '../assets/logos');
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

const svgCache = new Map<string, string>();
export function logoSvg(key: string): string | null {
  const b = BRANDS[key];
  if (!b?.logo) return null;
  if (!svgCache.has(b.logo)) svgCache.set(b.logo, readFileSync(path.join(DIR, `${b.logo}.svg`), 'utf8'));
  return svgCache.get(b.logo)!;
}
const uriCache = new Map<string, string>();
export function logoUri(key: string, fill: string): string | null {
  const svg = logoSvg(key);
  if (!svg) return null;
  const k = key + fill;
  if (!uriCache.has(k)) uriCache.set(k, 'data:image/svg+xml;base64,' + Buffer.from(svg.replace('<svg ', `<svg fill="${fill}" `)).toString('base64'));
  return uriCache.get(k)!;
}

/** 贡献方在该风格下的颜色。单色风格按排名给灰阶 */
export function colorOf(key: string, t: Theme, rank = 0): string {
  if (key === 'unattributed') return t.faint;
  if (key === 'other') return t.faint;
  if (t.mono) return ['#000000', '#4A4A4A', '#7A7A7A', '#A3A3A3', '#C2C2C2', '#D6D6D6'][Math.min(rank, 5)];
  const b = BRANDS[key];
  if (!b) return t.soft;
  return t.dark && b.onDark ? b.onDark : b.color;
}
