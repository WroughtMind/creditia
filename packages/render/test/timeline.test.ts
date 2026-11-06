import { test } from 'node:test';
import assert from 'node:assert/strict';
import { timelineFor, sampleTimes, frameDelays, timeAt, petalMotion } from '../src/timeline.ts';

test('GIF delays and native/HTML clocks share duration independently of sample count', () => {
  const timeline = { durationMs: 2600, holdMs: 1600, fps: 20, loop: true };
  for (const count of [4, 24, 40, 60, 120]) {
    const times = sampleTimes(count), delays = frameDelays(times, timeline);
    assert.equal(delays.slice(0, -1).reduce((a, b) => a + b, 0), 2600);
    assert.ok(delays.every(delay => delay % 10 === 0));
    assert.equal(delays.at(-1), 1600);
  }
  assert.equal(timeAt(1300, timeline), .5);
  assert.equal(timeAt(3000, timeline), 1);
  assert.equal(timeAt(4200, timeline), 0);
  assert.equal(timeAt(9000, { ...timeline, loop: false }), 1);
  const reduced = timelineFor('ink', 'asterisk', {} as any, true);
  assert.equal(timeAt(0, reduced), 1);
  assert.deepEqual(sampleTimes(24, true), [1]);
});

test('petals stagger, grow organically and return exact resting geometry for every material', () => {
  for (const style of ['ink', 'ukiyoe', 'woodblock', 'meadow', 'pastel', 'glass', 'arcade', 'terminal', 'cinema', 'minimal-light']) {
    assert.deepEqual(petalMotion(style, 1, 3, 5), { growth: 1, turn: 0, opacity: 1 });
    assert.equal(petalMotion(style, 0, 0, 5).opacity, 0);
    assert.equal(petalMotion(style, .1, 4, 5).opacity, 0);
    const mid = petalMotion(style, .5, 0, 5);
    assert.ok(mid.growth > 0 && mid.growth < 1.2);
  }
  assert.notDeepEqual(petalMotion('ink', .5, 2, 5), petalMotion('terminal', .5, 2, 5));
});
