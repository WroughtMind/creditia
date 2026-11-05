// The same clock describes native playback, GIF delays and standalone HTML.
import type { Art } from './export.ts';

export interface Timeline { durationMs: number; holdMs: number; fps: number; loop: boolean }
export interface Animation { frames: Art[]; times: number[]; timeline: Timeline }
export const clamp = (t: number) => Math.max(0, Math.min(1, t));

export function timelineFor(style: string, piece: string, art: Art, reducedMotion = false): Timeline {
  if (reducedMotion) return { durationMs: 0, holdMs: 0, fps: 20, loop: false };
  const durationMs = art.motion?.kind === 'scroll' ? Math.round(art.motion.secs * 1000)
    : piece === 'asterisk' ? (['ink', 'ukiyoe', 'woodblock'].includes(style) ? 2600 : 1800)
    : ['terminal', 'crt', 'arcade', 'cyber'].includes(style) ? 1400
    : ['film', 'cinema', 'gala', 'baroque', 'grimoire'].includes(style) ? 2200 : 1700;
  return { durationMs, holdMs: 1600, fps: 20, loop: true };
}

/** Fixed samples include both endpoints. Durations never depend on sample count. */
export function sampleTimes(count: number, reducedMotion = false): number[] {
  if (reducedMotion) return [1];
  if (!Number.isSafeInteger(count) || count < 1 || count > 120) throw new Error('frameCount must be from 1 to 120');
  return Array.from({ length: count + 1 }, (_, i) => i / count);
}

export function frameDelays(times: readonly number[], timeline: Timeline): number[] {
  return times.map((t, i) => Math.max(10, Math.round(i + 1 < times.length
    ? (times[i + 1] - t) * timeline.durationMs : timeline.holdMs || 10)));
}

/** Resolve a playback clock. Paused/interrupted players retain elapsedMs themselves. */
export function timeAt(elapsedMs: number, timeline: Timeline): number {
  if (timeline.durationMs <= 0) return 1;
  const cycle = timeline.durationMs + timeline.holdMs;
  const at = timeline.loop && cycle > 0 ? Math.max(0, elapsedMs) % cycle : Math.max(0, elapsedMs);
  return clamp(at / timeline.durationMs);
}

/** Material-specific petal growth; t=1 always returns the exact resting geometry. */
export function petalMotion(style: string, t: number, i: number, count: number): { growth: number; turn: number; opacity: number } {
  if (t >= 1) return { growth: 1, turn: 0, opacity: 1 };
  const stagger = count > 1 ? (i / (count - 1)) * 0.42 : 0;
  const u = clamp((t - stagger) / (1 - stagger));
  const smooth = u * u * (3 - 2 * u);
  if (['ink', 'ukiyoe', 'woodblock', 'manuscript'].includes(style)) {
    const wash = 1 - Math.pow(1 - u, 2.4);
    return { growth: wash, turn: Math.sin(u * Math.PI) * Math.sin(i * 1.7 + 0.6) * 0.012, opacity: Math.min(1, u * 3) };
  }
  if (['terminal', 'crt', 'arcade', 'cyber'].includes(style)) {
    return { growth: Math.floor(u * 12) / 12, turn: 0, opacity: u > 0 ? 1 : 0 };
  }
  if (['meadow', 'pastel', 'glass', 'neon'].includes(style)) {
    return { growth: smooth + Math.sin(u * Math.PI * 2) * (1 - u) * u * 0.22,
      turn: Math.sin(u * Math.PI) * Math.sin(i * 2.2) * 0.018, opacity: Math.min(1, u * 4) };
  }
  return { growth: 1 - Math.pow(1 - u, 3), turn: 0, opacity: Math.min(1, u * 4) };
}
