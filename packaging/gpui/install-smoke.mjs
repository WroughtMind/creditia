#!/usr/bin/env node
// Exercise the files users install, from a new directory outside the checkout.
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { bridgeRequest, layout, verify } from './package.mjs';

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

async function exercise(root,format,{uninstaller=false}={}){
  const info=JSON.parse(await fs.readFile(path.join(root,'package-info.json'),'utf8'));
  if(info.target!==candidate.target||info.commit!==candidate.candidate_sha)throw new Error(`Wrong ${format} candidate provenance`);
  if(uninstaller){
    // NSIS adds its generated uninstaller; it is not part of the sealed runtime.
    const generated=path.join(root,'uninstall.exe'), parked=path.join(temporary,'uninstall.exe');
    await fs.rename(generated,parked);
    try{await verify(root);}finally{await fs.rename(parked,generated);}
  }else await verify(root);
  const paths=layout(root,info.platform);
  run(paths.gui,['--check-resources']);
  run(paths.core,['help']);
  report.checks.push({id:`${format}-installed-payload-and-resource-launch`,passed:true,files:info.files.length});
  const repository = path.join(temporary, 'existing-project');
  const archive = path.join(repository, '.creditia', 'credits.json');
  if (!(await fs.stat(repository).catch(() => null))) {
    await fs.mkdir(repository);
    run('git', ['init', repository]);
    await fs.writeFile(path.join(repository, 'README.md'), '# Existing project\n');
    run('git', ['-C', repository, 'add', 'README.md']);
    run('git', ['-C', repository, '-c', 'user.name=Creditia QA', '-c', 'user.email=qa@example.invalid',
      'commit', '-m', 'Initial project']);
    run(paths.core, ['analyze', '--repo', repository]);
    const legacy = JSON.parse(await fs.readFile(fixture, 'utf8'));
    legacy.head = run('git', ['-C', repository, 'rev-parse', 'HEAD']).toString().trim();
    await fs.writeFile(archive, JSON.stringify(legacy));
  }
  const original = await fs.readFile(archive);
  const raw = JSON.parse(original);
  for (const phase of ['launch', 'restart']) {
    const label = `${format}-${phase}`;
    const captureRoot = path.join(path.dirname(output), label);
    const platformCheck = phase === 'launch' && ['zip', 'tar'].includes(format);
    if (process.platform === 'win32') {
      run('pwsh', ['-NoProfile', '-STA', '-File', path.join(import.meta.dirname, 'windows-window.ps1'),
        '-PackageRoot', root, '-ProjectName', raw.project.name, '-Candidate', candidate.candidate_sha, '-Output', captureRoot, ...(platformCheck ? ['-CheckPlatform'] : [])], {timeout: 240_000});
    } else {
      run('python', [path.join(import.meta.dirname, 'gui-smoke.py'), '--binary', paths.gui,
        '--archive', archive, '--library', env.CREDITIA_HOME, '--output', captureRoot,
        '--candidate', candidate.candidate_sha, ...(platformCheck ? ['--platform-check'] : [])], {timeout: 330_000});
    }
    const captureFile = process.platform === 'win32' ? 'manifest.json' : 'window-evidence.json';
    const capture = JSON.parse(await fs.readFile(path.join(captureRoot, captureFile), 'utf8'));
    if (!capture.complete || capture.provenance.candidate_sha !== candidate.candidate_sha)
      throw new Error(`Installed project did not open: ${label}`);
    report.checks.push({id: label, passed: true, source: `${label}/${captureFile}`});
    if (platformCheck) {
      const checks = capture.checks ?? capture.cases.flatMap(item => item.checks);
      const verified = checks.some(check => check.id === 'system-clipboard-and-folder-dialog' && check.passed);
      if (!verified && !capture.manual_checks?.length) throw new Error('Missing clipboard and file dialog result');
      report.platform_input = {verified, source: `${label}/${captureFile}`, manual_checks: capture.manual_checks ?? []};
    }
    for (const file of await fs.readdir(captureRoot)) {
      const bytes = await fs.readFile(path.join(captureRoot, file));
      report.artifacts.push({path: `${label}/${file}`, bytes: bytes.length, sha256: digest(bytes)});
    }
    if (phase === 'launch') {
      const result = await bridgeRequest(paths.node, paths.bridge, {version: 1, id: label,
        op: 'export', raw, style: 'minimal-light', piece: 'asterisk', format: 'png', size: 'native'}, temporary);
      const bytes = Buffer.from(result.bytes, 'base64');
      if (result.ext !== 'png' || !bytes.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex')))
        throw new Error('Installed renderer returned an invalid PNG');
      await artifact(`${format}-installed.png`, bytes);
      report.checks.push({id: `${format}-installed-export`, passed: true});
    }
  }
  if (!(await fs.readFile(archive)).equals(original)) throw new Error('Opening the project changed its archive');
}

async function checkAnalysis(paths) {
  const repo = path.join(temporary, 'analysis-retry'), library = path.join(temporary,'analysis-library');
  await fs.mkdir(repo);
  run('git',['init',repo]);
  await fs.writeFile(path.join(repo,'initial.rs'),'pub fn initial() {}\n');
  run('git',['-C',repo,'add','initial.rs']);
  run('git',['-C',repo,'-c','user.name=Creditia QA','-c','user.email=qa@example.invalid','commit','-m','Initial source']);
  run(paths.core,['analyze','--repo',repo],{env:{...env,CREDITIA_HOME:library}});
  const capture = path.join(path.dirname(output),'analysis-retry');
  run('python',[path.join(import.meta.dirname,'gui-smoke.py'),'--binary',paths.gui,
    '--archive',path.join(repo,'.creditia/credits.json'),'--library',library,'--output',capture,
    '--candidate',candidate.candidate_sha,'--analysis-check'],{timeout:330_000});
  const result = JSON.parse(await fs.readFile(path.join(capture,'window-evidence.json'),'utf8'));
  if (!result.complete || !result.checks.some(c=>c.id==='analysis-cancel-retry' && c.passed))
    throw Error('Installed analysis/cancel/retry did not complete');
  report.checks.push({id:'installed-analysis-cancel-retry',passed:true,source:'analysis-retry/window-evidence.json'});
  for(const file of await fs.readdir(capture)) {
    const bytes=await fs.readFile(path.join(capture,file));
    report.artifacts.push({path:`analysis-retry/${file}`,bytes:bytes.length,sha256:digest(bytes)});
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
      const installed = path.join(extracted,name.slice(0,-7));
      await exercise(installed,'tar');
      await checkAnalysis(layout(installed,'linux'));
    }else if(name.endsWith('.deb')){
      run('dpkg-deb',['--extract',artifact,extracted]);
      await exercise(path.join(extracted,'usr/lib/creditia-gpui'),'deb');
      if(await fs.readlink(path.join(extracted,'usr/bin/creditia-gpui'))!=='../lib/creditia-gpui/creditia-gpui')throw new Error('Invalid installed command link');
      report.checks.push({id:'deb-command-link',passed:true});
    }else if(name.endsWith('.dmg')){
      // Copy the app as a user would. LaunchServices and helper processes can
      // otherwise keep the read-only DMG busy after an otherwise successful run.
      const mount=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-dmg-'));
      let mounted=false;
      try{
        run('hdiutil',['attach','-readonly','-nobrowse','-mountpoint',mount,artifact]);
        mounted=true;
        run('ditto',[path.join(mount,'Creditia.app'),path.join(extracted,'Creditia.app')]);
        for(const file of ['INSTALL.md','package-info.json']){
          await fs.copyFile(path.join(mount,file),path.join(extracted,file));
        }
      }finally{
        if(mounted){
          try{run('hdiutil',['detach',mount]);}
          catch(error){
            if(error.status!==16)throw error;
            run('hdiutil',['detach',mount,'-force']);
          }
        }
        // hdiutil may remove the mountpoint itself. Never recurse into a volume.
        await fs.rmdir(mount).catch(error=>{if(error.code!=='ENOENT')throw error;});
      }
      run('codesign',['--verify','--deep','--strict',path.join(extracted,'Creditia.app')]);
      await exercise(extracted,'dmg');
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
