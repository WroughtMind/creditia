import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runInNewContext } from 'node:vm';
import { scrollPaperPose, scrollPaperTexture } from '../src/scroll-paper.ts';

test('纸墨长卷的两函数可独立注入离线播放器，完整底墨和最后像素保持', () => {
  // The HTML exporter serializes functions, so an accidental imported helper
  // would work in Node/App but fail only in the user's offline HTML playback.
  const pose = runInNewContext(`(${scrollPaperPose.toString()})`) as typeof scrollPaperPose;
  const texture = runInNewContext(`(${scrollPaperTexture.toString()})`) as typeof scrollPaperTexture;
  for (const style of ['ink', 'ukiyoe', 'woodblock', 'baroque', 'grimoire', 'manuscript']) {
    for (const role of ['role', 'credits', 'row']) {
      assert.equal(JSON.stringify(pose(style, role, 1)), JSON.stringify({ dx: 0, dy: 0, opacity: 1 }));
      for (const p of [.1, .25, .5, .75]) {
        const plate = pose(style, role, p);
        assert.ok(plate.opacity > 0 && plate.opacity <= 1, `${style}: complete letter face is present`);
        for (let y = -11; y < 42; y++) for (let x = -11; x < 42; x++) {
          const ink = texture(style, p, x, y);
          assert.ok(ink >= .58 && ink <= 1, `${style}: no texture hole at ${x},${y}`);
          assert.equal(ink, texture(style, p, x, y), 'the pigment stays in place between frames');
          assert.equal(texture(style, 1, x, y), 1, 'the final atlas ink is unchanged');
        }
      }
    }
  }
});
