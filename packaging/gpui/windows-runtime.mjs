#!/usr/bin/env node
// Resolve actual PE imports, then copy only required release VC runtime DLLs.
// Microsoft documents application-local deployment and the redistributable set:
// https://learn.microsoft.com/en-us/cpp/windows/determining-which-dlls-to-redistribute
// https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { files } from './package.mjs';

if(process.platform!=='win32')throw new Error('Windows PE inspection must run on the Windows builder');
const args=Object.fromEntries(process.argv.slice(2).reduce((out,arg,i,all)=>i%2?out:[...out,[arg.replace(/^--/,''),all[i+1]]],[]));
if(!args.root||!args.output)throw new Error('Usage: windows-runtime.mjs --root package-directory --output evidence-directory');
const root=path.resolve(args.root),output=path.resolve(args.output);
await fs.mkdir(output,{recursive:true});
const vswhere=path.join(process.env['ProgramFiles(x86)'],'Microsoft Visual Studio/Installer/vswhere.exe');
const installations=JSON.parse(execFileSync(vswhere,['-latest','-products','*','-requires','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','-format','json'],{encoding:'utf8'}));
const installation=installations[0];
if(!installation)throw new Error('No Visual C++ toolchain installation is available');
const vc=path.join(installation.installationPath,'VC');
const compareVersion=(a,b)=>b.localeCompare(a,undefined,{numeric:true});
const toolVersions=(await fs.readdir(path.join(vc,'Tools/MSVC'))).sort(compareVersion);
let dumpbin;
for(const version of toolVersions){const candidate=path.join(vc,'Tools/MSVC',version,'bin/Hostx64/x64/dumpbin.exe');try{await fs.access(candidate);dumpbin=candidate;break;}catch{}}
if(!dumpbin)throw new Error('dumpbin was not found in the Visual C++ toolchain');
const redistRoot=path.join(vc,'Redist/MSVC');
const redistVersions=(await fs.readdir(redistRoot)).filter(name=>/^\d+\.\d+/.test(name)).sort(compareVersion);
let redist;
for(const version of redistVersions){const candidate=path.join(redistRoot,version,'x64');try{await fs.access(candidate);redist=candidate;break;}catch{}}
if(!redist)throw new Error('The x64 Visual C++ redistributable files are unavailable');
const available=new Map();
for(const directory of await fs.readdir(redist,{withFileTypes:true})){
  if(!directory.isDirectory()||!/^Microsoft\.VC\d+\.(CRT|OpenMP)$/i.test(directory.name))continue;
  for(const file of await fs.readdir(path.join(redist,directory.name)))if(file.endsWith('.dll'))available.set(file.toLowerCase(),path.join(redist,directory.name,file));
}
const runtime=/^(vcruntime\d+(?:_\d+)?|msvcp\d+(?:_[a-z0-9_]+)?|concrt\d+|vcomp\d+)\.dll$/i;
const pe=(await files(root)).filter(file=>/\.(exe|dll|node)$/i.test(file));
const pending=[...pe],seen=new Set(),dependencies=[],bundled=[],transcript=[];
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
while(pending.length){
  const relative=pending.shift();if(seen.has(relative))continue;seen.add(relative);
  const source=path.join(root,relative);
  const text=execFileSync(dumpbin,['/NOLOGO','/DEPENDENTS',source],{encoding:'utf8',maxBuffer:8*1024*1024});
  transcript.push(`### ${relative}\n${text}`);
  const imports=[...new Set([...text.matchAll(/^\s+([a-z0-9_.-]+\.dll)\s*$/gmi)].map(match=>match[1]))];
  dependencies.push({path:relative,imports});
  for(const name of imports){
    if(/^(vcruntime|msvcp|concrt|vcomp).*d\.dll$/i.test(name))throw new Error(`A release artifact imports a non-redistributable debug runtime: ${name}`);
    if(!runtime.test(name))continue;
    const original=available.get(name.toLowerCase());
    if(!original)throw new Error(`Required redistributable DLL not found in the release redist directory: ${name}`);
    // Executable directories must each be self-contained. Node's native addons
    // resolve their CRT through resources/node.exe's executable directory.
    const destination=relative.endsWith('.node')?path.join(root,'resources',name):path.join(path.dirname(source),name);
    const destinationRelative=path.relative(root,destination).split(path.sep).join('/');
    if(bundled.some(file=>file.path.toLowerCase()===destinationRelative.toLowerCase()))continue;
    await fs.copyFile(original,destination);
    const bytes=await fs.readFile(destination);
    bundled.push({path:destinationRelative,source:path.relative(installation.installationPath,original).split(path.sep).join('/'),bytes:bytes.length,sha256:digest(bytes)});
    pending.push(destinationRelative);
  }
}
const licenses=path.join(root,'resources/licenses/Microsoft-VC');
if(bundled.length){
  if(!installation.installationVersion.startsWith('17.'))throw new Error('Update the retained runtime license for this Visual Studio generation before redistributing it');
  await fs.mkdir(licenses,{recursive:true});
  const license='https://visualstudio.microsoft.com/wp-content/uploads/2021/09/Visual-C-Runtime-2015-2022-License-1.docx';
  const response=await fetch(license,{signal:AbortSignal.timeout(60_000)});
  if(!response.ok)throw new Error(`Cannot retain the Microsoft runtime license: ${response.status}`);
  const bytes=Buffer.from(await response.arrayBuffer());
  if(bytes.subarray(0,2).toString()!=='PK')throw new Error('Runtime license download is not the expected DOCX');
  await fs.writeFile(path.join(licenses,'Microsoft-VC-Runtime-License.docx'),bytes);
  await fs.writeFile(path.join(licenses,'redistribution.txt'),`Microsoft Visual C++ Runtime DLLs, unmodified from the build toolchain release redistributable directory.\n\nLicense source: ${license}\nLicense SHA-256: ${digest(bytes)}\nRedistribution list: https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution\nDeployment documentation: https://learn.microsoft.com/en-us/cpp/windows/determining-which-dlls-to-redistribute\n\nApplication-local runtime copies are updated with the application; retain current security updates when rebuilding.\n`);
}
const report={schema_version:1,candidate_sha:process.env.GITHUB_SHA,visual_studio_version:installation.installationVersion,toolset:path.relative(vc,path.dirname(dumpbin)),redist:path.relative(vc,redist),dependencies,bundled,scope:'PE import tables, including delay imports shown by dumpbin; operating-system DLLs remain system dependencies'};
await fs.writeFile(path.join(output,'windows-dependencies.json'),JSON.stringify(report,null,2)+'\n');
await fs.writeFile(path.join(output,'windows-dependencies.txt'),transcript.join('\n'));
await fs.writeFile(path.join(root,'resources/licenses/Windows-runtime-provenance.json'),JSON.stringify(report,null,2)+'\n');
console.log(`Inspected ${dependencies.length} PE files; bundled ${bundled.length} required release runtime DLLs`);
