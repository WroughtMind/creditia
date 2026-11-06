#!/usr/bin/env node
// Exercise the files users install, from a new directory outside the checkout.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { layout, verify } from './package.mjs';

const options = Object.fromEntries(process.argv.slice(2).reduce((pairs,arg,i,all) => i%2 ? pairs : [...pairs,[arg.replace(/^--/,''),all[i+1]]],[]));
if (!options.installers || !options.output || !options.fixture) throw new Error('Usage: install-smoke.mjs --installers installer-directory --output evidence.json --fixture archive.json');
const directory=path.resolve(options.installers), output=path.resolve(options.output);
const fixture=path.resolve(options.fixture);
const candidate=JSON.parse(await fs.readFile(path.join(directory,'candidate.json'),'utf8'));
const temporary=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-install-'));
const checksums=new Map((await fs.readFile(path.join(directory,'SHA256SUMS'),'utf8')).trim().split('\n').map(line=>{
  const match=/^([a-f0-9]{64})  ([a-zA-Z0-9_.-]+)$/.exec(line);
  if(!match)throw new Error('Malformed installer checksum');
  return [match[2],match[1]];
}));
const report={schema_version:1,kind:'package',platform:`${process.platform}-${process.arch}`,target:candidate.target,provenance:{candidate_sha:candidate.candidate_sha},acceptance:{passed:false},complete:false,checks:[],installers:[],artifacts:[]};
const env={...process.env,CREDITIA_HOME:path.join(temporary,'library')};
for(const key of ['CREDITIA_CORE','CREDITIA_NODE','CREDITIA_RENDER_BRIDGE','NODE_PATH','NODE_OPTIONS'])delete env[key];
const transcript=[];
const run=(program,args,settings={})=>{
  transcript.push(`$ ${JSON.stringify([program,...args])}`);
  try{const result=execFileSync(program,args,{cwd:temporary,env,timeout:180_000,stdio:'pipe',...settings});transcript.push(String(result),'exit: 0');return result;}
  catch(error){transcript.push(String(error.stdout??''),String(error.stderr??''),`exit: ${error.status??error.code}`);throw error;}
};
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const artifact=async(name,bytes)=>{await fs.writeFile(path.join(path.dirname(output),name),bytes);report.artifacts.push({path:name,bytes:Buffer.byteLength(bytes),sha256:digest(bytes)});};

async function exercise(root,format,{payloadOnly=false,uninstaller=false}={}){
  const info=JSON.parse(await fs.readFile(path.join(root,'package-info.json'),'utf8'));
  if(info.target!==candidate.target||info.commit!==candidate.candidate_sha)throw new Error(`Wrong ${format} candidate provenance`);
  if(payloadOnly){
    // Read-only DMGs also contain an Applications link and volume metadata.
    // Every sealed payload file must still have its original bytes.
    for(const item of info.files){
      if(path.posix.isAbsolute(item.path)||item.path.split('/').includes('..'))throw new Error('Unsafe manifest path');
      const bytes=await fs.readFile(path.join(root,item.path));
      if(bytes.length!==item.size||digest(bytes)!==item.sha256)throw new Error(`Installed payload differs: ${item.path}`);
    }
  }else if(uninstaller){
    // NSIS adds its generated uninstaller; it is not part of the sealed runtime.
    const generated=path.join(root,'uninstall.exe'), parked=path.join(temporary,'uninstall.exe');
    await fs.rename(generated,parked);
    try{await verify(root);}finally{await fs.rename(parked,generated);}
  }else await verify(root);
  const paths=layout(root,info.platform);
  run(paths.gui,['--check-resources']);
  run(paths.core,['help']);
  report.checks.push({id:`${format}-installed-payload-and-resource-launch`,passed:true,files:info.files.length});
  if(['darwin','linux'].includes(process.platform)){
    const captureRoot=path.join(path.dirname(output),`${format}-native-window`);
    try{
      run('python',[path.join(import.meta.dirname,'gui-smoke.py'),'--binary',paths.gui,'--archive',fixture,'--output',captureRoot,'--candidate',candidate.candidate_sha],{timeout:330_000});
      const capture=JSON.parse(await fs.readFile(path.join(captureRoot,'window-evidence.json'),'utf8'));
      if(!capture.complete||capture.provenance.candidate_sha!==candidate.candidate_sha||!capture.checks.every(check=>check.passed))throw new Error('Installed native window evidence is incomplete');
      report.checks.push({id:`${format}-installed-native-window-and-normal-quit`,passed:true,source:`${format}-native-window/window-evidence.json`});
    }finally{
      for(const file of await fs.readdir(captureRoot).catch(()=>[])){
        const bytes=await fs.readFile(path.join(captureRoot,file));
        report.artifacts.push({path:`${format}-native-window/${file}`,bytes:bytes.length,sha256:digest(bytes)});
      }
    }
  }
}

