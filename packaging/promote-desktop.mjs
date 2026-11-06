// Copy the reviewed GPUI payloads into a version release; the installers have
// already been opened and exercised on each target in the GPUI release workflow.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, copyFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { apps, version, ROOT } from './release.mjs';
import { validateReport, fileHash } from './gpui/release.mjs';

const destination = process.argv[2];
if (!destination) throw Error('Expected a release assets directory');
const channel = JSON.parse(readFileSync(path.join(ROOT, 'packaging/desktop-channel.json')));
if (!/^gpui-v[0-9]+\.[0-9]+\.[0-9]+-[a-zA-Z0-9.-]+$/.test(channel.tag) ||
    !channel.tag.startsWith(`gpui-v${version}-`) || !/^[a-f0-9]{40}$/.test(channel.candidate_sha)) {
  throw Error('Select a reviewed GPUI desktop release for this version');
}
const repo = 'WroughtMind/creditia';
const meta = JSON.parse(execFileSync('gh', ['api',`repos/${repo}/releases/tags/${channel.tag}`]));
if (meta.draft || meta.tag_name !== channel.tag) throw Error('Desktop prerelease has not been published');
const ref = `refs/tags/${channel.tag}`;
execFileSync('git', ['fetch','--quiet','origin',ref],{cwd:ROOT});
const sha = execFileSync('git',['rev-parse','FETCH_HEAD^{commit}'],{cwd:ROOT,encoding:'utf8'}).trim();
if (sha !== channel.candidate_sha) throw Error('Desktop release tag does not match the reviewed build');
execFileSync('git',['diff','--exit-code',sha,'HEAD','--','apps/gpui/src','apps/gpui/Cargo.toml','apps/gpui/Cargo.lock','crates','Cargo.toml','Cargo.lock','packages/render','packages/schema','apps/desktop/src-tauri','packaging/gpui/package.mjs','packaging/gpui/installers.mjs','packaging/gpui/licenses.mjs','packaging/gpui/mac-icons.mjs','packaging/gpui/node-runtime.py','packaging/gpui/windows-runtime.mjs','.github/workflows/gpui.yml'],{cwd:ROOT,stdio:'inherit'});
const temp = mkdtempSync(path.join(tmpdir(),'creditia-desktop-release-'));
try {
  const names = ['report.json','SHA256SUMS',...apps.map(a => a.file)];
  for (const name of names) {
    if (!meta.assets.some(a => a.name === name)) throw Error(`Desktop asset missing: ${name}`);
  }
  execFileSync('gh',['release','download',channel.tag,'--repo',repo,'--dir',temp,...names.flatMap(n=>['--pattern',n])],{stdio:'inherit'});
  const report = JSON.parse(readFileSync(path.join(temp,'report.json')));
  validateReport(report,sha);
  const sums = new Map(readFileSync(path.join(temp,'SHA256SUMS'),'utf8').trim().split('\n').map(s=>{
    const m=/^([a-f0-9]{64})  ([a-zA-Z0-9_.-]+)$/.exec(s); if (!m) throw Error('Invalid desktop checksum'); return [m[2],m[1]];
  }));
  for (const {file} of apps) {
    const digest=await fileHash(path.join(temp,file));
    if (sums.get(file)!==digest || meta.assets.find(a=>a.name===file).digest!==`sha256:${digest}`) throw Error(`Desktop checksum mismatch: ${file}`);
    copyFileSync(path.join(temp,file),path.join(destination,file));
  }
} finally { rmSync(temp,{recursive:true,force:true}); }
