import { test } from 'node:test';
import assert from 'node:assert/strict';
import { downloads } from './downloads.mjs';
import { apps, version } from '../packaging/release.mjs';
const response = (data, status = 200) => new Response(typeof data === 'string' ? data : JSON.stringify(data), { status });
const release = { tag_name: `v${version}`, draft: false, prerelease: false,
  assets: apps.flatMap(a => [a.file, a.file + '.sha256'].map(name => ({ name, browser_download_url: `https://github.com/WroughtMind/creditia/releases/download/v${version}/${name}` }))) };
const request = (rel, npm = null, cask = null) => async url => {
  const value = url.includes('api.github.com') ? rel : url.includes('registry.npmjs.org') ? npm : cask;
  return value === null ? response('', 404) : response(value);
};
test('no published channels produces no downloads or install commands', async () => {
  const r = await downloads(request(null));
  assert.deepEqual(r.apps, []); assert.equal(r.npm, false); assert.equal(r.brew, false);
});
test('a draft, prerelease or wrong-version release is never advertised', async () => {
  for (const change of [{ draft: true }, { prerelease: true }, { tag_name: 'v99.0.0' }]) {
    assert.deepEqual((await downloads(request({ ...release, ...change }))).apps, []);
  }
});
test('an installer requires its checksum; npm availability is separate', async () => {
  const rel = { ...release, assets: release.assets.filter(a => a.name !== apps[0].file + '.sha256') };
  const r = await downloads(request(rel, { name: 'creditia', version }));
  assert.equal(r.apps.length, apps.length - 1); assert.equal(r.npm, true); assert.equal(r.brew, false);
});
test('a placeholder cask is not offered as an installation channel', async () => {
  const cask = `version "${version}"\nWroughtMind/creditia/releases/download/\nsha256 arm: "${'0'.repeat(64)}", intel: "${'0'.repeat(64)}"`;
  assert.equal((await downloads(request(release, null, cask))).brew, false);
});
test('network and server errors do not masquerade as an unpublished release', async () => {
  await assert.rejects(downloads(async () => response('', 503)), /Availability lookup failed/);
  await assert.rejects(downloads(async () => { throw Error('offline'); }), /offline/);
});

test('the selected published GPUI candidate exposes only verified downloads', async () => {
  const { readFileSync } = await import('node:fs');
  const channel=JSON.parse(readFileSync(new URL('../packaging/desktop-channel.json',import.meta.url)));
  const rc={tag_name:channel.tag,target_commitish:channel.candidate_sha,draft:false,prerelease:true,
    assets:[apps[0].file,'SHA256SUMS'].map(name=>({name,browser_download_url:'https://example.org/'+name}))};
  const fetchRc=async url => url.endsWith('/'+channel.tag) ? response(rc) : response('',404);
  const actual=await downloads(fetchRc);
  assert.equal(actual.apps.length,1);
  assert.ok(actual.apps[0].checksum.endsWith('/SHA256SUMS'));
  assert.equal(actual.brew,false);
  rc.target_commitish='some-other-commit';
  assert.deepEqual((await downloads(fetchRc)).apps,[]);
});

test('a core-only stable release does not hide the reviewed desktop downloads', async () => {
  const {readFileSync}=await import('node:fs');
  const channel=JSON.parse(readFileSync(new URL('../packaging/desktop-channel.json',import.meta.url)));
  const rc={tag_name:channel.tag,target_commitish:channel.candidate_sha,draft:false,prerelease:true,
    assets:[apps[0].file,'SHA256SUMS'].map(name=>({name,browser_download_url:'https://example.org/'+name}))};
  const old={...release,assets:[{name:'creditia-core-x86_64-unknown-linux-gnu',browser_download_url:'https://example.org/core'}]};
  const result=await downloads(async url=>url.endsWith('/'+channel.tag)?response(rc):url.includes('api.github.com')?response(old):response('',404));
  assert.equal(result.apps[0].file,apps[0].file);
  assert.equal(result.desktopTag,channel.tag);
  assert.equal(result.brew,false);
});