try{
  for(const name of candidate.installers){
    if(!checksums.has(name)||path.basename(name)!==name)throw new Error(`Unlisted or unsafe installer: ${name}`);
    const artifact=path.join(directory,name), bytes=await fs.readFile(artifact), hash=digest(bytes);
    if(hash!==checksums.get(name))throw new Error(`Installer checksum mismatch: ${name}`);
    report.installers.push({name,sha256:hash,bytes:bytes.length});
    const extracted=path.join(temporary,name.replace(/\.(tar\.gz|zip|deb|dmg|exe)$/,''));
    await fs.mkdir(extracted,{recursive:true});
    if(name.endsWith('.zip')){
      if(process.platform==='darwin')run('ditto',['-x','-k',artifact,extracted]);
      else run(path.join(process.env.SystemRoot,'System32','tar.exe'),['-xf',artifact,'-C',extracted]);
      await exercise(path.join(extracted,name.slice(0,-4)),'zip');
    }else if(name.endsWith('.tar.gz')){
      run('tar',['-xzf',artifact,'-C',extracted]);
      await exercise(path.join(extracted,name.slice(0,-7)),'tar');
    }else if(name.endsWith('.deb')){
      run('dpkg-deb',['--extract',artifact,extracted]);
      await exercise(path.join(extracted,'usr/lib/creditia-gpui'),'deb');
      if(await fs.readlink(path.join(extracted,'usr/bin/creditia-gpui'))!=='../lib/creditia-gpui/creditia-gpui')throw new Error('Invalid installed command link');
      report.checks.push({id:'deb-command-link',passed:true});
    }else if(name.endsWith('.dmg')){
      let mounted=false;
      try{
        run('hdiutil',['attach','-readonly','-nobrowse','-mountpoint',extracted,artifact]);mounted=true;
        run('codesign',['--verify','--deep','--strict',path.join(extracted,'Creditia.app')]);
        await exercise(extracted,'dmg',{payloadOnly:true});
      }finally{if(mounted)run('hdiutil',['detach',extracted]);}
    }else if(name.endsWith('-setup.exe')){
      run(artifact,['/S',`/D=${extracted}`]);
      await exercise(extracted,'nsis',{uninstaller:true});
      const sentinel=path.join(env.CREDITIA_HOME,'keep-user-data');
      await fs.mkdir(env.CREDITIA_HOME,{recursive:true});await fs.writeFile(sentinel,'preserved');
      run(path.join(extracted,'uninstall.exe'),['/S',`_?=${extracted}`]);
      if(await fs.readFile(sentinel,'utf8')!=='preserved')throw new Error('Uninstall changed the user library');
      try{await fs.access(path.join(extracted,'creditia-gpui.exe'));throw new Error('Uninstall left the application executable');}catch(error){if(error.code!=='ENOENT')throw error;}
      report.checks.push({id:'nsis-uninstall-preserves-user-library',passed:true});
    }else throw new Error(`Unknown installer format: ${name}`);
    await fs.rm(extracted,{recursive:true,force:true});
  }
  report.complete=true;report.acceptance.passed=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
  await fs.mkdir(path.dirname(output),{recursive:true});
  await artifact('installation-transcript.txt',transcript.join('\n')+'\n');
  await artifact('SHA256SUMS',await fs.readFile(path.join(directory,'SHA256SUMS')));
  await artifact('candidate.json',await fs.readFile(path.join(directory,'candidate.json')));
  await fs.writeFile(output,JSON.stringify(report,null,2)+'\n');
  await fs.rm(temporary,{recursive:true,force:true});
}
console.log(JSON.stringify({complete:report.complete,checks:report.checks.length,error:report.error}));
