import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { TARGETS, validateRequest, runMatchesCandidate, validateReport, verifyInstallers, verifyBundle } from './release.mjs';
const sha='a'.repeat(40), head='b'.repeat(40), base='c'.repeat(40);
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const request=()=>({enabled:true,candidate_sha:sha,native_run:'1',artwork_run:'2',performance_run:'3',core_run:'4',review_commit:head,review_path:'docs/review.json',tag:'gpui-v0.2.0-rc.1'});
const ready=()=>({schema_version:1,candidate_sha:sha,release_ready:true,invalid_evidence:[],native_failures:[],feature_failures:[],recorded_review:{status:'complete'},real_project_visual_sample:true,native_platforms_complete:true,native_catalog:{complete:true},browser_catalog:{complete:true},package_platforms:['darwin-arm64','darwin-x64','win32-x64','linux-x64'].map(platform=>({platform,acceptance:{passed:true}})),performance_comparisons:['startup','scroll','roam','zoom','idle','hidden-idle'].flatMap(scenario=>['cold','warm'].map(cache_state=>({scenario,cache_state,status:'comparable'}))),artwork:[{status:'machine_matched',all_preferences:true,motion_sampled:true,exports_checked:true}]});
async function temporary(fn){const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-release-test-'));try{await fn(root);}finally{await fs.rm(root,{recursive:true,force:true});}}

test('release requests default off and reject stable tags, traversal and output injection',()=>{
 assert.deepEqual(validateRequest({enabled:false}),{enabled:false});
 assert.equal(validateRequest(request()).tag,'gpui-v0.2.0-rc.1');
 for(const change of [{enabled:undefined},{candidate_sha:'main'},{review_commit:'HEAD'},{tag:'v0.2.0'},{tag:'gpui-v0.2.0'},{review_path:'../review.json'},{review_path:'review\nenabled=true.json'},{native_run:'1\ntag=evil'}])assert.throws(()=>validateRequest({...request(),...change}));
});
test('successful workflow provenance accepts exact tested PR merge and rejects unrelated heads or base',()=>{
 const run={status:'completed',conclusion:'success',path:'.github/workflows/gpui.yml',repository:{full_name:'org/repo'},head_repository:{full_name:'org/repo'},head_sha:head,event:'pull_request',pull_requests:[{head:{sha:head},base:{sha:base}}]};
 const commit={sha,parents:[{sha:base},{sha:head}]};
 runMatchesCandidate(run,commit,'gpui','org/repo');
 runMatchesCandidate({...run,head_sha:sha,event:'workflow_dispatch'},commit,'gpui','org/repo');
 for(const change of [{conclusion:'failure'},{status:'in_progress'},{path:'.github/workflows/ci.yml'},{head_sha:'d'.repeat(40)},{pull_requests:[{head:{sha:head},base:{sha:'d'.repeat(40)}}]},{head_repository:{full_name:'fork/repo'}}])assert.throws(()=>runMatchesCandidate({...run,...change},commit,'gpui','org/repo'));
});
test('a release_ready boolean cannot replace installed platforms, recorded review and performance gates',()=>{
 validateReport({...ready(),artwork:[],native_catalog:{complete:false},browser_catalog:{complete:false}},sha);
 for(const change of [{candidate_sha:head},{release_ready:false},{package_platforms:[]},{performance_comparisons:[]},{native_failures:[{}]},{recorded_review:{status:'incomplete'}},{feature_failures:[{}]}])assert.throws(()=>validateReport({...ready(),...change},sha));
});
test('installed candidate checksum validation rejects tampered bytes and wrong SHA',async()=>temporary(async root=>{
 const target=TARGETS[0], names=[`creditia-gpui-${target}.zip`,`creditia-gpui-${target}.dmg`];
 for(const name of names)await fs.writeFile(path.join(root,name),`test installer ${name}`);
 const sums=await Promise.all(names.map(async name=>`${digest(await fs.readFile(path.join(root,name)))}  ${name}`));
 await fs.writeFile(path.join(root,'candidate.json'),JSON.stringify({target,candidate_sha:sha,installers:names}));
 await fs.writeFile(path.join(root,'SHA256SUMS'),sums.join('\n')+'\n');
 assert.equal((await verifyInstallers(root,sha,target)).length,2);
 await assert.rejects(verifyInstallers(root,head,target));
 await fs.appendFile(path.join(root,names[0]),'changed');
 await assert.rejects(verifyInstallers(root,sha,target),/checksum/);
}));
test('publish verifies sealed assets, reconstructed evidence and rejects added or changed files',async()=>temporary(async root=>{
 const assets=path.join(root,'assets');await fs.mkdir(assets);
 const evidence=Buffer.from('explicit release validator fixture; not real acceptance evidence');
 const values={'release-notes.md':'test notes','assets/report.json':JSON.stringify(ready()),'assets/REPORT.md':'explicit report fixture','assets/ACCEPTANCE.zh.md':'验收报告测试样例','assets/ARTWORK-STATUS.tsv':'row\tstatus\n','assets/FEATURE-STATUS.tsv':'feature\tstatus\n','assets/SHA256SUMS':'fixture checksums','assets/evidence.tar.part0':evidence,'assets/evidence-archive.json':JSON.stringify({name:'gpui-acceptance-evidence.tar.gz',sha256:digest(evidence),bytes:evidence.length,parts:[{path:'evidence.tar.part0',sha256:digest(evidence),bytes:evidence.length}]})};
 for(const target of TARGETS)for(const extension of (target.includes('apple')?['.zip','.dmg']:target.includes('windows')?['.zip','-setup.exe']:['.tar.gz','.deb']))values[`assets/creditia-gpui-${target}${extension}`]='explicit installer fixture';
 const records=[];for(const [name,bytes]of Object.entries(values)){await fs.writeFile(path.join(root,name),bytes);records.push({path:name,bytes:Buffer.byteLength(bytes),sha256:digest(bytes)});}
 const manifest=Buffer.from(JSON.stringify({schema_version:1,repository:'org/repo',candidate_sha:sha,tag:'gpui-v0.2.0-rc.1',draft:true,prerelease:true,release_ready:true,assets:records}));
 await fs.writeFile(path.join(root,'release-bundle.json'),manifest);
 assert.equal((await verifyBundle(root,digest(manifest))).candidate_sha,sha);
 await assert.rejects(verifyBundle(root,'0'.repeat(64)),/manifest checksum/);
 await fs.writeFile(path.join(assets,'injected.txt'),'extra');
 await assert.rejects(verifyBundle(root,digest(manifest)),/injected/);
 await fs.rm(path.join(assets,'injected.txt'));
 await fs.appendFile(path.join(assets,'evidence.tar.part0'),'modified');
 await assert.rejects(verifyBundle(root,digest(manifest)),/asset changed/);
}));
