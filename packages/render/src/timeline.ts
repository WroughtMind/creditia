// The same clock describes native playback, GIF delays and standalone HTML.
import type { Art } from './export.ts';
import { motionProfile } from './motion-profiles.ts';
import { motionDuration } from './motion.ts';

export interface Timeline { durationMs: number; holdMs: number; fps: number; loop: boolean }
export interface Animation { frames: Art[]; times: number[]; timeline: Timeline }
export const clamp = (t: number) => Math.max(0, Math.min(1, t));

export function timelineFor(style: string, piece: string, art: Art, reducedMotion = false): Timeline {
  if (reducedMotion) return { durationMs: 0, holdMs: 0, fps: 20, loop: false };
  const durationMs = art.motion?.kind === 'scroll' ? Math.round(art.motion.secs * 1000)
    : motionDuration(piece, art.selection?.variant, style);
  return { durationMs, holdMs: 1600, fps: 20, loop: true };
}

/** Fixed samples include both endpoints. Durations never depend on sample count. */
export function sampleTimes(count: number, reducedMotion = false): number[] {
  if (reducedMotion) return [1];
  if (!Number.isSafeInteger(count) || count < 1 || count > 120) throw new Error('frameCount must be from 1 to 120');
  return Array.from({ length: count + 1 }, (_, i) => i / count);
}

export function frameDelays(times: readonly number[], timeline: Timeline): number[] {
  // GIF stores centiseconds. Quantize cumulative endpoints, so rounding never
  // speeds up a long clip when many small intervals round in the same direction.
  return times.map((t, i) => 10 * Math.max(1, i + 1 < times.length
    ? Math.round(times[i + 1] * timeline.durationMs / 10) - Math.round(t * timeline.durationMs / 10)
    : Math.round(timeline.holdMs / 10)));
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
  const profile = motionProfile(style), kind = profile.material;
  const order = kind === 'split' ? (i % 2 ? Math.ceil(count / 2) + Math.floor(i / 2) : Math.floor(i / 2)) : i;
  const stagger = count > 1 ? (order / (count - 1)) * profile.stagger : 0;
  const u = clamp((t - stagger) / (1 - stagger));
  const smooth = u * u * (3 - 2 * u);
  const turn = Math.min(.018, .22 / Math.max(1, count));
  if (['ink', 'bands', 'press', 'writing'].includes(kind)) {
    const wash = 1 - Math.pow(1 - u, 2.4);
    return { growth: profile.steps ? Math.floor(wash * profile.steps) / profile.steps : wash,
      turn: Math.sin(u * Math.PI) * Math.sin(i * 1.7 + 0.6) * turn * .65, opacity: Math.min(1, u * 3) };
  }
  if (['scan', 'pixels', 'slices'].includes(kind)) {
    const steps = profile.steps ?? 12;
    return { growth: Math.floor(u * steps) / steps, turn: 0, opacity: u > 0 ? 1 : 0 };
  }
  if (['leaves', 'bubbles', 'glass'].includes(kind)) {
    return { growth: smooth + Math.sin(u * Math.PI * 2) * (1 - u) * u * 0.22,
      turn: Math.sin(u * Math.PI) * Math.sin(i * 2.2) * turn, opacity: Math.min(1, u * 4) };
  }
  if (['folds', 'blocks', 'panels', 'page'].includes(kind)) return {
    growth: 1 - Math.pow(1 - u, 3), turn: (1 - smooth) * (i % 2 ? 1 : -1) * turn * .7, opacity: u > 0 ? 1 : 0,
  };
  if (['iris', 'curtain', 'shutter'].includes(kind)) return {
    growth: smooth, turn: (1 - smooth) * turn * .6, opacity: Math.min(1, u * 5),
  };
  if (kind === 'neon') return { growth: smooth, turn: 0, opacity: smooth * smooth };
  const growth = 1 - Math.pow(1 - u, 3);
  return { growth: profile.steps ? Math.floor(growth * profile.steps) / profile.steps : growth, turn: 0, opacity: Math.min(1, u * 4) };
}
