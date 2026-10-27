// Controlled fixture generation. Upstream source and package pins: README.md.
// No model, credentials or external network is used by this generator.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
const [src, modules] = process.argv.slice(2);
if (!src || !modules) throw new Error('Usage: node generate.mjs <pinned Eve source directory> <node_modules with devalue 6.0.3 and @opentelemetry/otlp-transformer 0.214.0>');
const { stringify } = await import(pathToFileURL(path.join(modules, 'devalue/src/stringify.js')));
const require=createRequire(import.meta.url);
const { JsonTraceSerializer }=require(path.join(modules, '@opentelemetry/otlp-transformer'));
const { diffValue }=await import(src+'/packages/eve/src/shared/value-delta.ts');
const { applyPatchToSandbox }=await import(src+'/packages/eve-code/extension/lib/patch.ts');
const { promisify }=await import('node:util');
const { execFile }=await import('node:child_process');
const exec=promisify(execFile);
const directory=await fs.mkdtemp(path.join(os.tmpdir(), 'creditia-eve-native-'));
const code=p=>Array.from({length:8},(_,i)=>`export const ${p}_${i} = ${i} + 739;\n`).join('');
const newText=code('eve_created'),patchText=code('eve_patched');
const nativePatch='*** Begin Patch\n*** Add File: patched.ts\n'+patchText.split('\n').filter(Boolean).map(l=>'+'+l+'\n').join('')+'*** End Patch';
const assertPath=p=>{ if(p!==directory&&!p.startsWith(directory+'/'))throw Error('Outside test workspace');return p;};
const sandbox={
 readTextFile:async({path:p})=>{try{return await fs.readFile(assertPath(p),'utf8')}catch(e){if(e.code==='ENOENT')return null;throw e;}},
 writeTextFile:async({path:p,content})=>fs.writeFile(assertPath(p),content),
 removePath:async({path:p})=>fs.rm(assertPath(p),{force:true}),
 run:async({command})=>{
  // Eve targets Linux sandboxes. Only realpath -m needs a macOS equivalent in
  // this temporary local filesystem harness; the patch source is unmodified.
  const m=/^realpath -m -- '([^']+)'$/.exec(command);
  if(m)return {exitCode:0,stdout:assertPath(path.resolve(m[1]))+'\n',stderr:''};
  if(!/^(mkdir -p |mv (?:-n )?)-- '/.test(command))throw Error('Unexpected sandbox command: '+command);
  try{const {stdout,stderr}=await exec('/bin/sh',['-c',command],{cwd:directory,env:{PATH:'/usr/bin:/bin'}});return {exitCode:0,stdout,stderr}}
  catch(e){return {exitCode:e.code,stdout:e.stdout,stderr:e.stderr}}
 }
};
const files=await applyPatchToSandbox({patchText:nativePatch,patchRoot:directory,sandbox,sessionId:'eve-session'});
if(await fs.readFile(directory+'/patched.ts','utf8')!==patchText)throw Error('Native patch mismatch');
const target=path.dirname(fileURLToPath(import.meta.url));
const before={kind:'channel:github',state:{owner:'creditia-fixtures',repo:'native-eve',repositoryId:178,checkoutPath:null}};
const after={...before,state:{...before.state,checkoutPath:'/app/repository'}};
const input=[{sessionState:{version:2,sessionId:'eve-session'},serializedContext:{'eve.channel':before,'eve.bundle':{source:{kind:'disk',appRoot:'__EVE_BUNDLE_ROOT__'}}},history:[]}];
const output={action:'continue',stateDelta:{serializedContext:diffValue(input[0].serializedContext,{...input[0].serializedContext,'eve.channel':after,'eve.sessionDynamicToolRuntimeRevision':'disk:__EVE_BUNDLE_ROOT__:'+('b'.repeat(64))+':mtime-mghq0000'})}};
const encode=x=>({__type:'Uint8Array',data:Buffer.from('devl'+stringify(x)).toString('base64')});
const step={runId:'wrun_fixture',stepId:'step_fixture',status:'completed',stepName:'step//node_modules/eve/dist/src/execution/session/turn-step.js//turnStep',input:encode({args:input,closureVars:undefined,thisVal:undefined}),output:encode(output),completedAt:'2026-09-17T00:00:02.000Z'};
await fs.writeFile(target+'/checkpoint.json',JSON.stringify(step,null,2)+'\n');
let seq=0;
function span(name,args,result,{framework=true,status=0,session='eve-session',call=name}={}){
 const spanId=(++seq).toString(16).padStart(16,'0');
 const attributes={'agent.trace.schema.version':4,'agent.run.id':session,'gen_ai.operation.name':'execute_tool','agent.tool.is_framework':framework,'gen_ai.tool.call.id':call,'gen_ai.tool.name':name,'gen_ai.tool.call.arguments':JSON.stringify(args),'gen_ai.tool.call.result':JSON.stringify(result)};
 const raw={spanContext:()=>({traceId:'a'.repeat(32),spanId,traceFlags:1}),attributes,resource:{attributes:{'service.name':'eve'}},instrumentationScope:{name:'agent'},name:'execute_tool '+name,kind:0,startTime:[1789603201,0],endTime:[1789603202,0],status:{code:status},events:[],links:[],droppedAttributesCount:0,droppedEventsCount:0,droppedLinksCount:0};
 return JsonTraceSerializer.serializeRequest([raw]);
}
await fs.writeFile(target+'/write.otlp.json',span('write_file',{filePath:'/app/repository/created.ts',content:newText},{existed:false,path:'/app/repository/created.ts'}));
await fs.writeFile(target+'/patch.otlp.json',span('code__apply_patch',{root:'/app/repository',patchText:nativePatch},{diagnostics:[],files},{framework:false}));
await fs.writeFile(target+'/expected.json',JSON.stringify({created:newText,patched:patchText},null,2)+'\n');
await fs.writeFile(target+'/checkpoint-readable.json',JSON.stringify({input:{args:input},output},null,2)+'\n');
console.log(JSON.stringify({nativePatchFiles:files,actualWrittenBytes:Buffer.byteLength(patchText),nativePatchDirectory:directory,fixtureDirectory:target}));
