import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { apps, cores, coreFile, assemble, checkVersion, version } from './release.mjs';

test('incomplete candidates cannot be assembled', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-release-'));
  try { assert.throws(() => assemble(dir), /ENOENT/); }
  finally { rmSync(dir, { recursive: true, force: true }); }
});
test('checksums and Homebrew cask refer to the final architecture-specific bytes', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'creditia-release-'));
  try {
    for (const f of [...cores.map(coreFile), ...apps.map(a => a.file)]) writeFileSync(path.join(dir, f), f);
    assemble(dir);
    for (const a of apps) assert.equal(readFileSync(path.join(dir, a.file + '.sha256'), 'utf8'), `${createHash('sha256').update(a.file).digest('hex')}  ${a.file}\n`);
    const cask = readFileSync(path.join(dir, 'creditia.rb'), 'utf8');
    assert.ok(cask.includes(`version "${version}"`));
    for (const a of apps.filter(a => a.os === 'mac')) assert.ok(cask.includes(createHash('sha256').update(a.file).digest('hex')));
    assert.ok(!cask.includes('@VERSION@') && !cask.includes('xattr') && !cask.includes('postflight'));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
test('a different release tag cannot package this version', () => {
  checkVersion(`v${version}`);
  assert.throws(() => checkVersion('v999.0.0'), /does not match/);
});
