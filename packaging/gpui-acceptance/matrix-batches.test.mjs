import test from 'node:test';
import assert from 'node:assert/strict';
import { partitions } from './matrix-batches.mjs';
test('sequential process batches cover exactly the original shard with no duplicates', () => {
  for (let shard = 0; shard < 8; shard++) {
    const original = Array.from({ length: 1738 }, (_, i) => i).filter(i => i % 8 === shard);
    const expanded = partitions(8, shard, 8).flatMap(p => Array.from({ length: 1738 }, (_, i) => i).filter(i => i % p.shards === p.shard));
    assert.equal(new Set(expanded).size, expanded.length);
    assert.deepEqual(expanded.sort((a, b) => a - b), original);
  }
  assert.throws(() => partitions(8, 8, 1), /Invalid/);
});
