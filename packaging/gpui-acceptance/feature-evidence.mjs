#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { args, artifact, countBy } from './common.mjs';
export function testOutcomes(text) {
 const results=[];
 for(const [i,line]of text.split(/\r?\n/).entries()){
  const plain=line.replace(/\u001b\[[0-9;]*m/g,'').replace(/^\uFEFF?/,'').replace(/^\d{4}-\d\d-\d\dT[\d:.]+Z\s+/, '');
  const match=/^test ([\w:./-]+) \.\.\. (ok|FAILED|ignored)(?:\s|$)/.exec(plain);
  if(match) { results.push({test_id:match[1],status:match[2]==='ok'?'passed':match[2]==='FAILED'?'failed':'not_run',line:i+1}); continue; }
  const spec=/^\s*([✔✓✖]) (.+?) \([\d.]+(?:ms|s)\)$/.exec(plain);
  if(spec) { results.push({test_id:spec[2],status:spec[1]==='✖'?'failed':'passed',line:i+1}); continue; }
  const tap=/^(not )?ok \d+ - (.*?)(?: # (SKIP|TODO).*)?$/.exec(plain);
  if(tap)results.push({test_id:tap[2],status:tap[3]?'not_run':tap[1]?'failed':'passed',line:i+1});
 }
 return results;
}
async function main() {
const options=args();
if(!options.logs||!options.bindings||!options.output||!options['candidate-sha'])throw Error('logs, bindings, output and candidate-sha are required');
const logs=path.resolve(options.logs),output=path.resolve(options.output),bindings=JSON.parse(fs.readFileSync(options.bindings));
const provenance=JSON.parse(fs.readFileSync(path.join(logs,'provenance.json')));
if(provenance.candidate_sha!==options['candidate-sha']||!/^([a-f0-9]{40})$/.test(options['candidate-sha']))throw Error('Test-log provenance belongs to a different candidate');
const outcomes=new Map(),artifacts=[];
for(const source of provenance.sources){
 if(path.basename(source.file)!==source.file)throw Error('Test log path must be a basename');
 const bytes=fs.readFileSync(path.join(logs,source.file)),text=bytes.toString();
 if(!text.includes(provenance.candidate_sha))throw Error(`Test log lacks its recorded checkout commit: ${source.file}`);
 artifacts.push(artifact(output,`logs/${source.file}`,bytes));
 for(const result of testOutcomes(text)){
  const record={...result,log:`logs/${source.file}`,job_id:source.job_id,job_name:source.job_name,run_id:source.run_id,platform:source.platform};
  if(!outcomes.has(result.test_id))outcomes.set(result.test_id,[]);
  outcomes.get(result.test_id).push(record);
 }
}
const cases=bindings.bindings.map((binding,index)=>{
 const tests=binding.required_tests.map(id=>({test_id:id,outcomes:outcomes.get(id)??[]}));
 const failed=tests.some(test=>test.outcomes.some(result=>result.status==='failed'));
 const passed=tests.every(test=>test.outcomes.some(result=>result.status==='passed'));
 return {key:`${binding.feature_id}/${index+1}`,feature_id:binding.feature_id,check:binding.check,
  test_id:binding.required_tests.join(' + '),behavior:binding.behavior,status:failed?'failed':passed?'passed':'not_run',tests};
});
artifacts.push(artifact(output,'bindings.json',fs.readFileSync(options.bindings)),artifact(output,'test-log-provenance.json',JSON.stringify(provenance,null,2)+'\n'));
const manifest={schema_version:1,kind:'feature-checks',complete:true,provenance:{candidate_sha:provenance.candidate_sha,source:'Executed Rust/Node test command logs or decoded GitHub Actions job logs, with recorded checkout commit, per-test outcomes and explicit inspected assertion mappings'},cases,artifacts,
 scope:{mapped_checks:cases.length,summary:countBy(cases,c=>c.status),limitation:'Mappings cover only their stated backend/native-unit behavior. Missing, ignored or merely compiled tests remain not_run. UI routes and other checklist entries are not implied.'}};
fs.writeFileSync(path.join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
console.log(JSON.stringify({output,...manifest.scope.summary}));

}
if(process.argv[1]&&pathToFileURL(path.resolve(process.argv[1])).href===import.meta.url)await main();
