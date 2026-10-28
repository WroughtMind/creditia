import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(new URL('./detach-dmg.sh', import.meta.url));
function detach(outcomes) {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-detach-'));
  try {
    const log = path.join(dir, 'calls'), state = path.join(dir, 'remaining');
    writeFileSync(state, outcomes.join('\n') + '\n');
    writeFileSync(path.join(dir, 'hdiutil'), `#!/bin/sh
printf '%s\\t%s\\t%s\\n' "$#" "$1" "$2" >> "$DETACH_TEST_LOG"
code=$(head -n 1 "$DETACH_TEST_STATE")
tail -n +2 "$DETACH_TEST_STATE" > "$DETACH_TEST_STATE.next"
mv "$DETACH_TEST_STATE.next" "$DETACH_TEST_STATE"
exit "$code"
`, { mode: 0o755 });
    writeFileSync(path.join(dir, 'sleep'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    const result = spawnSync('bash', [script, '/Volumes/creditia test'], {
      encoding: 'utf8',
      env: { ...process.env, PATH: dir + path.delimiter + process.env.PATH, DETACH_TEST_LOG: log, DETACH_TEST_STATE: state },
    });
    assert.ifError(result.error);
    return { status: result.status, calls: readFileSync(log, 'utf8').trim().split('\n') };
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

const expectedCall = '2\tdetach\t/Volumes/creditia test';
test('busy images are retried without forcing unmount', () => {
  assert.deepEqual(detach([16, 16, 0]), { status: 0, calls: Array(3).fill(expectedCall) });
});
test('other detach failures are returned immediately', () => {
  assert.deepEqual(detach([1, 0]), { status: 1, calls: [expectedCall] });
});
test('a persistently busy image still fails acceptance', () => {
  assert.deepEqual(detach([16, 16, 16, 0]), { status: 16, calls: Array(3).fill(expectedCall) });
});
