// Every desktop material has a named, deterministic entrance. Piece renderers
// still own their data motion (shares, rings, cells, chronology and scrolling).
export type MaterialMotion = 'line' | 'split' | 'glass' | 'shutter' | 'curtain' | 'columns' | 'page' | 'iris'
  | 'blocks' | 'scan' | 'neon' | 'pixels' | 'ink' | 'bands' | 'press' | 'writing' | 'bubbles' | 'folds' | 'leaves' | 'panels' | 'slices';
export interface MotionProfile {
  material: MaterialMotion; durationMs: number; starMs: number; stagger: number; parts: number; distance: number; steps?: number;
}
const profile = (material: MaterialMotion, durationMs: number, starMs: number, stagger: number, parts: number, distance: number, steps?: number): MotionProfile =>
  ({ material, durationMs, starMs, stagger, parts, distance, ...(steps ? { steps } : {}) });
export const MOTION_PROFILES: Record<string, MotionProfile> = {
  'minimal-light': profile('line', 1500, 1800, .24, 1, 6),
  'minimal-dark': profile('split', 1600, 1800, .26, 2, 8),
  glass: profile('glass', 1900, 2100, .36, 5, 16),
  film: profile('shutter', 2200, 2200, .32, 2, 22),
  cinema: profile('curtain', 2400, 2300, .38, 2, 28),
  editorial: profile('columns', 1700, 1900, .34, 2, 18),
  broadsheet: profile('columns', 1800, 2000, .44, 4, 12, 8),
  playbill: profile('page', 2000, 2100, .38, 1, 20),
  gala: profile('iris', 2400, 2400, .42, 8, 18),
  mono: profile('line', 1300, 1700, .22, 1, 10, 10),
  poster: profile('blocks', 1500, 1800, .36, 3, 32),
  terminal: profile('scan', 1400, 1800, .5, 18, 0, 12),
  crt: profile('scan', 1600, 1800, .38, 38, 0, 24),
  neon: profile('neon', 2100, 2300, .36, 1, 8),
  arcade: profile('pixels', 1400, 1800, .46, 10, 12, 8),
  ink: profile('ink', 2600, 2600, .5, 1, 10),
  ukiyoe: profile('bands', 2400, 2600, .45, 7, 14),
  woodblock: profile('press', 2100, 2600, .38, 6, 6, 16),
  baroque: profile('iris', 2400, 2500, .38, 12, 16),
  grimoire: profile('page', 2600, 2700, .48, 2, 24),
  manuscript: profile('writing', 2300, 2500, .55, 12, 5),
  pastel: profile('bubbles', 1900, 2100, .38, 7, 10),
  cutout: profile('folds', 1700, 2000, .42, 4, 26),
  meadow: profile('leaves', 2300, 2500, .45, 5, 18),
  manga: profile('panels', 1600, 1900, .4, 4, 28),
  cyber: profile('slices', 1500, 1900, .5, 9, 20, 12),
};
const aliases: Record<string, string> = { minimal: 'minimal-light', noir: 'minimal-dark' };
export const motionProfile = (style: string): MotionProfile => MOTION_PROFILES[aliases[style] ?? style] ?? MOTION_PROFILES['minimal-light'];
export const unit = (t: number) => Math.max(0, Math.min(1, t));
/** Delays depend on position, never randomness or the host's frame rate. */
export function materialProgress(profile: MotionProfile, t: number, index = 0, count = 1): number {
  const start = count > 1 ? profile.stagger * index / (count - 1) : 0;
  const raw = unit((t - start) / (1 - start));
  const eased = 1 - Math.pow(1 - raw, profile.material === 'ink' || profile.material === 'writing' ? 1.6 : 2.4);
  return profile.steps && t < 1 ? Math.floor(eased * profile.steps) / profile.steps : eased;
}
